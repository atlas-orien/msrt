//! Mac-to-MCU smoke test simulation for the SRT facade crate.

use srt::{Config, Engine, Event, MAX_WIRE_BYTES, Message, Receive, Write, core::PacketType};

fn main() {
    sequential_smoke();
    simultaneous_duplex_smoke();

    println!(
        "srt smoke ok: half packet, sticky packets, noise, crc error, drop, ack, retransmit, simultaneous duplex, and bidirectional messages"
    );
}

fn sequential_smoke() {
    let mut mac = Engine::new(Config {
        fragment_bytes: 12,
        ..Config::default()
    });
    let mut mcu = Engine::new(Config::default());

    inject_noise("mcu", &mut mcu);

    mac.send(b"ping from mac; split into several packets")
        .expect("queue mac message");

    let mut link = DemoLink::new();
    drain_with_faults("mac", &mut mac, "mcu", &mut mcu, &mut link);

    println!("mac: tick triggers retransmit for packets without ACK");
    mac.tick(1);
    drain_sticky("mac", &mut mac, "mcu", &mut mcu);

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
}

fn simultaneous_duplex_smoke() {
    let mut mac = Engine::new(Config {
        fragment_bytes: 10,
        ..Config::default()
    });
    let mut mcu = Engine::new(Config {
        fragment_bytes: 11,
        ..Config::default()
    });
    let mac_message = b"mac simultaneous message with several fragments";
    let mcu_message = b"mcu simultaneous message with interleaved packets";
    let mut link = ChaoticDuplexLink::new();
    let mut mac_received = false;
    let mut mcu_received = false;

    println!("duplex: both sides queue messages before either side drains writes");
    mac.send(mac_message)
        .expect("queue simultaneous mac message");
    mcu.send(mcu_message)
        .expect("queue simultaneous mcu message");

    for round in 0..6 {
        pump_duplex(
            &mut mac,
            &mut mcu,
            &mut link,
            mac_message,
            mcu_message,
            &mut mac_received,
            &mut mcu_received,
        );
        link.flush_all(&mut mac, &mut mcu);
        pump_duplex(
            &mut mac,
            &mut mcu,
            &mut link,
            mac_message,
            mcu_message,
            &mut mac_received,
            &mut mcu_received,
        );

        if mac_received && mcu_received {
            break;
        }

        println!("duplex: tick round={round} triggers both sides retransmit missing ACKs");
        mac.tick(round + 1);
        mcu.tick(round + 1);
    }

    assert!(
        mac_received,
        "mac should receive the simultaneous mcu message"
    );
    assert!(
        mcu_received,
        "mcu should receive the simultaneous mac message"
    );
}

fn pump_duplex(
    mac: &mut Engine,
    mcu: &mut Engine,
    link: &mut ChaoticDuplexLink,
    mac_message: &[u8],
    mcu_message: &[u8],
    mac_received: &mut bool,
    mcu_received: &mut bool,
) {
    for _ in 0..128 {
        let mut progressed = false;

        progressed |= poll_one_endpoint("mac", mac, "mcu", mcu, link, mcu_message, mac_received);
        progressed |= poll_one_endpoint("mcu", mcu, "mac", mac, link, mac_message, mcu_received);

        if !progressed {
            break;
        }
    }
}

fn poll_one_endpoint(
    src_name: &str,
    src: &mut Engine,
    dst_name: &str,
    dst: &mut Engine,
    link: &mut ChaoticDuplexLink,
    expected_message: &[u8],
    received: &mut bool,
) -> bool {
    match src.poll_event() {
        Some(Event::Write(write)) => {
            link.deliver(src_name, dst_name, dst, write);
            true
        }
        Some(Event::Message(message)) => {
            print_message(src_name, message);
            assert_eq!(message.as_bytes(), expected_message);
            *received = true;
            true
        }
        Some(Event::SendFailed(failed)) => {
            panic!("{src_name}: unexpected send failure: {failed:?}");
        }
        None => false,
    }
}

