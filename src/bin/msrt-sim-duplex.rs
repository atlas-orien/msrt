use std::{
    env, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
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
const DEFAULT_CORRUPT_PER_MILLE: u16 = 30;
const DEFAULT_DROP_BYTE_PER_MILLE: u16 = 30;
const DEFAULT_INSERT_BYTE_PER_MILLE: u16 = 30;
const DEFAULT_BURST_CORRUPT_PER_MILLE: u16 = 5;
const DEFAULT_BURST_DROP_PER_MILLE: u16 = 5;
const DEFAULT_PACKET_DROP_PER_MILLE: u16 = 5;
const EVENT_LOG_LEN: usize = 512;
const DEFAULT_LOG_FILE: &str = "log/msrt-sim-duplex.log";
const STATUS_INTERVAL: Duration = Duration::from_secs(60);

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

    let mut sim = Sim::new(args);
    if let Err(()) = sim.run() {
        std::process::exit(1);
    }
}

struct Sim {
    args: Args,
    host: ClientEndpoint,
    mcu: PassiveEndpoint,
    host_last_state: PeerState,
    mcu_last_state: PeerState,
    host_last_send_ms: u64,
    mcu_last_send_ms: u64,
    host_sent: usize,
    mcu_sent: usize,
    host_received: usize,
    mcu_received: usize,
    last_status_at: Instant,
    noise: NoiseLcg,
    noise_stats: NoiseStats,
    log: EventLog,
}

impl Sim {
    fn new(args: Args) -> Self {
        Self {
            args,
            host: ClientEndpoint::new(test_config(0x484f_5354)),
            mcu: PassiveEndpoint::new(test_config(0x4d43_5520)),
            host_last_state: PeerState::Disconnected,
            mcu_last_state: PeerState::Disconnected,
            host_last_send_ms: 0,
            mcu_last_send_ms: 0,
            host_sent: 0,
            mcu_sent: 0,
            host_received: 0,
            mcu_received: 0,
            last_status_at: Instant::now(),
            noise: NoiseLcg::with_seed(0x5349_4d31),
            noise_stats: NoiseStats::default(),
            log: EventLog::new(),
        }
    }

    fn run(&mut self) -> Result<(), ()> {
        if let Err(error) = self.init_log_file() {
            eprintln!("sim log init error: {error}");
        }

        println!(
            "msrt sim duplex interval={}ms message_len={} {} log_file={}",
            self.args.interval_ms,
            self.args.message.len(),
            noise_config_summary(self.args.noise),
            self.args.log_file.display()
        );

        self.host.connect(0).expect("host connect");

        let mut now_ms = 0;
        while now_ms <= self.args.duration_ms {
            self.log_state_changes(now_ms);
            self.pump(now_ms)?;
            self.send_application_messages(now_ms);
            self.pump(now_ms)?;
            self.record_periodic_status(now_ms);
            now_ms = now_ms.saturating_add(1);
        }

        let summary = format!(
            "sim complete elapsed={}ms host_sent={} mcu_sent={} host_received={} mcu_received={} {}",
            self.args.duration_ms,
            self.host_sent,
            self.mcu_sent,
            self.host_received,
            self.mcu_received,
            noise_stats_summary(self.noise_stats),
        );
        println!("{summary}");
        if let Err(error) = self.write_log("complete", &summary) {
            eprintln!("sim log write error: {error}");
        }
        Ok(())
    }

    fn init_log_file(&self) -> io::Result<()> {
        if let Some(parent) = self.args.log_file.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::File::create(&self.args.log_file)?;
        writeln!(file, "status=running")?;
        writeln!(
            file,
            "config interval={}ms message_len={} {}",
            self.args.interval_ms,
            self.args.message.len(),
            noise_config_summary(self.args.noise)
        )
    }

    fn pump(&mut self, now_ms: u64) -> Result<(), ()> {
        for _ in 0..512 {
            let mut progressed = false;
            progressed |= self.poll_host(now_ms)?;
            progressed |= self.poll_mcu(now_ms)?;

            if !progressed {
                return Ok(());
            }
        }

        self.log.push(now_ms, "sim", "pump limit reached");
        Ok(())
    }

