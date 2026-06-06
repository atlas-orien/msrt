use std::{
    env, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
};

mod support;

use msrt::{
    endpoint::{ClientEndpoint, EndpointPoll, PassiveEndpoint, PeerState},
    engine::{EngineConfig, ReceiveReport, SendFailedEvent},
};

use support::noise::{NoiseConfig, NoiseLcg, NoiseStats, mutate_or_copy, parse_percent_per_mille};

const TX_BUF_BYTES: usize = 256;
const MAX_MESSAGE_BYTES: usize = 256;
const DEFAULT_MESSAGE_BYTES: usize = 96;
const TEST_FRAGMENT_BYTES: usize = 48;
const DEFAULT_INTERVAL_MS: u64 = 1;
const DEFAULT_MESSAGES_PER_TICK: usize = 1;
const DEFAULT_DURATION_MS: u64 = 60 * 60 * 1000;
const DEFAULT_NOISE_PER_MILLE: u16 = 1;
const DEFAULT_LOG_FILE: &str = "log/msrt-sim-dynamic.log";
const STATUS_INTERVAL_MS: u64 = 60 * 1000;
const MAX_POLLS_PER_TICK: usize = 256;

#[derive(Clone, Debug)]
struct Args {
    interval_ms: u64,
    duration_ms: Option<u64>,
    messages_per_tick: usize,
    message: Vec<u8>,
    noise: NoiseConfig,
    log_file: PathBuf,
    status_interval_ms: u64,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut parsed = Self {
            interval_ms: DEFAULT_INTERVAL_MS,
            duration_ms: Some(DEFAULT_DURATION_MS),
            messages_per_tick: DEFAULT_MESSAGES_PER_TICK,
            message: make_message(DEFAULT_MESSAGE_BYTES),
            log_file: PathBuf::from(DEFAULT_LOG_FILE),
            status_interval_ms: STATUS_INTERVAL_MS,
            noise: NoiseConfig {
                corrupt_per_mille: DEFAULT_NOISE_PER_MILLE,
                drop_byte_per_mille: DEFAULT_NOISE_PER_MILLE,
                insert_byte_per_mille: DEFAULT_NOISE_PER_MILLE,
                burst_corrupt_per_mille: DEFAULT_NOISE_PER_MILLE,
                burst_drop_per_mille: DEFAULT_NOISE_PER_MILLE,
                packet_drop_per_mille: DEFAULT_NOISE_PER_MILLE,
            },
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--interval-ms" => {
                    parsed.interval_ms = next_value(&mut args, "--interval-ms")?
                        .parse()
                        .map_err(|error| format!("invalid --interval-ms: {error}"))?;
                }
                "--messages-per-tick" => {
                    parsed.messages_per_tick = next_value(&mut args, "--messages-per-tick")?
                        .parse()
                        .map_err(|error| format!("invalid --messages-per-tick: {error}"))?;
                }
                "--duration-ms" => {
                    parsed.duration_ms = Some(
                        next_value(&mut args, "--duration-ms")?
                            .parse()
                            .map_err(|error| format!("invalid --duration-ms: {error}"))?,
                    );
                }
                "--duration-sec" => {
                    let secs: u64 = next_value(&mut args, "--duration-sec")?
                        .parse()
                        .map_err(|error| format!("invalid --duration-sec: {error}"))?;
                    parsed.duration_ms = Some(secs.saturating_mul(1000));
                }
                "--duration-hours" => {
                    let hours: u64 = next_value(&mut args, "--duration-hours")?
                        .parse()
                        .map_err(|error| format!("invalid --duration-hours: {error}"))?;
                    parsed.duration_ms = Some(hours.saturating_mul(60 * 60 * 1000));
                }
                "--forever" => parsed.duration_ms = None,
                "--message-size" => {
                    let len = next_value(&mut args, "--message-size")?
                        .parse()
                        .map_err(|error| format!("invalid --message-size: {error}"))?;
                    parsed.message = make_message(len);
                }
                "--log-file" => {
                    parsed.log_file = PathBuf::from(next_value(&mut args, "--log-file")?);
                }
                "--status-interval-sec" => {
                    let secs: u64 = next_value(&mut args, "--status-interval-sec")?
                        .parse()
                        .map_err(|error| format!("invalid --status-interval-sec: {error}"))?;
                    parsed.status_interval_ms = secs.saturating_mul(1000);
                }
                "--noise-percent" => {
                    let value = parse_percent_per_mille(
                        next_value(&mut args, "--noise-percent")?,
                        "--noise-percent",
                    )?;
                    parsed.noise.corrupt_per_mille = value;
                    parsed.noise.drop_byte_per_mille = value;
                    parsed.noise.insert_byte_per_mille = value;
                    parsed.noise.burst_corrupt_per_mille = value;
                    parsed.noise.burst_drop_per_mille = value;
                    parsed.noise.packet_drop_per_mille = value;
                }
                "--help" | "-h" => return Err(usage()),
                other => return Err(format!("unknown argument: {other}\n\n{}", usage())),
            }
        }

        if parsed.interval_ms == 0 {
            return Err("--interval-ms must be > 0".to_string());
        }
        if parsed.messages_per_tick == 0 {
            return Err("--messages-per-tick must be > 0".to_string());
        }
        if parsed.status_interval_ms == 0 {
            return Err("--status-interval-sec must be > 0".to_string());
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

    if let Err(error) = run(args) {
        eprintln!("sim dynamic error: {error}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    init_log_file(&args).map_err(|error| format!("log init failed: {error}"))?;
    println!(
        "msrt sim dynamic recovery={} interval={}ms duration={} message_len={} messages_per_tick={} {} log_file={}",
        recovery_label(),
        args.interval_ms,
        duration_label(args.duration_ms),
        args.message.len(),
        args.messages_per_tick,
        noise_config_summary(args.noise),
        args.log_file.display()
    );
    io::stdout()
        .flush()
        .map_err(|error| format!("stdout flush failed: {error}"))?;

    let mut sim = DynamicSim::new(args);
    sim.run()
}

struct DynamicSim {
    args: Args,
    host: ClientEndpoint,
    mcu: PassiveEndpoint,
    host_to_mcu: DynamicLink,
    mcu_to_host: DynamicLink,
    host_last_send_ms: u64,
    mcu_last_send_ms: u64,
    host_sent: usize,
    mcu_sent: usize,
    host_received: usize,
    mcu_received: usize,
    stats: NoiseStats,
    link_dropped: usize,
    max_host_queue: usize,
    max_mcu_queue: usize,
    next_status_ms: u64,
    last_status_ms: Option<u64>,
}

impl DynamicSim {
    fn new(args: Args) -> Self {
        let next_status_ms = args.status_interval_ms;

        Self {
            args,
            host: ClientEndpoint::new(test_config(0x4459_484f)),
            mcu: PassiveEndpoint::new(test_config(0x4459_4d43)),
            host_to_mcu: DynamicLink::new(0x484f_5354),
            mcu_to_host: DynamicLink::new(0x4d43_5520),
            host_last_send_ms: 0,
            mcu_last_send_ms: 0,
            host_sent: 0,
            mcu_sent: 0,
            host_received: 0,
            mcu_received: 0,
            stats: NoiseStats::default(),
            link_dropped: 0,
            max_host_queue: 0,
            max_mcu_queue: 0,
            next_status_ms,
            last_status_ms: None,
        }
    }

    fn run(&mut self) -> Result<(), String> {
        self.host
            .connect(0)
            .map_err(|error| format!("host connect failed: {error:?}"))?;

        let mut now_ms = 0;
        loop {
            self.tick(now_ms)?;

            if now_ms >= self.next_status_ms {
                self.write_status(now_ms)
                    .map_err(|error| error.to_string())?;
                self.next_status_ms = self
                    .next_status_ms
                    .saturating_add(self.args.status_interval_ms);
            }

            if matches!(self.args.duration_ms, Some(duration_ms) if now_ms >= duration_ms) {
                break;
            }

            now_ms = now_ms.saturating_add(1);
        }

        if self.last_status_ms != Some(now_ms) {
            self.write_status(now_ms)
                .map_err(|error| error.to_string())?;
        }
        println!("sim dynamic completed");
        Ok(())
    }

    fn tick(&mut self, now_ms: u64) -> Result<(), String> {
        self.deliver_host_to_mcu(now_ms)?;
        self.deliver_mcu_to_host(now_ms)?;
        self.poll_host(now_ms)?;
        self.poll_mcu(now_ms)?;
        self.send_messages(now_ms);
        self.poll_host(now_ms)?;
        self.poll_mcu(now_ms)
    }

    fn deliver_host_to_mcu(&mut self, now_ms: u64) -> Result<(), String> {
        for bytes in self.host_to_mcu.pop_ready(now_ms) {
            receive_bytes(&mut self.mcu, now_ms, &bytes, Side::Mcu)?;
        }
        Ok(())
    }

    fn deliver_mcu_to_host(&mut self, now_ms: u64) -> Result<(), String> {
        for bytes in self.mcu_to_host.pop_ready(now_ms) {
            receive_bytes(&mut self.host, now_ms, &bytes, Side::Host)?;
        }
        Ok(())
    }

    fn poll_host(&mut self, now_ms: u64) -> Result<(), String> {
        for _ in 0..MAX_POLLS_PER_TICK {
            let mut tx_buf = [0; TX_BUF_BYTES];
            match self
                .host
                .poll(now_ms, &mut tx_buf)
                .map_err(|error| format!("host poll failed: {error:?}"))?
            {
                EndpointPoll::Transmit { bytes, .. } => {
                    self.enqueue_link(now_ms, Side::Host, bytes.to_vec());
                }
                EndpointPoll::Message(_) => self.host_received += 1,
                EndpointPoll::SendFailed(failed) => {
                    return self.fail(now_ms, Side::Host, failed, self.host.peer().state());
                }
                EndpointPoll::Idle => return Ok(()),
            }
        }

        Err(format!(
            "host exceeded {MAX_POLLS_PER_TICK} polls at now={now_ms}"
        ))
    }

    fn poll_mcu(&mut self, now_ms: u64) -> Result<(), String> {
        for _ in 0..MAX_POLLS_PER_TICK {
            let mut tx_buf = [0; TX_BUF_BYTES];
            match self
                .mcu
                .poll(now_ms, &mut tx_buf)
                .map_err(|error| format!("mcu poll failed: {error:?}"))?
            {
                EndpointPoll::Transmit { bytes, .. } => {
                    self.enqueue_link(now_ms, Side::Mcu, bytes.to_vec());
                }
                EndpointPoll::Message(_) => self.mcu_received += 1,
                EndpointPoll::SendFailed(failed) => {
                    return self.fail(now_ms, Side::Mcu, failed, self.mcu.peer().state());
                }
                EndpointPoll::Idle => return Ok(()),
            }
        }

        Err(format!(
            "mcu exceeded {MAX_POLLS_PER_TICK} polls at now={now_ms}"
        ))
    }

    fn enqueue_link(&mut self, now_ms: u64, side: Side, bytes: Vec<u8>) {
        let connected = match side {
            Side::Host => self.host.peer().is_connected(),
            Side::Mcu => self.mcu.peer().is_connected(),
        };
        let noise = if connected {
            self.args.noise
        } else {
            NoiseConfig::default()
        };

        match side {
            Side::Host => {
                let result = self.host_to_mcu.enqueue(now_ms, bytes, noise);
                self.stats.add(result.noise);
                self.link_dropped += result.link_dropped;
                self.max_host_queue = self.max_host_queue.max(self.host_to_mcu.len());
            }
            Side::Mcu => {
                let result = self.mcu_to_host.enqueue(now_ms, bytes, noise);
                self.stats.add(result.noise);
                self.link_dropped += result.link_dropped;
                self.max_mcu_queue = self.max_mcu_queue.max(self.mcu_to_host.len());
            }
        }
    }

    fn send_messages(&mut self, now_ms: u64) {
        if self.host.peer().is_connected()
            && now_ms.saturating_sub(self.host_last_send_ms) >= self.args.interval_ms
        {
            let mut sent = 0;
            for _ in 0..self.args.messages_per_tick {
                if self.host.peer_mut().send(&self.args.message).is_err() {
                    break;
                }
                sent += 1;
            }
            if sent > 0 {
                self.host_last_send_ms = now_ms;
                self.host_sent += sent;
            }
        }

        if self.mcu.peer().is_connected()
            && now_ms.saturating_sub(self.mcu_last_send_ms) >= self.args.interval_ms
        {
            let mut sent = 0;
            for _ in 0..self.args.messages_per_tick {
                if !matches!(self.mcu.send(&self.args.message), Ok(Some(_))) {
                    break;
                }
                sent += 1;
            }
            if sent > 0 {
                self.mcu_last_send_ms = now_ms;
                self.mcu_sent += sent;
            }
        }
    }

    fn fail(
        &mut self,
        now_ms: u64,
        side: Side,
        failed: SendFailedEvent,
        state: PeerState,
    ) -> Result<(), String> {
        let summary = self.summary(now_ms, Some((side, failed, state)));
        eprintln!("{summary}");
        self.write_failure(&summary)
            .map_err(|error| format!("failed to write log: {error}"))?;
        Err(summary)
    }

    fn write_status(&mut self, now_ms: u64) -> io::Result<()> {
        let summary = self.summary(now_ms, None);
        println!("{summary}");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.args.log_file)?;
        writeln!(file, "{summary}")?;
        self.last_status_ms = Some(now_ms);
        Ok(())
    }

    fn write_failure(&self, summary: &str) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.args.log_file)?;
        writeln!(file, "status=failed")?;
        writeln!(file, "{summary}")
    }

    fn summary(&self, now_ms: u64, failed: Option<(Side, SendFailedEvent, PeerState)>) -> String {
        let failure = failed
            .map(|(side, event, state)| {
                format!(
                    " failed_side={} failed_state={:?} failed_type={:?} failed_msg={}",
                    side.label(),
                    state,
                    event.packet_type,
                    event.message_id.get()
                )
            })
            .unwrap_or_default();
        let profile = DynamicProfile::at(now_ms);

        format!(
            "sim dynamic now={}s profile={} base_delay={} jitter={} link_drop={} host_state={:?} mcu_state={:?} host_sent={} mcu_sent={} host_received={} mcu_received={} host_queue={} mcu_queue={} max_host_queue={} max_mcu_queue={} link_dropped={} {}{}",
            now_ms / 1000,
            profile.name,
            profile.base_delay_ms,
            profile.jitter_ms,
            profile.packet_drop_per_mille,
            self.host.peer().state(),
            self.mcu.peer().state(),
            self.host_sent,
            self.mcu_sent,
            self.host_received,
            self.mcu_received,
            self.host_to_mcu.len(),
            self.mcu_to_host.len(),
            self.max_host_queue,
            self.max_mcu_queue,
            self.link_dropped,
            noise_stats_summary(self.stats),
            failure,
        )
    }
}

