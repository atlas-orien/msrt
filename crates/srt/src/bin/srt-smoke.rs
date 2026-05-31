//! Mac-to-MCU smoke test simulation for the SRT facade crate.

use srt::{
    Engine, EngineConfig, EngineOutput, MAX_WIRE_BYTES, MessageEvent, ReceiveReport, WriteEvent,
};

fn main() {
    let mut mac = Engine::new(EngineConfig::default());
    let mut mcu = Engine::new(EngineConfig::default());

    inject_noise("mcu", &mut mcu);

    mac.send(b"ping from mac; split into several packets")
        .expect("queue mac message");

    let mut link = DemoLink::new();
    drain_with_faults("mac", &mut mac, "mcu", &mut mcu, &mut link);

    println!("mac: tick triggers retransmit for packets without ACK");
    mac.tick(1);
    drain_clean("mac", &mut mac, "mcu", &mut mcu);

    let ping = drain_until_message("mcu", &mut mcu, "mac", &mut mac);
    assert_eq!(
        ping.as_bytes(),
        b"ping from mac; split into several packets"
    );

    mcu.send(b"pong from mcu; same no-std engine")
        .expect("queue mcu message");
    drain_clean("mcu", &mut mcu, "mac", &mut mac);

    let pong = drain_until_message("mac", &mut mac, "mcu", &mut mcu);
    assert_eq!(pong.as_bytes(), b"pong from mcu; same no-std engine");

    println!("srt smoke ok: noise, crc error, drop, ack, retransmit, and bidirectional messages");
}

fn inject_noise(name: &str, engine: &mut Engine) {
    match engine.receive(&[0xde, 0xad, 0xbe, 0xef]) {
        ReceiveReport::Noise { skipped } => {
            println!("{name}: noise skipped={skipped}");
        }
        other => panic!("{name}: unexpected noise report: {other:?}"),
    }
}

fn drain_with_faults(
    src_name: &str,
    src: &mut Engine,
    dst_name: &str,
    dst: &mut Engine,
    link: &mut DemoLink,
) {
    while let Some(event) = src.poll_event() {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        link.deliver(src_name, dst_name, dst, write);
    }
}

fn drain_clean(src_name: &str, src: &mut Engine, dst_name: &str, dst: &mut Engine) {
    while let Some(event) = src.poll_event() {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        println!(
            "{src_name} -> {dst_name}: packet_number={}, wire_bytes={}",
            write.packet_number.get(),
            write.as_bytes().len()
        );
        log_receive(dst_name, dst.receive(write.as_bytes()));
    }
}

fn drain_until_message(
    src_name: &str,
    src: &mut Engine,
    dst_name: &str,
    dst: &mut Engine,
) -> MessageEvent {
    loop {
        match src.poll_event() {
            Some(EngineOutput::Write(write)) => {
                println!(
                    "{src_name} -> {dst_name}: packet_number={}, wire_bytes={}",
                    write.packet_number.get(),
                    write.as_bytes().len()
                );
                log_receive(dst_name, dst.receive(write.as_bytes()));
            }
            Some(EngineOutput::Message(message)) => {
                print_message(src_name, message);
                return message;
            }
            None => panic!("{src_name}: expected a complete message"),
        }
    }
}

fn log_receive(name: &str, report: ReceiveReport) {
    match report {
        ReceiveReport::Packet { packet_number } => {
            println!("{name}: accepted packet_number={}", packet_number.get());
        }
        ReceiveReport::Ack { packet_number } => {
            println!("{name}: acked packet_number={}", packet_number.get());
        }
        ReceiveReport::Noise { skipped } => {
            println!("{name}: noise skipped={skipped}");
        }
        ReceiveReport::Corrupted => {
            println!("{name}: crc error detected");
        }
        ReceiveReport::Incomplete { needed } => {
            println!("{name}: incomplete packet needed={needed:?}");
        }
        ReceiveReport::Error(error) => {
            panic!("{name}: receive error: {error:?}");
        }
    }
}

fn print_message(name: &str, message: MessageEvent) {
    let text = core::str::from_utf8(message.as_bytes()).expect("utf-8 message");

    println!(
        "{name}: received message_id={}, message={text}",
        message.message_id.get()
    );
}

#[derive(Debug, Default)]
struct DemoLink {
    dropped_packet_one: bool,
    corrupted_packet_two: bool,
}

impl DemoLink {
    fn new() -> Self {
        Self::default()
    }

    fn deliver(&mut self, src_name: &str, dst_name: &str, dst: &mut Engine, write: WriteEvent) {
        let packet_number = write.packet_number.get();

        if packet_number == 1 && !self.dropped_packet_one {
            self.dropped_packet_one = true;
            println!("{src_name} -> {dst_name}: drop packet_number={packet_number}");
            return;
        }

        if packet_number == 2 && !self.corrupted_packet_two {
            self.corrupted_packet_two = true;
            let mut corrupted = [0; MAX_WIRE_BYTES];
            let len = write.as_bytes().len();
            corrupted[..len].copy_from_slice(write.as_bytes());
            let last = len - 1;
            corrupted[last] ^= 0xff;

            println!("{src_name} -> {dst_name}: corrupt packet_number={packet_number}");
            log_receive(dst_name, dst.receive(&corrupted[..len]));
            return;
        }

        println!(
            "{src_name} -> {dst_name}: packet_number={}, wire_bytes={}",
            packet_number,
            write.as_bytes().len()
        );
        log_receive(dst_name, dst.receive(write.as_bytes()));
    }
}