    fn poll_host(&mut self, now_ms: u64) -> Result<bool, ()> {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match self.host.poll(now_ms, &mut tx_buf).expect("host poll") {
            EndpointPoll::Transmit { bytes, attempts } => {
                let packet = PacketInfo::from_wire(bytes);
                self.log.push(
                    now_ms,
                    "host->mcu",
                    Event::tx(packet, attempts, self.host.peer().state()),
                );
                let connected = self.host.peer().is_connected() && self.mcu.peer().is_connected();
                let bytes = self.mutate_if_connected(bytes, connected);
                self.receive_mcu(now_ms, &bytes);
                Ok(true)
            }
            EndpointPoll::Message(message) => {
                self.host_received += 1;
                self.log.push(
                    now_ms,
                    "host",
                    format!(
                        "message ch={} len={}",
                        message.channel_id.get(),
                        message.as_bytes().len()
                    ),
                );
                Ok(true)
            }
            EndpointPoll::SendFailed(failed) => {
                self.log_failure(now_ms, "host", failed);
                Err(())
            }
            EndpointPoll::Idle => Ok(false),
        }
    }

    fn poll_mcu(&mut self, now_ms: u64) -> Result<bool, ()> {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match self.mcu.poll(now_ms, &mut tx_buf).expect("mcu poll") {
            EndpointPoll::Transmit { bytes, attempts } => {
                let packet = PacketInfo::from_wire(bytes);
                self.log.push(
                    now_ms,
                    "mcu->host",
                    Event::tx(packet, attempts, self.mcu.peer().state()),
                );
                let connected = self.host.peer().is_connected() && self.mcu.peer().is_connected();
                let bytes = self.mutate_if_connected(bytes, connected);
                self.receive_host(now_ms, &bytes);
                Ok(true)
            }
            EndpointPoll::Message(message) => {
                self.mcu_received += 1;
                self.log.push(
                    now_ms,
                    "mcu",
                    format!(
                        "message ch={} len={}",
                        message.channel_id.get(),
                        message.as_bytes().len()
                    ),
                );
                Ok(true)
            }
            EndpointPoll::SendFailed(failed) => {
                self.log_failure(now_ms, "mcu", failed);
                Err(())
            }
            EndpointPoll::Idle => Ok(false),
        }
    }

    fn mutate_if_connected(&mut self, bytes: &[u8], connected: bool) -> Vec<u8> {
        let noise = if connected {
            self.args.noise
        } else {
            NoiseConfig::default()
        };
        let (bytes, stats) = mutate_or_copy(&mut self.noise, bytes, noise);
        self.noise_stats.corrupted += stats.corrupted;
        self.noise_stats.dropped += stats.dropped;
        self.noise_stats.inserted += stats.inserted;
        self.noise_stats.burst_corrupted += stats.burst_corrupted;
        self.noise_stats.burst_dropped += stats.burst_dropped;
        self.noise_stats.packet_dropped += stats.packet_dropped;
        bytes
    }

    fn receive_host(&mut self, now_ms: u64, bytes: &[u8]) {
        let mut index = 0;
        while index < bytes.len() {
            let chunk_len = self.receive_chunk_len(bytes.len() - index);
            let report = self.host.receive(now_ms, &bytes[index..index + chunk_len]);
            self.log_receive(now_ms, "host<-mcu", report);
            index += chunk_len;
        }
    }

    fn receive_mcu(&mut self, now_ms: u64, bytes: &[u8]) {
        let mut index = 0;
        while index < bytes.len() {
            let chunk_len = self.receive_chunk_len(bytes.len() - index);
            let report = self.mcu.receive(now_ms, &bytes[index..index + chunk_len]);
            self.log_receive(now_ms, "mcu<-host", report);
            index += chunk_len;
        }
    }

    fn receive_chunk_len(&mut self, remaining: usize) -> usize {
        let max = remaining.min(16);
        1 + self.noise.next_byte() as usize % max
    }