#[derive(Clone, Debug)]
struct DynamicLink {
    packets: Vec<PendingPacket>,
    noise: NoiseLcg,
}

impl DynamicLink {
    fn new(seed: u32) -> Self {
        Self {
            packets: Vec::new(),
            noise: NoiseLcg::with_seed(seed),
        }
    }

    fn len(&self) -> usize {
        self.packets.len()
    }

    fn enqueue(&mut self, now_ms: u64, bytes: Vec<u8>, noise: NoiseConfig) -> EnqueueResult {
        let profile = DynamicProfile::at(now_ms);
        let mut result = EnqueueResult::default();

        if self.roll_per_mille(profile.packet_drop_per_mille) {
            result.link_dropped = 1;
            return result;
        }

        let (bytes, stats) = mutate_or_copy(&mut self.noise, &bytes, noise);
        result.noise = stats;
        if bytes.is_empty() {
            return result;
        }

        let jitter = if profile.jitter_ms == 0 {
            0
        } else {
            self.next_u64() % (profile.jitter_ms + 1)
        };
        let deliver_at_ms = now_ms
            .saturating_add(profile.base_delay_ms)
            .saturating_add(jitter);

        self.packets.push(PendingPacket {
            deliver_at_ms,
            bytes,
        });

        result
    }

