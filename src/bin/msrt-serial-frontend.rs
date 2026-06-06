use std::{
    env, io,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod support;

use msrt::{
    core::ErrorKind,
    endpoint::{ClientEndpoint, EndpointPoll, PeerState},
    engine::EngineConfig,
};

use support::noise::{
    NoiseConfig, NoiseLcg, NoiseStats, has_noise, mutate_or_copy, parse_percent_per_mille,
};

const TX_BUF_BYTES: usize = 256;
const RX_BUF_BYTES: usize = 256;
const DEFAULT_BAUD: u32 = 115_200;
const DEFAULT_INTERVAL_MS: u64 = 1_000;
const DEFAULT_MESSAGE: &[u8] = b"ping";
const REOPEN_INTERVAL: Duration = Duration::from_millis(500);
const STATS_INTERVAL: Duration = Duration::from_secs(60);
const TEST_FRAGMENT_BYTES: usize = 48;

#[derive(Clone, Debug)]
struct Args {
    port: String,
    baud: u32,
    interval: Duration,
    message: Vec<u8>,
    noise: NoiseConfig,
    verbose: bool,
}

#[derive(Debug)]
struct FrontendStats {
    received_messages: usize,
    backpressure: usize,
    noise_state: NoiseLcg,
    noise_stats: NoiseStats,
    last_stats: Instant,
}

impl FrontendStats {
    fn new() -> Self {
        Self {
            received_messages: 0,
            backpressure: 0,
            noise_state: NoiseLcg::with_seed(0x53455231),
            noise_stats: NoiseStats::default(),
            last_stats: Instant::now(),
        }
    }
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut parsed = Self {
            port: String::new(),
            baud: DEFAULT_BAUD,
            interval: Duration::from_millis(DEFAULT_INTERVAL_MS),
            message: DEFAULT_MESSAGE.to_vec(),
            noise: NoiseConfig::default(),
            verbose: false,
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--port" => parsed.port = next_value(&mut args, "--port")?,
                "--baud" => {
                    parsed.baud = next_value(&mut args, "--baud")?
                        .parse()
                        .map_err(|error| format!("invalid --baud: {error}"))?;
                }
                "--interval-ms" => {
                    let millis = next_value(&mut args, "--interval-ms")?
                        .parse()
                        .map_err(|error| format!("invalid --interval-ms: {error}"))?;
                    parsed.interval = Duration::from_millis(millis);
                }
                "--message" => parsed.message = next_value(&mut args, "--message")?.into_bytes(),
                "--noise-percent" => {
                    let total = parse_percent_per_mille(
                        next_value(&mut args, "--noise-percent")?,
                        "--noise-percent",
                    )?;
                    set_mixed_noise(&mut parsed.noise, total);
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
                "--verbose" => parsed.verbose = true,
                "--help" | "-h" => return Err(usage()),
                other => return Err(format!("unknown argument: {other}\n\n{}", usage())),
            }
        }

        if parsed.port.is_empty() {
            return Err(format!("--port is required\n\n{}", usage()));
        }

        Ok(parsed)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = match Args::parse() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    println!(
        "msrt serial frontend port={} baud={} interval={}ms message_len={} noise={} corrupt={} drop_byte={} insert_byte={} burst_corrupt={} burst_drop={} packet_drop={} verbose={}",
        args.port,
        args.baud,
        args.interval.as_millis(),
        args.message.len(),
        percent_text(noise_total(args.noise)),
        percent_text(args.noise.corrupt_per_mille),
        percent_text(args.noise.drop_byte_per_mille),
        percent_text(args.noise.insert_byte_per_mille),
        percent_text(args.noise.burst_corrupt_per_mille),
        percent_text(args.noise.burst_drop_per_mille),
        percent_text(args.noise.packet_drop_per_mille),
        args.verbose
    );

    let start = Instant::now();
    let mut endpoint = ClientEndpoint::new(test_config());
    let mut serial = open_serial(&args)?;
    let mut last_state = PeerState::Disconnected;
    let mut last_send = Instant::now() - args.interval;
    let mut last_reopen = Instant::now();
    let mut sent_messages = 0usize;
    let mut stats = FrontendStats::new();
    let mut rx_buf = [0; RX_BUF_BYTES];

    connect_endpoint(&mut endpoint, 0, &mut last_state)?;

    loop {
        let now_ms = elapsed_ms(start);

        if serial.is_none() {
            if last_reopen.elapsed() >= REOPEN_INTERVAL {
                last_reopen = Instant::now();
                serial = open_serial(&args)?;
                connect_endpoint(&mut endpoint, now_ms, &mut last_state)?;
            }
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        let link = serial.as_mut().expect("serial exists");
        if let Err(error) = read_serial(&mut **link, &mut endpoint, now_ms, &mut rx_buf) {
            drop_serial(&mut serial, &mut endpoint, &mut last_state, "read", error);
            continue;
        }

        if let Err(error) = pump_endpoint(
            &mut **link,
            &mut endpoint,
            now_ms,
            &mut last_state,
            &args,
            &mut stats,
        ) {
            drop_serial(&mut serial, &mut endpoint, &mut last_state, "write", error);
            continue;
        }

        log_state_change(&mut last_state, endpoint.peer().state());

        if !endpoint.peer().has_session() {
            connect_endpoint(&mut endpoint, now_ms, &mut last_state)?;
            last_send = Instant::now();
        }

        if endpoint.peer().is_connected() && last_send.elapsed() >= args.interval {
            match endpoint.peer_mut().send(&args.message) {
                Ok(Some(_)) => {
                    sent_messages += 1;
                    last_send = Instant::now();
                    if args.verbose {
                        println!("frontend send count={sent_messages}");
                    }
                }
                Ok(None) => {}
                Err(error) if error.kind() == ErrorKind::Engine => {
                    stats.backpressure += 1;
                    last_send = Instant::now();
                    if args.verbose {
                        println!("frontend send backpressure count={}", stats.backpressure);
                    }
                }
                Err(error) => {
                    println!("frontend send error: {error:?}");
                }
            }
        }

        print_stats_if_due(start, &args, &mut stats, sent_messages);

        std::thread::sleep(Duration::from_millis(1));
    }
}

fn open_serial(args: &Args) -> io::Result<Option<Box<dyn serialport::SerialPort>>> {
    match serialport::new(&args.port, args.baud)
        .timeout(Duration::from_millis(1))
        .open()
    {
        Ok(serial) => {
            println!("frontend serial open port={}", args.port);
            Ok(Some(serial))
        }
        Err(error) => {
            println!("frontend serial wait port={} error={error}", args.port);
            Ok(None)
        }
    }
}

fn drop_serial(
    serial: &mut Option<Box<dyn serialport::SerialPort>>,
    endpoint: &mut ClientEndpoint,
    last_state: &mut PeerState,
    operation: &str,
    error: io::Error,
) {
    println!("frontend serial disconnect operation={operation} error={error}");
    *serial = None;
    endpoint.disconnect();
    log_state_change(last_state, endpoint.peer().state());
}

fn connect_endpoint(
    endpoint: &mut ClientEndpoint,
    now_ms: u64,
    last_state: &mut PeerState,
) -> io::Result<()> {
    endpoint.connect(now_ms).map_err(msrt_io_error)?;
    log_state_change(last_state, endpoint.peer().state());
    Ok(())
}

fn read_serial(
    serial: &mut dyn serialport::SerialPort,
    endpoint: &mut ClientEndpoint,
    now_ms: u64,
    rx_buf: &mut [u8],
) -> io::Result<()> {
    loop {
        match serial.read(rx_buf) {
            Ok(0) => return Ok(()),
            Ok(len) => {
                for byte in &rx_buf[..len] {
                    let _ = endpoint.receive(now_ms, core::slice::from_ref(byte));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::TimedOut => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

fn pump_endpoint(
    serial: &mut dyn serialport::SerialPort,
    endpoint: &mut ClientEndpoint,
    now_ms: u64,
    last_state: &mut PeerState,
    args: &Args,
    stats: &mut FrontendStats,
) -> io::Result<()> {
    loop {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match endpoint.poll(now_ms, &mut tx_buf).map_err(msrt_io_error)? {
            EndpointPoll::Transmit { bytes, .. } => {
                let noise = if endpoint.peer().is_connected() {
                    args.noise
                } else {
                    NoiseConfig::default()
                };
                let (tx_bytes, noise_delta) = mutate_or_copy(&mut stats.noise_state, bytes, noise);
                stats.noise_stats.add(noise_delta);
                if args.verbose && has_noise(noise) && noise_delta.any() {
                    println!(
                        "frontend noise corrupted={} dropped={} inserted={} burst_corrupted={} burst_dropped={} packet_dropped={}",
                        stats.noise_stats.corrupted,
                        stats.noise_stats.dropped,
                        stats.noise_stats.inserted,
                        stats.noise_stats.burst_corrupted,
                        stats.noise_stats.burst_dropped,
                        stats.noise_stats.packet_dropped
                    );
                }
                if tx_bytes.is_empty() {
                    continue;
                }
                serial.write_all(&tx_bytes)?;
                serial.flush()?;
            }
            EndpointPoll::Message(message) => {
                stats.received_messages += 1;
                if args.verbose {
                    println!(
                        "frontend message count={} packet_type={:?} len={} text={}",
                        stats.received_messages,
                        message.packet_type,
                        message.as_bytes().len(),
                        printable(message.as_bytes())
                    );
                }
            }
            EndpointPoll::SendFailed(failed) => {
                println!(
                    "frontend send_failed packet_type={:?} msg={}",
                    failed.packet_type,
                    failed.message_id.get()
                );
                log_state_change(last_state, endpoint.peer().state());
            }
            EndpointPoll::Idle => return Ok(()),
        }
    }
}

fn print_stats_if_due(
    start: Instant,
    args: &Args,
    stats: &mut FrontendStats,
    sent_messages: usize,
) {
    if stats.last_stats.elapsed() < STATS_INTERVAL {
        return;
    }

    stats.last_stats = Instant::now();
    println!(
        "frontend stats elapsed={}s sent={} received={} backpressure={} corrupted={} dropped={} inserted={} burst_corrupted={} burst_dropped={} packet_dropped={}",
        start.elapsed().as_secs(),
        sent_messages,
        stats.received_messages,
        stats.backpressure,
        stats.noise_stats.corrupted,
        stats.noise_stats.dropped,
        stats.noise_stats.inserted,
        stats.noise_stats.burst_corrupted,
        stats.noise_stats.burst_dropped,
        stats.noise_stats.packet_dropped
    );

    if args.verbose && !has_noise(args.noise) {
        println!("frontend stats noise=disabled");
    }
}

fn log_state_change(last: &mut PeerState, current: PeerState) {
    if *last == current {
        return;
    }

    match current {
        PeerState::Disconnected => println!("frontend disconnect"),
        PeerState::Connecting => println!("frontend connect state=Connecting"),
        PeerState::Connected => println!("frontend connect state=Connected"),
    }
    *last = current;
}

fn printable(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| {
            if byte.is_ascii_graphic() || *byte == b' ' {
                char::from(*byte)
            } else {
                '.'
            }
        })
        .collect()
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn msrt_io_error(error: msrt::core::Error) -> io::Error {
    io::Error::other(format!("{error:?}"))
}

fn process_session_id() -> u32 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();

    millis as u32
}

fn test_config() -> EngineConfig {
    let session_id = process_session_id();
    EngineConfig {
        initial_message_id: msrt::core::MessageId::new(session_id),
        fragment_bytes: TEST_FRAGMENT_BYTES,
        ..EngineConfig::default()
    }
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value\n\n{}", usage()))
}

fn usage() -> String {
    "usage: msrt-serial-frontend --port PATH [--baud N] [--interval-ms N] [--message TEXT] [--noise-percent N] [--drop-byte-percent N] [--insert-byte-percent N] [--burst-corrupt-percent N] [--burst-drop-percent N] [--packet-drop-percent N] [--verbose]".to_string()
}

fn percent_text(per_mille: u16) -> String {
    format!("{:.1}", f32::from(per_mille) / 10.0)
}

fn set_mixed_noise(noise: &mut NoiseConfig, total_per_mille: u16) {
    let base = total_per_mille / 6;
    let remainder = total_per_mille % 6;

    noise.corrupt_per_mille = base + u16::from(remainder > 0);
    noise.drop_byte_per_mille = base + u16::from(remainder > 1);
    noise.insert_byte_per_mille = base + u16::from(remainder > 2);
    noise.burst_corrupt_per_mille = base + u16::from(remainder > 3);
    noise.burst_drop_per_mille = base + u16::from(remainder > 4);
    noise.packet_drop_per_mille = base;
}

fn noise_total(noise: NoiseConfig) -> u16 {
    noise
        .corrupt_per_mille
        .saturating_add(noise.drop_byte_per_mille)
        .saturating_add(noise.insert_byte_per_mille)
        .saturating_add(noise.burst_corrupt_per_mille)
        .saturating_add(noise.burst_drop_per_mille)
        .saturating_add(noise.packet_drop_per_mille)
}

trait NoiseStatsExt {
    fn add(&mut self, other: Self);
    fn any(self) -> bool;
}

impl NoiseStatsExt for NoiseStats {
    fn add(&mut self, other: Self) {
        self.corrupted += other.corrupted;
        self.dropped += other.dropped;
        self.inserted += other.inserted;
        self.burst_corrupted += other.burst_corrupted;
        self.burst_dropped += other.burst_dropped;
        self.packet_dropped += other.packet_dropped;
    }

    fn any(self) -> bool {
        self.corrupted != 0
            || self.dropped != 0
            || self.inserted != 0
            || self.burst_corrupted != 0
            || self.burst_dropped != 0
            || self.packet_dropped != 0
    }
}