    fn log_receive(&mut self, now_ms: u64, side: &'static str, report: ReceiveReport) {
        match report {
            ReceiveReport::Packet { packet_number } => {
                self.log.push(
                    now_ms,
                    side,
                    format!("rx packet pn={}", packet_number.get()),
                );
            }
            ReceiveReport::Duplicate { packet_number } => {
                self.log.push(
                    now_ms,
                    side,
                    format!("rx duplicate pn={}", packet_number.get()),
                );
            }
            ReceiveReport::Ack { packet_number } => {
                self.log.push(
                    now_ms,
                    side,
                    format!("rx ack largest={}", packet_number.get()),
                );
            }
            ReceiveReport::Ping {
                packet_number,
                message_id,
            } => {
                self.log.push(
                    now_ms,
                    side,
                    format!(
                        "rx ping pn={} msg={}",
                        packet_number.get(),
                        message_id.get()
                    ),
                );
            }
            ReceiveReport::Pong {
                packet_number,
                message_id,
            } => {
                self.log.push(
                    now_ms,
                    side,
                    format!(
                        "rx pong pn={} msg={}",
                        packet_number.get(),
                        message_id.get()
                    ),
                );
            }
            ReceiveReport::Corrupted => {
                self.log.push(now_ms, side, "rx corrupted");
            }
            ReceiveReport::Error(error) => {
                self.log.push(now_ms, side, format!("rx error {error:?}"));
            }
            ReceiveReport::Noise { .. } | ReceiveReport::Incomplete { .. } => {}
        }
    }

    fn send_application_messages(&mut self, now_ms: u64) {
        if !self.host.peer().is_connected() || !self.mcu.peer().is_connected() {
            return;
        }

        if now_ms.saturating_sub(self.host_last_send_ms) >= self.args.interval_ms
            && self.host.peer_mut().send(&self.args.message).is_ok()
        {
            self.host_sent += 1;
            self.host_last_send_ms = now_ms;
            self.log.push(now_ms, "host", "send app");
        }

        if now_ms.saturating_sub(self.mcu_last_send_ms) >= self.args.interval_ms
            && self.mcu.send(&self.args.message).is_ok()
        {
            self.mcu_sent += 1;
            self.mcu_last_send_ms = now_ms;
            self.log.push(now_ms, "mcu", "send app");
        }
    }

    fn log_state_changes(&mut self, now_ms: u64) {
        let host_state = self.host.peer().state();
        if self.host_last_state != host_state {
            self.log.push(
                now_ms,
                "host",
                format!("state {:?}->{:?}", self.host_last_state, host_state),
            );
            println!("host state={host_state:?} now={now_ms}");
            self.host_last_state = host_state;
        }

        let mcu_state = self.mcu.peer().state();
        if self.mcu_last_state != mcu_state {
            self.log.push(
                now_ms,
                "mcu",
                format!("state {:?}->{:?}", self.mcu_last_state, mcu_state),
            );
            println!("mcu state={mcu_state:?} now={now_ms}");
            self.mcu_last_state = mcu_state;
        }
    }

    fn record_periodic_status(&mut self, now_ms: u64) {
        if now_ms == 0 || self.last_status_at.elapsed() < STATUS_INTERVAL {
            return;
        }

        self.last_status_at = Instant::now();
        let status = format!(
            "sim stats elapsed={}s host_state={:?} mcu_state={:?} host_sent={} mcu_sent={} host_received={} mcu_received={} {}",
            now_ms / 1000,
            self.host.peer().state(),
            self.mcu.peer().state(),
            self.host_sent,
            self.mcu_sent,
            self.host_received,
            self.mcu_received,
            noise_stats_summary(self.noise_stats),
        );
        println!("{status}");
        self.log.push(now_ms, "sim", status.clone());
        if let Err(error) = self.append_status_log(&status) {
            eprintln!("sim status log write error: {error}");
        }
    }