    fn pop_ready(&mut self, now_ms: u64) -> Vec<Vec<u8>> {
        let mut ready = Vec::new();
        let mut index = 0;

        while index < self.packets.len() {
            if self.packets[index].deliver_at_ms <= now_ms {
                ready.push(self.packets.swap_remove(index).bytes);
            } else {
                index += 1;
            }
        }

        ready
    }

    fn roll_per_mille(&mut self, threshold: u16) -> bool {
        threshold != 0 && self.next_u16() % 1000 < threshold
    }

    fn next_u16(&mut self) -> u16 {
        u16::from(self.noise.next_byte()) << 8 | u16::from(self.noise.next_byte())
    }

    fn next_u64(&mut self) -> u64 {
        u64::from(self.noise.next_byte()) << 24
            | u64::from(self.noise.next_byte()) << 16
            | u64::from(self.noise.next_byte()) << 8
            | u64::from(self.noise.next_byte())
    }
}

#[derive(Clone, Debug)]
struct PendingPacket {
    deliver_at_ms: u64,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
struct EnqueueResult {
    noise: NoiseStats,
    link_dropped: usize,
}

#[derive(Clone, Copy, Debug)]
struct DynamicProfile {
    name: &'static str,
    base_delay_ms: u64,
    jitter_ms: u64,
    packet_drop_per_mille: u16,
}

impl DynamicProfile {
    fn at(now_ms: u64) -> Self {
        match (now_ms / 15_000) % 4 {
            0 => Self {
                name: "fast",
                base_delay_ms: 2,
                jitter_ms: 2,
                packet_drop_per_mille: 0,
            },
            1 => Self {
                name: "slow",
                base_delay_ms: 80,
                jitter_ms: 80,
                packet_drop_per_mille: 1,
            },
            2 => Self {
                name: "jitter",
                base_delay_ms: 20,
                jitter_ms: 220,
                packet_drop_per_mille: 1,
            },
            _ => Self {
                name: "congested",
                base_delay_ms: 180,
                jitter_ms: 320,
                packet_drop_per_mille: 3,
            },
        }
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
) -> Result<(), String> {
    let report = endpoint.receive(now_ms, bytes);
    if matches!(report, ReceiveReport::Error(_)) {
        eprintln!(
            "sim dynamic receive_error now={} side={} report={:?}",
            now_ms,
            side.label(),
            report
        );
    }

    Ok(())
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
        "config dynamic recovery={} interval={}ms duration={} status_interval={}ms message_len={} messages_per_tick={} {}",
        recovery_label(),
        args.interval_ms,
        duration_label(args.duration_ms),
        args.status_interval_ms,
        args.message.len(),
        args.messages_per_tick,
        noise_config_summary(args.noise)
    )
}

fn test_config(seed: u32) -> EngineConfig {
    EngineConfig {
        initial_message_id: msrt::core::MessageId::new(seed),
        fragment_bytes: TEST_FRAGMENT_BYTES,
        max_retransmit_attempts: 5,
        retransmit_timeout_ms: 150,
        #[cfg(feature = "dynamic-recovery")]
        dynamic_recovery: msrt::reliability::DynamicRecoveryConfig {
            initial_rtt_ms: 100,
            max_ack_delay_ms: 10,
            timer_granularity_ms: 1,
            max_backoff_exponent: 8,
        },
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

fn duration_label(duration_ms: Option<u64>) -> String {
    duration_ms
        .map(|duration_ms| format!("{duration_ms}ms"))
        .unwrap_or_else(|| "forever".to_string())
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

fn recovery_label() -> &'static str {
    if cfg!(feature = "dynamic-recovery") {
        "dynamic"
    } else {
        "fixed"
    }
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value\n\n{}", usage()))
}

fn usage() -> String {
    "usage: msrt-sim-dynamic [--interval-ms N] [--messages-per-tick N] [--duration-ms N] [--duration-sec N] [--duration-hours N] [--forever] [--message-size N] [--log-file PATH] [--status-interval-sec N] [--noise-percent N]"
        .to_string()
}
