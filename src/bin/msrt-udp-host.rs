use std::{
    env, io,
    net::{SocketAddr, UdpSocket},
    thread,
    time::{Duration, Instant},
};

use msrt::{
    Engine,
    engine::{EngineConfig, EnginePoll, ReceiveReport},
};

const TX_BUF_BYTES: usize = 256;
const RX_BUF_BYTES: usize = 2048;
const CHAOS_NOISE_CHUNK_BYTES: usize = 512;
const DEFAULT_STATS_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
struct Args {
    bind: SocketAddr,
    peer: SocketAddr,
    interval: Duration,
    message: Vec<u8>,
    count: Option<usize>,
    duration: Option<Duration>,
    wire_chaos: bool,
    drop_tx: Duration,
    noise_percent: u8,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut parsed = Self {
            bind: "127.0.0.1:9001".parse().expect("valid default bind addr"),
            peer: "127.0.0.1:9002".parse().expect("valid default peer addr"),
            interval: Duration::from_millis(1_000),
            message: b"ping".to_vec(),
            count: None,
            duration: None,
            wire_chaos: false,
            drop_tx: Duration::ZERO,
            noise_percent: 0,
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
                "--duration-sec" => {
                    let secs = next_value(&mut args, "--duration-sec")?
                        .parse()
                        .map_err(|error| format!("invalid --duration-sec: {error}"))?;
                    parsed.duration = Some(Duration::from_secs(secs));
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
                    parsed.noise_percent = next_value(&mut args, "--noise-percent")?
                        .parse()
                        .map_err(|error| format!("invalid --noise-percent: {error}"))?;
                    if parsed.noise_percent > 100 {
                        return Err("--noise-percent must be <= 100".to_string());
                    }
                }
                "--help" | "-h" => return Err(usage()),
                other => return Err(format!("unknown argument: {other}\n\n{}", usage())),
            }
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

    println!(
        "msrt udp host bind={} peer={} interval={:?} message_len={} noise_percent={}",
        args.bind,
        args.peer,
        args.interval,
        args.message.len(),
        args.noise_percent,
    );

    let start = Instant::now();
    let mut engine = Engine::new(EngineConfig {
        max_retransmit_attempts: u8::MAX,
        ..EngineConfig::default()
    });
    let mut rx_buf = [0; RX_BUF_BYTES];
    let mut last_send = Instant::now() - args.interval;
    let mut last_stats = Instant::now();
    let mut sent_messages = 0;
    let mut corrupted_packets = 0usize;
    let mut send_done = false;
    let mut noise_state = NoiseLcg::new();

    loop {
        let now = elapsed_ms(start);

        if !send_done && should_send(&args, sent_messages, last_send, start) {
            if engine.send(&args.message).is_ok() {
                sent_messages += 1;
                last_send = Instant::now();
            }
        }

        recv_udp(&socket, &mut engine, &mut rx_buf)?;
        let link_connected = start.elapsed() >= args.drop_tx;
        pump_engine(
            &socket,
            &args.peer,
            &mut engine,
            now,
            link_connected,
            args.noise_percent,
            &mut noise_state,
            &mut corrupted_packets,
        )?;

        if args
            .duration
            .is_some_and(|duration| start.elapsed() >= duration)
        {
            send_done = true;
        }

        if let Some(count) = args.count
            && sent_messages >= count
        {
            println!("host completed {count} message(s), corrupted_packets={corrupted_packets}");
            return Ok(());
        }

        if send_done {
            println!(
                "host completed duration test: sent={} corrupted_packets={}",
                sent_messages, corrupted_packets
            );
            return Ok(());
        }

        if last_stats.elapsed() >= DEFAULT_STATS_INTERVAL {
            println!(
                "host stats elapsed={}s sent={} corrupted_packets={}",
                start.elapsed().as_secs(),
                sent_messages,
                corrupted_packets
            );
            last_stats = Instant::now();
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

fn should_send(args: &Args, sent_messages: usize, last_send: Instant, start: Instant) -> bool {
    if args.count.is_some_and(|count| sent_messages >= count) {
        return false;
    }

    if args
        .duration
        .is_some_and(|duration| start.elapsed() >= duration)
    {
        return false;
    }

    last_send.elapsed() >= args.interval
}

fn recv_udp(socket: &UdpSocket, engine: &mut Engine, rx_buf: &mut [u8]) -> io::Result<()> {
    loop {
        match socket.recv_from(rx_buf) {
            Ok((len, _from)) => {
                for byte in &rx_buf[..len] {
                    let report = engine.receive(std::slice::from_ref(byte));
                    if !matches!(
                        report,
                        ReceiveReport::Packet { .. }
                            | ReceiveReport::Duplicate { .. }
                            | ReceiveReport::Ack { .. }
                            | ReceiveReport::Incomplete { .. }
                    ) {
                        eprintln!("host receive error: {report:?}");
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pump_engine(
    socket: &UdpSocket,
    peer: &SocketAddr,
    engine: &mut Engine,
    now_ms: u64,
    link_connected: bool,
    noise_percent: u8,
    noise_state: &mut NoiseLcg,
    corrupted_packets: &mut usize,
) -> io::Result<()> {
    loop {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match engine.poll(now_ms, &mut tx_buf).expect("engine poll") {
            EnginePoll::Transmit { bytes, attempts } => {
                if link_connected {
                    let mut packet = bytes.to_vec();
                    if noise_state.should_corrupt(noise_percent) {
                        let pos = noise_state.next_byte() as usize % packet.len();
                        packet[pos] ^= noise_state.next_byte() | 1;
                        *corrupted_packets += 1;
                    }
                    if attempts > 0 {
                        eprintln!("host retransmit attempt={attempts} len={}", packet.len());
                    }
                    socket.send_to(&packet, peer)?;
                }
            }
            EnginePoll::Message(_) => {}
            EnginePoll::SendFailed(failed) => {
                eprintln!("host send_failed: {failed:?}");
            }
            EnginePoll::Idle => return Ok(()),
        }
    }
}

struct NoiseLcg {
    state: u32,
    tx_count: usize,
}

impl NoiseLcg {
    fn new() -> Self {
        Self {
            state: 0x4d535254,
            tx_count: 0,
        }
    }

    fn next_byte(&mut self) -> u8 {
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.state >> 24) as u8
    }

    fn should_corrupt(&mut self, noise_percent: u8) -> bool {
        if noise_percent == 0 {
            return false;
        }
        self.tx_count += 1;
        let every = 100 / usize::from(noise_percent);
        every != 0 && self.tx_count % every == 0
    }
}

fn next_transmit(engine: &mut Engine, now_ms: u64) -> Vec<u8> {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match engine.poll(now_ms, &mut tx_buf).expect("engine poll") {
        EnginePoll::Transmit { bytes, .. } => bytes.to_vec(),
        other => panic!("expected transmit packet in chaos fixture, got {other:?}"),
    }
}

fn make_noise_bytes(len: usize) -> Vec<u8> {
    let mut lcg = NoiseLcg::new();
    (0..len).map(|_| lcg.next_byte()).collect()
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

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value\n\n{}", usage()))
}

fn usage() -> String {
    "usage: msrt-udp-host [--bind ADDR] [--peer ADDR] [--interval-ms N] [--message TEXT] [--message-size N] [--count N] [--duration-sec N] [--noise-percent N] [--wire-chaos] [--drop-tx-ms N]"
        .to_string()
}
