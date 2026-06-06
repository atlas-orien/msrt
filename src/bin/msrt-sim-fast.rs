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
const DEFAULT_MESSAGE_BYTES: usize = 240;
const TEST_FRAGMENT_BYTES: usize = 48;
const DEFAULT_INTERVAL_MS: u64 = 1;
const DEFAULT_MESSAGES_PER_TICK: usize = 10;
const DEFAULT_NOISE_PER_MILLE: u16 = 1;
const DEFAULT_LOG_FILE: &str = "log/msrt-sim-fast.log";
const STATUS_INTERVAL_MS: u64 = 60 * 60 * 1000;
const MAX_POLLS_PER_TICK: usize = 256;

#[derive(Clone, Debug)]
struct Args {
    interval_ms: u64,
    duration_ms: Option<u64>,
    messages_per_tick: usize,
    message: Vec<u8>,
    noise: NoiseConfig,
    log_file: PathBuf,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut parsed = Self {
            interval_ms: DEFAULT_INTERVAL_MS,
            duration_ms: None,
            messages_per_tick: DEFAULT_MESSAGES_PER_TICK,
            message: make_message(DEFAULT_MESSAGE_BYTES),
            log_file: PathBuf::from(DEFAULT_LOG_FILE),
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
                "--forever" => {
                    parsed.duration_ms = None;
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
                "--corrupt-percent" => {
                    parsed.noise.corrupt_per_mille = parse_percent_per_mille(
                        next_value(&mut args, "--corrupt-percent")?,
                        "--corrupt-percent",
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
        eprintln!("sim fast error: {error}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    init_log_file(&args).map_err(|error| format!("log init failed: {error}"))?;
    println!(
        "msrt sim fast interval={}ms duration={} message_len={} messages_per_tick={} {} log_file={}",
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

    let mut sim = FastSim::new(args);
    sim.run()
}

struct FastSim {
    args: Args,
    host: ClientEndpoint,
    mcu: PassiveEndpoint,
    host_to_mcu: Vec<Vec<u8>>,
    mcu_to_host: Vec<Vec<u8>>,
    host_noise: NoiseLcg,
    mcu_noise: NoiseLcg,
    host_last_send_ms: u64,
    mcu_last_send_ms: u64,
    host_sent: usize,
    mcu_sent: usize,
    host_received: usize,
    mcu_received: usize,
    stats: NoiseStats,
    next_status_ms: u64,
}

impl FastSim {
    fn new(args: Args) -> Self {
        Self {
            args,
            host: ClientEndpoint::new(test_config(0x4641_5354)),
            mcu: PassiveEndpoint::new(test_config(0x4d43_5520)),
            host_to_mcu: Vec::new(),
            mcu_to_host: Vec::new(),
            host_noise: NoiseLcg::with_seed(0x484f_5354),
            mcu_noise: NoiseLcg::with_seed(0x4d43_5520),
            host_last_send_ms: 0,
            mcu_last_send_ms: 0,
            host_sent: 0,
            mcu_sent: 0,
            host_received: 0,
            mcu_received: 0,
            stats: NoiseStats::default(),
            next_status_ms: STATUS_INTERVAL_MS,
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
                self.next_status_ms = self.next_status_ms.saturating_add(STATUS_INTERVAL_MS);
            }

            if matches!(self.args.duration_ms, Some(duration_ms) if now_ms >= duration_ms) {
                break;
            }

            now_ms = now_ms.saturating_add(1);
        }

        self.write_status(now_ms)
            .map_err(|error| error.to_string())?;
        println!("sim fast completed");
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
        for bytes in core::mem::take(&mut self.host_to_mcu) {
            receive_bytes(
                &mut self.mcu,
                now_ms,
                &bytes,
                Side::Mcu,
                &mut self.mcu_noise,
            )?;
        }
        Ok(())
    }

    fn deliver_mcu_to_host(&mut self, now_ms: u64) -> Result<(), String> {
        for bytes in core::mem::take(&mut self.mcu_to_host) {
            receive_bytes(
                &mut self.host,
                now_ms,
                &bytes,
                Side::Host,
                &mut self.host_noise,
            )?;
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
                    let connected = self.host.peer().is_connected();
                    let (bytes, stats) =
                        mutate_for_link(&mut self.host_noise, bytes, connected, self.args.noise);
                    self.stats.add(stats);
                    if !bytes.is_empty() {
                        self.host_to_mcu.push(bytes);
                    }
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
                    let connected = self.mcu.peer().is_connected();
                    let (bytes, stats) =
                        mutate_for_link(&mut self.mcu_noise, bytes, connected, self.args.noise);
                    self.stats.add(stats);
                    if !bytes.is_empty() {
                        self.mcu_to_host.push(bytes);
                    }
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

    fn write_status(&self, now_ms: u64) -> io::Result<()> {
        let summary = self.summary(now_ms, None);
        println!("{summary}");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.args.log_file)?;
        writeln!(file, "{summary}")
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

        format!(
            "sim fast now={}s host_state={:?} mcu_state={:?} host_sent={} mcu_sent={} host_received={} mcu_received={} {}{}",
            now_ms / 1000,
            self.host.peer().state(),
            self.mcu.peer().state(),
            self.host_sent,
            self.mcu_sent,
            self.host_received,
            self.mcu_received,
            noise_stats_summary(self.stats),
            failure,
        )
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
    noise: &mut NoiseLcg,
) -> Result<(), String> {
    let mut index = 0;
    while index < bytes.len() {
        let max = (bytes.len() - index).min(16);
        let chunk_len = 1 + noise.next_byte() as usize % max;
        let report = endpoint.receive(now_ms, &bytes[index..index + chunk_len]);
        if matches!(report, ReceiveReport::Error(_)) {
            eprintln!(
                "sim fast receive_error now={} side={} report={:?}",
                now_ms,
                side.label(),
                report
            );
        }
        index += chunk_len;
    }

    Ok(())
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
        "config fast interval={}ms duration={} message_len={} messages_per_tick={} {}",
        args.interval_ms,
        duration_label(args.duration_ms),
        args.message.len(),
        args.messages_per_tick,
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

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value\n\n{}", usage()))
}

fn usage() -> String {
    "usage: msrt-sim-fast [--interval-ms N] [--messages-per-tick N] [--duration-ms N] [--duration-sec N] [--duration-hours N] [--forever] [--message-size N] [--log-file PATH] [--noise-percent N] [--corrupt-percent N] [--drop-byte-percent N] [--insert-byte-percent N] [--burst-corrupt-percent N] [--burst-drop-percent N] [--packet-drop-percent N]"
        .to_string()
}
