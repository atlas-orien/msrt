use std::{
    env, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::{Duration, Instant},
};

mod support;

use msrt::{
    endpoint::{ClientEndpoint, EndpointPoll, PassiveEndpoint, PeerState},
    engine::{EngineConfig, ReceiveReport, SendFailedEvent},
};

use support::noise::{NoiseConfig, NoiseLcg, NoiseStats, mutate_or_copy, parse_percent_per_mille};

const TX_BUF_BYTES: usize = 256;
const MAX_MESSAGE_BYTES: usize = 256;
const DEFAULT_MESSAGE_BYTES: usize = 240;
const TEST_FRAGMENT_BYTES: usize = 48;
const DEFAULT_CORRUPT_PER_MILLE: u16 = 10;
const DEFAULT_DROP_BYTE_PER_MILLE: u16 = 10;
const DEFAULT_INSERT_BYTE_PER_MILLE: u16 = 10;
const DEFAULT_BURST_CORRUPT_PER_MILLE: u16 = 10;
const DEFAULT_BURST_DROP_PER_MILLE: u16 = 10;
const DEFAULT_PACKET_DROP_PER_MILLE: u16 = 10;
const PROFILE_NOISE_PER_MILLE: u16 = 30;
const DEFAULT_LOG_FILE: &str = "log/msrt-sim-duplex.log";
const STATUS_INTERVAL: Duration = Duration::from_secs(60);
const IDLE_SLEEP: Duration = Duration::from_micros(50);

#[derive(Clone, Debug)]
struct Args {
    interval_ms: u64,
    duration_ms: u64,
    message: Vec<u8>,
    noise: NoiseConfig,
    log_file: PathBuf,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut parsed = Self {
            interval_ms: 1,
            duration_ms: u64::MAX,
            message: make_message(DEFAULT_MESSAGE_BYTES),
            log_file: PathBuf::from(DEFAULT_LOG_FILE),
            noise: NoiseConfig {
                corrupt_per_mille: DEFAULT_CORRUPT_PER_MILLE,
                drop_byte_per_mille: DEFAULT_DROP_BYTE_PER_MILLE,
                insert_byte_per_mille: DEFAULT_INSERT_BYTE_PER_MILLE,
                burst_corrupt_per_mille: DEFAULT_BURST_CORRUPT_PER_MILLE,
                burst_drop_per_mille: DEFAULT_BURST_DROP_PER_MILLE,
                packet_drop_per_mille: DEFAULT_PACKET_DROP_PER_MILLE,
            },
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--interval-ms" => {
                    parsed.interval_ms = next_value(&mut args, "--interval-ms")?
                        .parse()
                        .map_err(|error| format!("invalid --interval-ms: {error}"))?;
                }
                "--duration-sec" => {
                    let secs: u64 = next_value(&mut args, "--duration-sec")?
                        .parse()
                        .map_err(|error| format!("invalid --duration-sec: {error}"))?;
                    parsed.duration_ms = secs.saturating_mul(1000);
                }
                "--message-size" => {
                    let len = next_value(&mut args, "--message-size")?
                        .parse()
                        .map_err(|error| format!("invalid --message-size: {error}"))?;
                    parsed.message = make_message(len);
                }
                "--log-file" => {
                    parsed.log_file = PathBuf::from(next_value(&mut args, "--log-file")?);
                }
                "--noise-percent" => {
                    parsed.noise.corrupt_per_mille = parse_percent_per_mille(
                        next_value(&mut args, "--noise-percent")?,
                        "--noise-percent",
                    )?;
                }
                "--drop-byte-percent" => {
                    parsed.noise.drop_byte_per_mille = parse_percent_per_mille(
                        next_value(&mut args, "--drop-byte-percent")?,
                        "--drop-byte-percent",
                    )?;
                }
                "--insert-byte-percent" => {
                    parsed.noise.insert_byte_per_mille = parse_percent_per_mille(
                        next_value(&mut args, "--insert-byte-percent")?,
                        "--insert-byte-percent",
                    )?;
                }
                "--burst-corrupt-percent" => {
                    parsed.noise.burst_corrupt_per_mille = parse_percent_per_mille(
                        next_value(&mut args, "--burst-corrupt-percent")?,
                        "--burst-corrupt-percent",
                    )?;
                }
                "--burst-drop-percent" => {
                    parsed.noise.burst_drop_per_mille = parse_percent_per_mille(
                        next_value(&mut args, "--burst-drop-percent")?,
                        "--burst-drop-percent",
                    )?;
                }
                "--packet-drop-percent" => {
                    parsed.noise.packet_drop_per_mille = parse_percent_per_mille(
                        next_value(&mut args, "--packet-drop-percent")?,
                        "--packet-drop-percent",
                    )?;
                }
                "--noise-profile" => {
                    apply_noise_profile(
                        &mut parsed.noise,
                        &next_value(&mut args, "--noise-profile")?,
                    )?;
                }
                "--help" | "-h" => return Err(usage()),
                other => return Err(format!("unknown argument: {other}\n\n{}", usage())),
            }
        }

        if parsed.message.len() > MAX_MESSAGE_BYTES {
            return Err(format!("message length must be <= {MAX_MESSAGE_BYTES}"));
        }

        Ok(parsed)
    }
}

