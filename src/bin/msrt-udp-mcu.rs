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

#[derive(Clone, Debug)]
struct Args {
    bind: SocketAddr,
    peer: SocketAddr,
    count: Option<usize>,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut parsed = Self {
            bind: "127.0.0.1:9002".parse().expect("valid default bind addr"),
            peer: "127.0.0.1:9001".parse().expect("valid default peer addr"),
            count: None,
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
                "--count" => {
                    parsed.count = Some(
                        next_value(&mut args, "--count")?
                            .parse()
                            .map_err(|error| format!("invalid --count: {error}"))?,
                    );
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

    println!("msrt udp mcu bind={} peer={}", args.bind, args.peer);

    let start = Instant::now();
    let mut engine = Engine::new(EngineConfig {
        max_retransmit_attempts: u8::MAX,
        ..EngineConfig::default()
    });
    let mut rx_buf = [0; RX_BUF_BYTES];
    let mut received_messages = 0;

    loop {
        let now = elapsed_ms(start);

        recv_udp(&socket, &mut engine, &mut rx_buf)?;
        received_messages += pump_engine(&socket, &args.peer, &mut engine, now)?;

        if let Some(count) = args.count
            && received_messages >= count
        {
            println!("mcu completed {count} received message(s)");
            return Ok(());
        }

        thread::sleep(Duration::from_millis(5));
    }
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
                        eprintln!("mcu receive error: {report:?}");
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

fn pump_engine(
    socket: &UdpSocket,
    peer: &SocketAddr,
    engine: &mut Engine,
    now_ms: u64,
) -> io::Result<usize> {
    let mut delivered = 0;

    loop {
        let mut tx_buf = [0; TX_BUF_BYTES];
        match engine.poll(now_ms, &mut tx_buf).expect("engine poll") {
            EnginePoll::Transmit { bytes, .. } => {
                socket.send_to(bytes, peer)?;
            }
            EnginePoll::Message(_message) => {
                delivered += 1;
            }
            EnginePoll::SendFailed(failed) => {
                eprintln!("mcu send_failed: {failed:?}");
            }
            EnginePoll::Idle => return Ok(delivered),
        }
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value\n\n{}", usage()))
}

fn usage() -> String {
    "usage: msrt-udp-mcu [--bind ADDR] [--peer ADDR] [--count N]".to_string()
}
