use std::{
    env, io,
    net::{SocketAddr, UdpSocket},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod support;

use msrt::{
    Engine,
    endpoint::{ClientEndpoint, EndpointPoll, PeerState},
    engine::{EngineConfig, EnginePoll, ReceiveReport},
};

use support::noise::{
    NoiseConfig, NoiseLcg, make_noise_bytes, mutate_or_copy, parse_percent_per_mille,
};

const TX_BUF_BYTES: usize = 256;
const RX_BUF_BYTES: usize = 2048;
const CHAOS_NOISE_CHUNK_BYTES: usize = 512;
const MAX_MESSAGE_BYTES: usize = 256;
const DEFAULT_MESSAGE_BYTES: usize = 240;
const TEST_FRAGMENT_BYTES: usize = 48;
const DEFAULT_CORRUPT_PER_MILLE: u16 = 5;
const DEFAULT_DROP_BYTE_PER_MILLE: u16 = 5;
const DEFAULT_INSERT_BYTE_PER_MILLE: u16 = 5;

#[derive(Clone, Debug)]
struct Args {
    bind: SocketAddr,
    peer: SocketAddr,
    interval: Duration,
    message: Vec<u8>,
    count: Option<usize>,
    wire_chaos: bool,
    drop_tx: Duration,
    noise: NoiseConfig,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut parsed = Self {
            bind: "127.0.0.1:9001".parse().expect("valid default bind addr"),
            peer: "127.0.0.1:9002".parse().expect("valid default peer addr"),
            interval: Duration::from_millis(20),
            message: make_message(DEFAULT_MESSAGE_BYTES),
            count: None,
            wire_chaos: false,
            drop_tx: Duration::ZERO,
            noise: NoiseConfig {
                corrupt_per_mille: DEFAULT_CORRUPT_PER_MILLE,
                drop_byte_per_mille: DEFAULT_DROP_BYTE_PER_MILLE,
                insert_byte_per_mille: DEFAULT_INSERT_BYTE_PER_MILLE,
            },
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bind" => {
                    parsed.bind = next_value(&mut args, "--bind")?
                        .parse()
                        .map_err(|error| format!("invalid --bind address: {error}"))?;
                }
                "--peer" => {
                    parsed.peer = next_value(&mut args, "--peer")?
                        .parse()
                        .map_err(|error| format!("invalid --peer address: {error}"))?;
                }
                "--interval-ms" => {
                    let millis = next_value(&mut args, "--interval-ms")?
                        .parse()
                        .map_err(|error| format!("invalid --interval-ms: {error}"))?;
                    parsed.interval = Duration::from_millis(millis);
                }
                "--message" => {
                    parsed.message = next_value(&mut args, "--message")?.into_bytes();
                }
                "--message-size" => {
                    let len = next_value(&mut args, "--message-size")?
                        .parse()
                        .map_err(|error| format!("invalid --message-size: {error}"))?;
                    parsed.message = make_message(len);
                }
                "--count" => {
                    parsed.count = Some(
                        next_value(&mut args, "--count")?
                            .parse()
                            .map_err(|error| format!("invalid --count: {error}"))?,
                    );
                }
                "--wire-chaos" => {
                    parsed.wire_chaos = true;
                }
                "--drop-tx-ms" => {
                    let millis = next_value(&mut args, "--drop-tx-ms")?
                        .parse()
                        .map_err(|error| format!("invalid --drop-tx-ms: {error}"))?;
                    parsed.drop_tx = Duration::from_millis(millis);
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = match Args::parse() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let socket = UdpSocket::bind(args.bind)?;
    socket.set_nonblocking(true)?;

    if args.wire_chaos {
        return run_wire_chaos(socket, args);
    }

    let start = Instant::now();
    let mut endpoint = ClientEndpoint::new(test_config());
    let mut last_state = PeerState::Disconnected;
    if let Err(error) = endpoint.connect(0) {
        eprintln!("host connect error: {error:?}");
        std::process::exit(1);
    }
    log_state_change("host", &mut last_state, endpoint.peer().state());
    let mut rx_buf = [0; RX_BUF_BYTES];
    let mut last_send = Instant::now() - args.interval;
    let mut sent_messages = 0;
    let mut noise_state = NoiseLcg::new();
    let mut last_connect_attempt = Instant::now();

    loop {
        let now = elapsed_ms(start);
        let apply_noise = endpoint.peer().is_connected();

        if !endpoint.peer().has_session() {
            if last_connect_attempt.elapsed() >= args.interval {
                if endpoint.connect(now).is_err() {
                    std::process::exit(1);
                }
                last_connect_attempt = Instant::now();
            }
        }

        if endpoint.peer().is_connected() && should_send(&args, sent_messages, last_send) {
            match endpoint.peer_mut().send(&args.message) {
                Ok(Some(_)) => {
                    sent_messages += 1;
                    last_send = Instant::now();
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }

        recv_udp_endpoint(&socket, &mut endpoint, now, &mut rx_buf)?;
        let link_connected = start.elapsed() >= args.drop_tx;
        let noise = if apply_noise {
            args.noise
        } else {
            NoiseConfig::default()
        };
        pump_endpoint(
            &socket,
            &args.peer,
            &mut endpoint,
            now,
            link_connected,
            noise,
            &mut noise_state,
        )?;
        log_state_change("host", &mut last_state, endpoint.peer().state());

        if let Some(count) = args.count
            && sent_messages >= count
        {
            return Ok(());
        }

        thread::sleep(Duration::from_millis(5));
    }
}

fn run_wire_chaos(socket: UdpSocket, args: Args) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "msrt udp host wire-chaos bind={} peer={}",
        args.bind, args.peer
    );

    let start = Instant::now();
    let mut engine = Engine::default();
    let mut rx_buf = [0; RX_BUF_BYTES];

    let noise = make_noise_bytes(512);
    for chunk in noise.chunks(CHAOS_NOISE_CHUNK_BYTES) {
        socket.send_to(chunk, args.peer)?;
        println!("host chaos send noise chunk len={}", chunk.len());
    }

    engine.send(b"crc-corrupt").expect("queue corrupt fixture");
    let mut corrupt = next_transmit(&mut engine, elapsed_ms(start));
    if let Some(last) = corrupt.last_mut() {
        *last ^= 0xff;
    }
    socket.send_to(&corrupt, args.peer)?;
    println!("host chaos send corrupted packet len={}", corrupt.len());

    engine.send(b"split-packet").expect("queue split fixture");
    let split = next_transmit(&mut engine, elapsed_ms(start));
    let split_at = core::cmp::min(3, split.len());
    socket.send_to(&split[..split_at], args.peer)?;
    println!("host chaos send split first len={split_at}");
    thread::sleep(Duration::from_millis(20));
    socket.send_to(&split[split_at..], args.peer)?;
    println!("host chaos send split rest len={}", split.len() - split_at);

    engine
        .send(b"sticky-one")
        .expect("queue first sticky fixture");
    engine
        .send(b"sticky-two")
        .expect("queue second sticky fixture");
    let first = next_transmit(&mut engine, elapsed_ms(start));
    let second = next_transmit(&mut engine, elapsed_ms(start));
    let mut sticky = Vec::with_capacity(first.len() + second.len());
    sticky.extend_from_slice(&first);
    sticky.extend_from_slice(&second);
    socket.send_to(&sticky, args.peer)?;
    println!("host chaos send sticky len={}", sticky.len());

    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        recv_udp(&socket, &mut engine, &mut rx_buf)?;
        thread::sleep(Duration::from_millis(5));
    }

    println!("host chaos complete");
    Ok(())
}

fn should_send(args: &Args, sent_messages: usize, last_send: Instant) -> bool {
    if args.count.is_some_and(|count| sent_messages >= count) {
        return false;
    }

    last_send.elapsed() >= args.interval
}

fn recv_udp_endpoint(
    socket: &UdpSocket,
    endpoint: &mut ClientEndpoint,
    now_ms: u64,
    rx_buf: &mut [u8],
) -> io::Result<()> {
    loop {
        match socket.recv_from(rx_buf) {
            Ok((len, _from)) => {
                for byte in &rx_buf[..len] {
                    let report = endpoint.receive(now_ms, std::slice::from_ref(byte));
                    let _ = matches!(
                        report,
                        ReceiveReport::Packet { .. }
                            | ReceiveReport::Duplicate { .. }
                            | ReceiveReport::Ack { .. }
                            | ReceiveReport::Ping { .. }
                            | ReceiveReport::Pong { .. }
                            | ReceiveReport::Incomplete { .. }
                    );
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

fn recv_udp(socket: &UdpSocket, engine: &mut Engine, rx_buf: &mut [u8]) -> io::Result<()> {
    loop {
        match socket.recv_from(rx_buf) {
            Ok((len, _from)) => {
                for byte in &rx_buf[..len] {
                    let report = engine.receive(std::slice::from_ref(byte));
                    let _ = matches!(
                        report,
                        ReceiveReport::Packet { .. }
                            | ReceiveReport::Duplicate { .. }
                            | ReceiveReport::Ack { .. }
                            | ReceiveReport::Ping { .. }
                            | ReceiveReport::Pong { .. }
                            | ReceiveReport::Incomplete { .. }
                    );
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pump_endpoint(
    socket: &UdpSocket,
    peer: &SocketAddr,
    endpoint: &mut ClientEndpoint,
    now_ms: u64,
    link_connected: bool,
    noise: NoiseConfig,
    noise_state: &mut NoiseLcg,
) -> io::Result<()> {
    loop {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match endpoint.poll(now_ms, &mut tx_buf).expect("endpoint poll") {
            EndpointPoll::Transmit { bytes, attempts: _ } => {
                if link_connected {
                    let (packet, _) = mutate_or_copy(noise_state, bytes, noise);
                    socket.send_to(&packet, peer)?;
                }
            }
            EndpointPoll::Message(_) => {}
            EndpointPoll::SendFailed(_) => {}
            EndpointPoll::Idle => return Ok(()),
        }
    }
}

fn log_state_change(prefix: &str, last: &mut PeerState, current: PeerState) {
    if *last == current {
        return;
    }

    match current {
        PeerState::Disconnected => println!("{prefix} disconnect"),
        PeerState::Connecting => println!("{prefix} connect state=Connecting"),
        PeerState::Connected => println!("{prefix} connect state=Connected"),
    }
    *last = current;
}

fn next_transmit(engine: &mut Engine, now_ms: u64) -> Vec<u8> {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match engine.poll(now_ms, &mut tx_buf).expect("engine poll") {
        EnginePoll::Transmit { bytes, .. } => bytes.to_vec(),
        other => panic!("expected transmit packet in chaos fixture, got {other:?}"),
    }
}

fn make_message(len: usize) -> Vec<u8> {
    let mut message = Vec::with_capacity(len);

    for index in 0..len {
        message.push(b'a' + (index % 26) as u8);
    }

    message
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
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
        initial_packet_number: msrt::core::PacketNumber::new(session_id),
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
    "usage: msrt-udp-host [--bind ADDR] [--peer ADDR] [--interval-ms N] [--message TEXT] [--message-size N] [--count N] [--noise-percent N] [--drop-byte-percent N] [--insert-byte-percent N] [--wire-chaos] [--drop-tx-ms N]"
        .to_string()
}
