use std::{
    env, io,
    net::{SocketAddr, UdpSocket},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod support;

use msrt::{
    endpoint::{EndpointPoll, PassiveEndpoint, PeerState},
    engine::{EngineConfig, ReceiveReport},
};

use support::noise::{NoiseConfig, NoiseLcg, mutate_or_copy, parse_percent_per_mille};

const TX_BUF_BYTES: usize = 256;
const RX_BUF_BYTES: usize = 2048;
const MAX_MESSAGE_BYTES: usize = 256;
const DEFAULT_MESSAGE_BYTES: usize = 240;
const TEST_FRAGMENT_BYTES: usize = 48;
const DEFAULT_CORRUPT_PER_MILLE: u16 = 30;
const DEFAULT_DROP_BYTE_PER_MILLE: u16 = 30;
const DEFAULT_INSERT_BYTE_PER_MILLE: u16 = 30;

#[derive(Clone, Debug)]
struct Args {
    bind: SocketAddr,
    peer: SocketAddr,
    interval: Duration,
    message: Vec<u8>,
    count: Option<usize>,
    duration: Option<Duration>,
    noise: NoiseConfig,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut parsed = Self {
            bind: "127.0.0.1:9002".parse().expect("valid default bind addr"),
            peer: "127.0.0.1:9001".parse().expect("valid default peer addr"),
            interval: Duration::from_millis(10),
            message: make_message(DEFAULT_MESSAGE_BYTES),
            count: None,
            duration: None,
            noise: NoiseConfig {
                corrupt_per_mille: DEFAULT_CORRUPT_PER_MILLE,
                drop_byte_per_mille: DEFAULT_DROP_BYTE_PER_MILLE,
                insert_byte_per_mille: DEFAULT_INSERT_BYTE_PER_MILLE,
                burst_corrupt_per_mille: 0,
                burst_drop_per_mille: 0,
                packet_drop_per_mille: 0,
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
                "--duration-sec" => {
                    let secs = next_value(&mut args, "--duration-sec")?
                        .parse()
                        .map_err(|error| format!("invalid --duration-sec: {error}"))?;
                    parsed.duration = Some(Duration::from_secs(secs));
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
    let rx_packets = spawn_udp_rx(&socket)?;

    let start = Instant::now();
    let mut endpoint = PassiveEndpoint::new(test_config());
    let mut sent_messages = 0;
    let mut receive_done = false;
    let mut last_send = Instant::now() - args.interval;
    let mut noise_state = NoiseLcg::with_seed(0x4d435520);
    let mut last_state = PeerState::Disconnected;

    loop {
        let now = elapsed_ms(start);

        recv_udp(
            &rx_packets,
            &socket,
            &args.peer,
            &mut endpoint,
            now,
            args.noise,
            &mut noise_state,
        )?;
        let noise = if endpoint.peer().is_connected() {
            args.noise
        } else {
            NoiseConfig::default()
        };
        pump_endpoint(
            &socket,
            &args.peer,
            &mut endpoint,
            now,
            noise,
            &mut noise_state,
        )?;
        log_state_change("mcu", &mut last_state, endpoint.peer().state());

        if endpoint.peer().is_connected() && should_send(&args, sent_messages, last_send, start) {
            match endpoint.send(&args.message) {
                Ok(Some(_)) => {
                    sent_messages += 1;
                    last_send = Instant::now();
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }

        if let Some(count) = args.count
            && sent_messages >= count
        {
            return Ok(());
        }

        if args
            .duration
            .is_some_and(|duration| start.elapsed() >= duration)
        {
            receive_done = true;
        }

        if receive_done {
            return Ok(());
        }

        thread::sleep(Duration::from_millis(5));
    }
}

fn recv_udp(
    rx_packets: &Receiver<Vec<u8>>,
    socket: &UdpSocket,
    peer: &SocketAddr,
    endpoint: &mut PassiveEndpoint,
    now_ms: u64,
    noise: NoiseConfig,
    noise_state: &mut NoiseLcg,
) -> io::Result<()> {
    while let Ok(packet) = rx_packets.try_recv() {
        let tx_noise = if endpoint.peer().is_connected() {
            noise
        } else {
            NoiseConfig::default()
        };
        // Keep the UART/ISR model: UDP only carries test bytes, the engine is fed one byte at a time.
        for byte in &packet {
            let report = endpoint.receive(now_ms, std::slice::from_ref(byte));
            let _ = matches!(
                report,
                ReceiveReport::Packet { .. }
                    | ReceiveReport::Duplicate { .. }
                    | ReceiveReport::Ack { .. }
                    | ReceiveReport::Ping
                    | ReceiveReport::Pong
                    | ReceiveReport::Incomplete { .. }
            );
        }

        pump_endpoint(socket, peer, endpoint, now_ms, tx_noise, noise_state)?;
    }

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

fn pump_endpoint(
    socket: &UdpSocket,
    peer: &SocketAddr,
    endpoint: &mut PassiveEndpoint,
    now_ms: u64,
    noise: NoiseConfig,
    noise_state: &mut NoiseLcg,
) -> io::Result<()> {
    loop {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match endpoint.poll(now_ms, &mut tx_buf) {
            Err(error) => {
                eprintln!("mcu endpoint poll error: {error:?}");
                return Ok(());
            }
            Ok(EndpointPoll::Transmit { bytes, .. }) => {
                let (packet, _) = mutate_or_copy(noise_state, bytes, noise);
                socket.send_to(&packet, peer)?;
            }
            Ok(EndpointPoll::Message(_)) => {}
            Ok(EndpointPoll::SendFailed(_)) => {}
            Ok(EndpointPoll::Idle) => return Ok(()),
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

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn spawn_udp_rx(socket: &UdpSocket) -> io::Result<Receiver<Vec<u8>>> {
    let socket = socket.try_clone()?;
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut rx_buf = [0; RX_BUF_BYTES];

        while let Ok((len, _from)) = socket.recv_from(&mut rx_buf) {
            if tx.send(rx_buf[..len].to_vec()).is_err() {
                break;
            }
        }
    });

    Ok(rx)
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

fn make_message(len: usize) -> Vec<u8> {
    let mut message = Vec::with_capacity(len);

    for index in 0..len {
        message.push(b'a' + (index % 26) as u8);
    }

    message
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value\n\n{}", usage()))
}

fn usage() -> String {
    "usage: msrt-udp-mcu [--bind ADDR] [--peer ADDR] [--interval-ms N] [--message TEXT] [--message-size N] [--count N] [--duration-sec N] [--noise-percent N] [--drop-byte-percent N] [--insert-byte-percent N]"
        .to_string()
}