    fn append_status_log(&self, status: &str) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.args.log_file)?;
        writeln!(file, "{status}")
    }

    fn log_failure(&mut self, now_ms: u64, side: &'static str, failed: SendFailedEvent) {
        let summary = format!(
            "sim send_failed now={} side={} ch={} msg={} host_state={:?} mcu_state={:?} host_sent={} mcu_sent={} host_received={} mcu_received={} {}",
            now_ms,
            side,
            failed.channel_id.get(),
            failed.message_id.get(),
            self.host.peer().state(),
            self.mcu.peer().state(),
            self.host_sent,
            self.mcu_sent,
            self.host_received,
            self.mcu_received,
            noise_stats_summary(self.noise_stats),
        );
        eprintln!("{summary}");
        if let Err(error) = self.write_log("send_failed", &summary) {
            eprintln!("sim log write error: {error}");
            self.log.dump_stderr();
        } else {
            eprintln!("sim log written to {}", self.args.log_file.display());
        }
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
        )?;
        self.log.dump_to(&mut file)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PacketInfo {
    packet_type: u8,
    packet_number: u32,
    channel_id: u8,
    message_id: u32,
    len: usize,
}

impl PacketInfo {
    fn from_wire(bytes: &[u8]) -> Self {
        let packet = if bytes.len() >= msrt::wire::WIRE_HEADER_LEN + 16 {
            &bytes[msrt::wire::WIRE_HEADER_LEN..]
        } else {
            &[]
        };
        let packet_type = packet.first().copied().unwrap_or_default();
        let packet_number = packet
            .get(2..6)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .unwrap_or_default();
        let channel_id = packet.get(6).copied().unwrap_or_default();
        let message_id = packet
            .get(7..11)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .unwrap_or_default();

        Self {
            packet_type,
            packet_number,
            channel_id,
            message_id,
            len: bytes.len(),
        }
    }
}

struct Event;

impl Event {
    fn tx(packet: PacketInfo, attempts: u8, state: PeerState) -> String {
        format!(
            "tx type={} pn={} ch={} msg={} attempts={} len={} state={:?}",
            packet.packet_type,
            packet.packet_number,
            packet.channel_id,
            packet.message_id,
            attempts,
            packet.len,
            state,
        )
    }
}

#[derive(Clone, Debug)]
struct EventLog {
    events: [Option<LogEntry>; EVENT_LOG_LEN],
    next: usize,
    len: usize,
}

impl EventLog {
    const fn new() -> Self {
        Self {
            events: [const { None }; EVENT_LOG_LEN],
            next: 0,
            len: 0,
        }
    }

    fn push(&mut self, now_ms: u64, side: &'static str, message: impl Into<String>) {
        self.events[self.next] = Some(LogEntry {
            now_ms,
            side,
            message: message.into(),
        });
        self.next = (self.next + 1) % EVENT_LOG_LEN;
        self.len = self.len.saturating_add(1).min(EVENT_LOG_LEN);
    }

    fn dump_stderr(&self) {
        let mut stderr = io::stderr();
        let _ = self.dump_to(&mut stderr);
    }

    fn dump_to(&self, out: &mut impl Write) -> io::Result<()> {
        writeln!(out, "sim recent events:")?;
        let start = (self.next + EVENT_LOG_LEN - self.len) % EVENT_LOG_LEN;
        for offset in 0..self.len {
            let index = (start + offset) % EVENT_LOG_LEN;
            if let Some(event) = &self.events[index] {
                writeln!(
                    out,
                    "sim event now={} side={} {}",
                    event.now_ms, event.side, event.message
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct LogEntry {
    now_ms: u64,
    side: &'static str,
    message: String,
}

fn test_config(seed: u32) -> EngineConfig {
    EngineConfig {
        initial_packet_number: msrt::core::PacketNumber::new(seed),
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
    "usage: msrt-sim-duplex [--interval-ms N] [--duration-sec N] [--message-size N] [--log-file PATH] [--noise-percent N] [--drop-byte-percent N] [--insert-byte-percent N] [--burst-corrupt-percent N] [--burst-drop-percent N] [--packet-drop-percent N]"
        .to_string()
}