fn main() {
    let args = match Args::parse() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    if let Err(error) = init_log_file(&args) {
        eprintln!("sim log init error: {error}");
    }

    println!(
        "msrt sim duplex threads=2 interval={}ms message_len={} {} log_file={}",
        args.interval_ms,
        args.message.len(),
        noise_config_summary(args.noise),
        args.log_file.display()
    );

    if run(args).is_err() {
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), ()> {
    let start = Instant::now();
    let (host_to_mcu_tx, host_to_mcu_rx) = mpsc::channel();
    let (mcu_to_host_tx, mcu_to_host_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    let host_args = args.clone();
    let host_event_tx = event_tx.clone();
    let host = thread::spawn(move || {
        let mut worker = HostWorker::new(
            host_args,
            start,
            host_to_mcu_tx,
            mcu_to_host_rx,
            host_event_tx,
        );
        worker.run()
    });

    let mcu_args = args.clone();
    let mcu_event_tx = event_tx.clone();
    let mcu = thread::spawn(move || {
        let mut worker = McuWorker::new(
            mcu_args,
            start,
            mcu_to_host_tx,
            host_to_mcu_rx,
            mcu_event_tx,
        );
        worker.run()
    });

    drop(event_tx);

    let mut monitor = Monitor::new(args, event_rx);
    let monitor_result = monitor.run();
    let host_result = host.join().unwrap_or(Err(()));
    let mcu_result = mcu.join().unwrap_or(Err(()));

    monitor_result?;
    host_result?;
    mcu_result
}

struct HostWorker {
    args: Args,
    endpoint: ClientEndpoint,
    start: Instant,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    events: Sender<SimEvent>,
    last_send_ms: u64,
    last_state: PeerState,
    noise: NoiseLcg,
}

impl HostWorker {
    fn new(
        args: Args,
        start: Instant,
        tx: Sender<Vec<u8>>,
        rx: Receiver<Vec<u8>>,
        events: Sender<SimEvent>,
    ) -> Self {
        Self {
            args,
            endpoint: ClientEndpoint::new(test_config(0x484f_5354)),
            start,
            tx,
            rx,
            events,
            last_send_ms: 0,
            last_state: PeerState::Disconnected,
            noise: NoiseLcg::with_seed(0x484f_5354),
        }
    }

    fn run(&mut self) -> Result<(), ()> {
        self.endpoint.connect(0).expect("host connect");

        loop {
            let now_ms = now_ms(self.start);
            if self.args.duration_ms != u64::MAX && now_ms > self.args.duration_ms {
                return Ok(());
            }

            self.log_state(now_ms);
            let mut progressed = self.drain_rx(now_ms);
            progressed |= self.poll_endpoint(now_ms)?;
            progressed |= self.send_application_message(now_ms);

            if !progressed {
                thread::sleep(IDLE_SLEEP);
            }
        }
    }

    fn log_state(&mut self, now_ms: u64) {
        let state = self.endpoint.peer().state();
        if self.last_state != state {
            let _ = self.events.send(SimEvent::State {
                side: Side::Host,
                now_ms,
                state,
            });
            self.last_state = state;
        }
    }

    fn drain_rx(&mut self, now_ms: u64) -> bool {
        let mut progressed = false;
        loop {
            match self.rx.try_recv() {
                Ok(bytes) => {
                    progressed = true;
                    receive_bytes(
                        &mut self.endpoint,
                        now_ms,
                        &bytes,
                        Side::Host,
                        &self.events,
                        &mut self.noise,
                    );
                }
                Err(TryRecvError::Empty) => return progressed,
                Err(TryRecvError::Disconnected) => return progressed,
            }
        }
    }

    fn poll_endpoint(&mut self, now_ms: u64) -> Result<bool, ()> {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match self.endpoint.poll(now_ms, &mut tx_buf).expect("host poll") {
            EndpointPoll::Transmit { bytes, attempts: _ } => {
                let connected = self.endpoint.peer().is_connected();
                let (bytes, stats) =
                    mutate_for_link(&mut self.noise, bytes, connected, self.args.noise);
                let _ = self.events.send(SimEvent::Noise { stats });
                self.tx.send(bytes).map_err(|_| ())?;
                Ok(true)
            }
            EndpointPoll::Message(_) => {
                let _ = self.events.send(SimEvent::Received { side: Side::Host });
                Ok(true)
            }
            EndpointPoll::SendFailed(failed) => {
                let _ = self.events.send(SimEvent::SendFailed {
                    side: Side::Host,
                    now_ms,
                    failed,
                    state: self.endpoint.peer().state(),
                });
                Err(())
            }
            EndpointPoll::Idle => Ok(false),
        }
    }

    fn send_application_message(&mut self, now_ms: u64) -> bool {
        if !self.endpoint.peer().is_connected() {
            return false;
        }

        if now_ms.saturating_sub(self.last_send_ms) < self.args.interval_ms {
            return false;
        }

        if self.endpoint.peer_mut().send(&self.args.message).is_ok() {
            self.last_send_ms = now_ms;
            let _ = self.events.send(SimEvent::Sent { side: Side::Host });
            return true;
        }

        false
    }
}

struct McuWorker {
    args: Args,
    endpoint: PassiveEndpoint,
    start: Instant,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    events: Sender<SimEvent>,
    last_send_ms: u64,
    last_state: PeerState,
    noise: NoiseLcg,
}

impl McuWorker {
    fn new(
        args: Args,
        start: Instant,
        tx: Sender<Vec<u8>>,
        rx: Receiver<Vec<u8>>,
        events: Sender<SimEvent>,
    ) -> Self {
        Self {
            args,
            endpoint: PassiveEndpoint::new(test_config(0x4d43_5520)),
            start,
            tx,
            rx,
            events,
            last_send_ms: 0,
            last_state: PeerState::Disconnected,
            noise: NoiseLcg::with_seed(0x4d43_5520),
        }
    }

    fn run(&mut self) -> Result<(), ()> {
        loop {
            let now_ms = now_ms(self.start);
            if self.args.duration_ms != u64::MAX && now_ms > self.args.duration_ms {
                return Ok(());
            }

            self.log_state(now_ms);
            let mut progressed = self.drain_rx(now_ms);
            progressed |= self.poll_endpoint(now_ms)?;
            progressed |= self.send_application_message(now_ms);

            if !progressed {
                thread::sleep(IDLE_SLEEP);
            }
        }
    }

    fn log_state(&mut self, now_ms: u64) {
        let state = self.endpoint.peer().state();
        if self.last_state != state {
            let _ = self.events.send(SimEvent::State {
                side: Side::Mcu,
                now_ms,
                state,
            });
            self.last_state = state;
        }
    }

    fn drain_rx(&mut self, now_ms: u64) -> bool {
        let mut progressed = false;
        loop {
            match self.rx.try_recv() {
                Ok(bytes) => {
                    progressed = true;
                    receive_bytes(
                        &mut self.endpoint,
                        now_ms,
                        &bytes,
                        Side::Mcu,
                        &self.events,
                        &mut self.noise,
                    );
                }
                Err(TryRecvError::Empty) => return progressed,
                Err(TryRecvError::Disconnected) => return progressed,
            }
        }
    }

    fn poll_endpoint(&mut self, now_ms: u64) -> Result<bool, ()> {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match self.endpoint.poll(now_ms, &mut tx_buf).expect("mcu poll") {
            EndpointPoll::Transmit { bytes, attempts: _ } => {
                let connected = self.endpoint.peer().is_connected();
                let (bytes, stats) =
                    mutate_for_link(&mut self.noise, bytes, connected, self.args.noise);
                let _ = self.events.send(SimEvent::Noise { stats });
                self.tx.send(bytes).map_err(|_| ())?;
                Ok(true)
            }
            EndpointPoll::Message(_) => {
                let _ = self.events.send(SimEvent::Received { side: Side::Mcu });
                Ok(true)
            }
            EndpointPoll::SendFailed(failed) => {
                let _ = self.events.send(SimEvent::SendFailed {
                    side: Side::Mcu,
                    now_ms,
                    failed,
                    state: self.endpoint.peer().state(),
                });
                Err(())
            }
            EndpointPoll::Idle => Ok(false),
        }
    }

    fn send_application_message(&mut self, now_ms: u64) -> bool {
        if !self.endpoint.peer().is_connected() {
            return false;
        }

        if now_ms.saturating_sub(self.last_send_ms) < self.args.interval_ms {
            return false;
        }

        if self.endpoint.send(&self.args.message).is_ok() {
            self.last_send_ms = now_ms;
            let _ = self.events.send(SimEvent::Sent { side: Side::Mcu });
            return true;
        }

        false
    }
}

trait SimEndpoint {
    fn receive(&mut self, now_ms: u64, bytes: &[u8]) -> ReceiveReport;
}

impl SimEndpoint for ClientEndpoint {
    fn receive(&mut self, now_ms: u64, bytes: &[u8]) -> ReceiveReport {
        self.receive(now_ms, bytes)
    }
}

impl SimEndpoint for PassiveEndpoint {
    fn receive(&mut self, now_ms: u64, bytes: &[u8]) -> ReceiveReport {
        self.receive(now_ms, bytes)
    }
}

fn receive_bytes(
    endpoint: &mut impl SimEndpoint,
    now_ms: u64,
    bytes: &[u8],
    side: Side,
    events: &Sender<SimEvent>,
    noise: &mut NoiseLcg,
) {
    let mut index = 0;
    while index < bytes.len() {
        let max = (bytes.len() - index).min(16);
        let chunk_len = 1 + noise.next_byte() as usize % max;
        let report = endpoint.receive(now_ms, &bytes[index..index + chunk_len]);
        if matches!(report, ReceiveReport::Error(_)) {
            let _ = events.send(SimEvent::ReceiveError {
                side,
                now_ms,
                report,
            });
        }
        index += chunk_len;
    }
}

fn mutate_for_link(
    noise: &mut NoiseLcg,
    bytes: &[u8],
    connected: bool,
    config: NoiseConfig,
) -> (Vec<u8>, NoiseStats) {
    if connected {
        mutate_or_copy(noise, bytes, config)
    } else {
        mutate_or_copy(noise, bytes, NoiseConfig::default())
    }
}

struct Monitor {
    args: Args,
    rx: Receiver<SimEvent>,
    host_state: PeerState,
    mcu_state: PeerState,
    host_sent: usize,
    mcu_sent: usize,
    host_received: usize,
    mcu_received: usize,
    noise_stats: NoiseStats,
    start: Instant,
    last_status_at: Instant,
}

impl Monitor {
    fn new(args: Args, rx: Receiver<SimEvent>) -> Self {
        Self {
            args,
            rx,
            host_state: PeerState::Disconnected,
            mcu_state: PeerState::Disconnected,
            host_sent: 0,
            mcu_sent: 0,
            host_received: 0,
            mcu_received: 0,
            noise_stats: NoiseStats::default(),
            start: Instant::now(),
            last_status_at: Instant::now(),
        }
    }

    fn run(&mut self) -> Result<(), ()> {
        loop {
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => self.handle_event(event)?,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            }

            if self.args.duration_ms != u64::MAX && now_ms(self.start) > self.args.duration_ms {
                return Ok(());
            }

            self.record_periodic_status();
        }
    }

    fn handle_event(&mut self, event: SimEvent) -> Result<(), ()> {
        match event {
            SimEvent::State {
                side,
                now_ms,
                state,
            } => {
                match side {
                    Side::Host => self.host_state = state,
                    Side::Mcu => self.mcu_state = state,
                }
                println!("{} state={state:?} now={now_ms}", side.label());
                Ok(())
            }
            SimEvent::Sent { side } => {
                match side {
                    Side::Host => self.host_sent += 1,
                    Side::Mcu => self.mcu_sent += 1,
                }
                Ok(())
            }
            SimEvent::Received { side } => {
                match side {
                    Side::Host => self.host_received += 1,
                    Side::Mcu => self.mcu_received += 1,
                }
                Ok(())
            }
            SimEvent::Noise { stats } => {
                self.noise_stats.add(stats);
                Ok(())
            }
            SimEvent::ReceiveError {
                side,
                now_ms,
                report,
            } => {
                eprintln!(
                    "sim receive_error now={} side={} report={:?}",
                    now_ms,
                    side.label(),
                    report
                );
                Err(())
            }
            SimEvent::SendFailed {
                side,
                now_ms,
                failed,
                state,
            } => {
                let summary = self.failure_summary(side, now_ms, failed, state);
                eprintln!("{summary}");
                if let Err(error) = self.write_log("send_failed", &summary) {
                    eprintln!("sim log write error: {error}");
                } else {
                    eprintln!("sim log written to {}", self.args.log_file.display());
                }
                Err(())
            }
        }
    }

    fn record_periodic_status(&mut self) {
        if self.last_status_at.elapsed() < STATUS_INTERVAL {
            return;
        }

        self.last_status_at = Instant::now();
        let status = self.status_summary();
        println!("{status}");
        if let Err(error) = self.append_status_log(&status) {
            eprintln!("sim status log write error: {error}");
        }
    }

    fn status_summary(&self) -> String {
        format!(
            "sim stats real_elapsed={}s host_state={:?} mcu_state={:?} host_sent={} mcu_sent={} host_received={} mcu_received={} {}",
            self.start.elapsed().as_secs(),
            self.host_state,
            self.mcu_state,
            self.host_sent,
            self.mcu_sent,
            self.host_received,
            self.mcu_received,
            noise_stats_summary(self.noise_stats),
        )
    }

    fn failure_summary(
        &self,
        side: Side,
        now_ms: u64,
        failed: SendFailedEvent,
        state: PeerState,
    ) -> String {
        format!(
            "sim send_failed now={} side={} state={:?} packet_type={:?} msg={} host_state={:?} mcu_state={:?} host_sent={} mcu_sent={} host_received={} mcu_received={} {}",
            now_ms,
            side.label(),
            state,
            failed.packet_type,
            failed.message_id.get(),
            self.host_state,
            self.mcu_state,
            self.host_sent,
            self.mcu_sent,
            self.host_received,
            self.mcu_received,
            noise_stats_summary(self.noise_stats),
        )
    }

    fn append_status_log(&self, status: &str) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.args.log_file)?;
        writeln!(file, "{status}")
    }

    fn write_log(&self, status: &str, summary: &str) -> io::Result<()> {
        if let Some(parent) = self.args.log_file.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::File::create(&self.args.log_file)?;
        writeln!(file, "status={status}")?;
        writeln!(file, "{summary}")?;
        writeln!(
            file,
            "config interval={}ms message_len={} {}",
            self.args.interval_ms,
            self.args.message.len(),
            noise_config_summary(self.args.noise)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Side {
    Host,
    Mcu,
}

impl Side {
    const fn label(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Mcu => "mcu",
        }
    }
}

#[derive(Clone, Debug)]
enum SimEvent {
    State {
        side: Side,
        now_ms: u64,
        state: PeerState,
    },
    Sent {
        side: Side,
    },
    Received {
        side: Side,
    },
    Noise {
        stats: NoiseStats,
    },
    ReceiveError {
        side: Side,
        now_ms: u64,
        report: ReceiveReport,
    },
    SendFailed {
        side: Side,
        now_ms: u64,
        failed: SendFailedEvent,
        state: PeerState,
    },
}

trait NoiseStatsExt {
    fn add(&mut self, other: NoiseStats);
}

impl NoiseStatsExt for NoiseStats {
    fn add(&mut self, other: NoiseStats) {
        self.corrupted += other.corrupted;
        self.dropped += other.dropped;
        self.inserted += other.inserted;
        self.burst_corrupted += other.burst_corrupted;
        self.burst_dropped += other.burst_dropped;
        self.packet_dropped += other.packet_dropped;
    }
}

fn now_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

fn init_log_file(args: &Args) -> io::Result<()> {
    if let Some(parent) = args.log_file.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(&args.log_file)?;
    writeln!(file, "status=running")?;
    writeln!(
        file,
        "config threads=2 interval={}ms message_len={} {}",
        args.interval_ms,
        args.message.len(),
        noise_config_summary(args.noise)
    )
}

fn test_config(seed: u32) -> EngineConfig {
    EngineConfig {
        initial_message_id: msrt::core::MessageId::new(seed),
        fragment_bytes: TEST_FRAGMENT_BYTES,
        ..EngineConfig::default()
    }
}

fn make_message(len: usize) -> Vec<u8> {
    let mut message = Vec::with_capacity(len);

    for index in 0..len {
        message.push(b'a' + (index % 26) as u8);
    }

    message
}

fn noise_config_summary(noise: NoiseConfig) -> String {
    format!(
        "noise={} drop_byte={} insert_byte={} burst_corrupt={} burst_drop={} packet_drop={}",
        noise.corrupt_per_mille,
        noise.drop_byte_per_mille,
        noise.insert_byte_per_mille,
        noise.burst_corrupt_per_mille,
        noise.burst_drop_per_mille,
        noise.packet_drop_per_mille,
    )
}

fn apply_noise_profile(noise: &mut NoiseConfig, profile: &str) -> Result<(), String> {
    noise.corrupt_per_mille = 0;
    noise.drop_byte_per_mille = 0;
    noise.insert_byte_per_mille = 0;
    noise.burst_corrupt_per_mille = 0;
    noise.burst_drop_per_mille = 0;
    noise.packet_drop_per_mille = 0;

    match profile {
        "none" => {}
        "corrupt" => noise.corrupt_per_mille = PROFILE_NOISE_PER_MILLE,
        "drop-byte" => noise.drop_byte_per_mille = PROFILE_NOISE_PER_MILLE,
        "insert-byte" => noise.insert_byte_per_mille = PROFILE_NOISE_PER_MILLE,
        "burst-corrupt" => noise.burst_corrupt_per_mille = PROFILE_NOISE_PER_MILLE,
        "burst-drop" => noise.burst_drop_per_mille = PROFILE_NOISE_PER_MILLE,
        "packet-drop" => noise.packet_drop_per_mille = PROFILE_NOISE_PER_MILLE,
        "byte-random" => {
            noise.corrupt_per_mille = PROFILE_NOISE_PER_MILLE;
            noise.drop_byte_per_mille = PROFILE_NOISE_PER_MILLE;
            noise.insert_byte_per_mille = PROFILE_NOISE_PER_MILLE;
        }
        "all" => {
            noise.corrupt_per_mille = PROFILE_NOISE_PER_MILLE;
            noise.drop_byte_per_mille = PROFILE_NOISE_PER_MILLE;
            noise.insert_byte_per_mille = PROFILE_NOISE_PER_MILLE;
            noise.burst_corrupt_per_mille = PROFILE_NOISE_PER_MILLE;
            noise.burst_drop_per_mille = PROFILE_NOISE_PER_MILLE;
            noise.packet_drop_per_mille = PROFILE_NOISE_PER_MILLE;
        }
        other => {
            return Err(format!(
                "unknown --noise-profile: {other}; expected none, corrupt, drop-byte, insert-byte, burst-corrupt, burst-drop, packet-drop, byte-random, or all"
            ));
        }
    }

    Ok(())
}

fn noise_stats_summary(stats: NoiseStats) -> String {
    format!(
        "corrupted={} dropped={} inserted={} burst_corrupted={} burst_dropped={} packet_dropped={}",
        stats.corrupted,
        stats.dropped,
        stats.inserted,
        stats.burst_corrupted,
        stats.burst_dropped,
        stats.packet_dropped,
    )
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value\n\n{}", usage()))
}

fn usage() -> String {
    "usage: msrt-sim-duplex [--interval-ms N] [--duration-sec N] [--message-size N] [--log-file PATH] [--noise-percent N] [--drop-byte-percent N] [--insert-byte-percent N] [--burst-corrupt-percent N] [--burst-drop-percent N] [--packet-drop-percent N] [--noise-profile none|corrupt|drop-byte|insert-byte|burst-corrupt|burst-drop|packet-drop|byte-random|all]"
        .to_string()
}