fn inject_noise(name: &str, engine: &mut Engine) {
    match engine.receive(&[0xde, 0xad, 0xbe, 0xef]) {
        Receive::Noise { skipped } => {
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
        let Event::Write(write) = event else {
            continue;
        };

        link.deliver(src_name, dst_name, dst, write);
    }
}

fn drain_clean(src_name: &str, src: &mut Engine, dst_name: &str, dst: &mut Engine) {
    while let Some(event) = src.poll_event() {
        let Event::Write(write) = event else {
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

fn drain_sticky(src_name: &str, src: &mut Engine, dst_name: &str, dst: &mut Engine) {
    let mut bytes = [0; MAX_WIRE_BYTES * 4];
    let mut len = 0;

    while let Some(event) = src.poll_event() {
        let Event::Write(write) = event else {
            continue;
        };
        let end = len + write.as_bytes().len();

        println!(
            "{src_name} -> {dst_name}: sticky packet_number={}, wire_bytes={}",
            write.packet_number.get(),
            write.as_bytes().len()
        );

        if end > bytes.len() {
            log_receive(dst_name, dst.receive(&bytes[..len]));
            len = 0;
        }

        let end = len + write.as_bytes().len();
        bytes[len..end].copy_from_slice(write.as_bytes());
        len = end;
    }

    if len > 0 {
        println!("{src_name} -> {dst_name}: sticky receive bytes={len}");
        log_receive(dst_name, dst.receive(&bytes[..len]));
    }
}

fn drain_until_message(
    src_name: &str,
    src: &mut Engine,
    dst_name: &str,
    dst: &mut Engine,
) -> Message {
    loop {
        match src.poll_event() {
            Some(Event::Write(write)) => {
                println!(
                    "{src_name} -> {dst_name}: packet_number={}, wire_bytes={}",
                    write.packet_number.get(),
                    write.as_bytes().len()
                );
                log_receive(dst_name, dst.receive(write.as_bytes()));
            }
            Some(Event::Message(message)) => {
                print_message(src_name, message);
                return message;
            }
            Some(Event::SendFailed(failed)) => {
                panic!("{src_name}: unexpected send failure: {failed:?}");
            }
            None => panic!("{src_name}: expected a complete message"),
        }
    }
}

fn log_receive(name: &str, report: Receive) {
    match report {
        Receive::Packet { packet_number } => {
            println!("{name}: accepted packet_number={}", packet_number.get());
        }
        Receive::Duplicate { packet_number } => {
            println!("{name}: duplicate packet_number={}", packet_number.get());
        }
        Receive::Ack { packet_number } => {
            println!("{name}: acked packet_number={}", packet_number.get());
        }
        Receive::Noise { skipped } => {
            println!("{name}: noise skipped={skipped}");
        }
        Receive::Corrupted => {
            println!("{name}: crc error detected");
        }
        Receive::Incomplete { needed } => {
            println!("{name}: incomplete packet needed={needed:?}");
        }
        Receive::Error(error) => {
            panic!("{name}: receive error: {error:?}");
        }
    }
}

fn print_message(name: &str, message: Message) {
    let text = core::str::from_utf8(message.as_bytes()).expect("utf-8 message");

    println!(
        "{name}: received message_id={}, message={text}",
        message.message_id.get()
    );
}

#[derive(Debug, Default)]
struct DemoLink {
    split_packet_zero: bool,
    dropped_packet_one: bool,
    corrupted_packet_two: bool,
}

#[derive(Debug, Default)]
struct ChaoticDuplexLink {
    mac_to_mcu: ChaoticDirection,
    mcu_to_mac: ChaoticDirection,
}

impl ChaoticDuplexLink {
    fn new() -> Self {
        Self::default()
    }

    fn deliver(&mut self, src_name: &str, dst_name: &str, dst: &mut Engine, write: Write) {
        if src_name == "mac" {
            self.mac_to_mcu.deliver(src_name, dst_name, dst, write);
        } else {
            self.mcu_to_mac.deliver(src_name, dst_name, dst, write);
        }
    }

    fn flush_all(&mut self, mac: &mut Engine, mcu: &mut Engine) {
        self.mac_to_mcu.flush("mac", "mcu", mcu);
        self.mcu_to_mac.flush("mcu", "mac", mac);
    }
}

#[derive(Debug)]
struct ChaoticDirection {
    data_writes_seen: u8,
    sticky: StickyBuffer,
}

impl Default for ChaoticDirection {
    fn default() -> Self {
        Self {
            data_writes_seen: 0,
            sticky: StickyBuffer::new(),
        }
    }
}

impl ChaoticDirection {
    fn deliver(&mut self, src_name: &str, dst_name: &str, dst: &mut Engine, write: Write) {
        if !is_data_packet(write) {
            self.deliver_clean_or_sticky(src_name, dst_name, dst, write);
            return;
        }

        self.data_writes_seen = self.data_writes_seen.saturating_add(1);

        match self.data_writes_seen {
            1 => {
                let split = 5;
                let packet_number = write.packet_number.get();

                println!(
                    "{src_name} -> {dst_name}: duplex half data packet_number={packet_number} part=1"
                );
                log_receive(dst_name, dst.receive(&write.as_bytes()[..split]));
                println!(
                    "{src_name} -> {dst_name}: duplex half data packet_number={packet_number} part=2"
                );
                log_receive(dst_name, dst.receive(&write.as_bytes()[split..]));
            }
            2 => {
                println!(
                    "{src_name} -> {dst_name}: duplex drop data packet_number={}",
                    write.packet_number.get()
                );
            }
            3 => {
                let mut corrupted = [0; MAX_WIRE_BYTES];
                let len = write.as_bytes().len();
                corrupted[..len].copy_from_slice(write.as_bytes());
                corrupted[len - 1] ^= 0xff;

                println!(
                    "{src_name} -> {dst_name}: duplex corrupt data packet_number={}",
                    write.packet_number.get()
                );
                log_receive(dst_name, dst.receive(&corrupted[..len]));
            }
            4 => {
                println!(
                    "{src_name} -> {dst_name}: duplex hold sticky data packet_number={}",
                    write.packet_number.get()
                );
                self.sticky.push(write.as_bytes());
            }
            _ => self.deliver_clean_or_sticky(src_name, dst_name, dst, write),
        }
    }

    fn deliver_clean_or_sticky(
        &mut self,
        src_name: &str,
        dst_name: &str,
        dst: &mut Engine,
        write: Write,
    ) {
        if self.sticky.is_empty() {
            println!(
                "{src_name} -> {dst_name}: duplex packet_number={}, wire_bytes={}",
                write.packet_number.get(),
                write.as_bytes().len()
            );
            log_receive(dst_name, dst.receive(write.as_bytes()));
            return;
        }

        self.sticky.push(write.as_bytes());
        println!(
            "{src_name} -> {dst_name}: duplex sticky mixed receive bytes={}",
            self.sticky.len()
        );
        log_receive(dst_name, dst.receive(self.sticky.as_bytes()));
        self.sticky.clear();
    }

    fn flush(&mut self, src_name: &str, dst_name: &str, dst: &mut Engine) {
        if self.sticky.is_empty() {
            return;
        }

        println!(
            "{src_name} -> {dst_name}: duplex flush sticky receive bytes={}",
            self.sticky.len()
        );
        log_receive(dst_name, dst.receive(self.sticky.as_bytes()));
        self.sticky.clear();
    }
}

#[derive(Debug)]
struct StickyBuffer {
    bytes: [u8; MAX_WIRE_BYTES * 4],
    len: usize,
}

impl StickyBuffer {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_WIRE_BYTES * 4],
            len: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        let end = self.len + bytes.len();

        assert!(end <= self.bytes.len(), "sticky smoke buffer too small");
        self.bytes[self.len..end].copy_from_slice(bytes);
        self.len = end;
    }

    const fn is_empty(&self) -> bool {
        self.len == 0
    }

    const fn len(&self) -> usize {
        self.len
    }

    const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }

    fn clear(&mut self) {
        self.len = 0;
    }
}

fn is_data_packet(write: Write) -> bool {
    write.as_bytes().get(8).copied() == Some(PacketType::Data.code())
}

impl DemoLink {
    fn new() -> Self {
        Self::default()
    }

    fn deliver(&mut self, src_name: &str, dst_name: &str, dst: &mut Engine, write: Write) {
        let packet_number = write.packet_number.get();

        if packet_number == 0 && !self.split_packet_zero {
            self.split_packet_zero = true;
            let split = 3;

            println!("{src_name} -> {dst_name}: half packet_number={packet_number} part=1");
            log_receive(dst_name, dst.receive(&write.as_bytes()[..split]));
            println!("{src_name} -> {dst_name}: half packet_number={packet_number} part=2");
            log_receive(dst_name, dst.receive(&write.as_bytes()[split..]));
            return;
        }

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
