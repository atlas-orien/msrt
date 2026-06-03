//! Minimal Mac-to-MCU smoke demo for the MSRT facade crate.

use msrt::{Config, Engine, MAX_WIRE_BYTES, Message, Poll, Receive};

fn main() {
    let mut mac = Engine::new(Config {
        fragment_bytes: 8,
        ..Config::default()
    });
    let mut mcu = Engine::new(Config {
        fragment_bytes: 8,
        ..Config::default()
    });

    println!("mac: send hello");
    mac.send(b"hello msrt").expect("queue mac hello");
    pump_link("mac", &mut mac, "mcu", &mut mcu);

    let (hello, ack_count) = drive_passive_until_message_and_ack("mcu", &mut mcu, "mac", &mut mac);
    assert_eq!(hello.as_bytes(), b"hello msrt");
    assert!(
        ack_count > 0,
        "mac should receive at least one ack from passive mcu"
    );

    println!("msrt smoke ok: passive mcu received hello and mac received {ack_count} ack(s)");
}

fn pump_link(src_name: &str, src: &mut Engine, dst_name: &str, dst: &mut Engine) {
    let mut tx_buf = [0; MAX_WIRE_BYTES];

    loop {
        match src.poll(&mut tx_buf).expect("poll engine") {
            Poll::Transmit(bytes) => {
                let _ = deliver_write(src_name, dst_name, dst, bytes);
            }
            Poll::Message(message) => print_message(src_name, message),
            Poll::SendFailed(failed) => {
                panic!("{src_name}: unexpected send failure: {failed:?}");
            }
            Poll::Idle => break,
        }
    }
}

fn deliver_write(src_name: &str, dst_name: &str, dst: &mut Engine, bytes: &[u8]) -> Receive {
    println!(
        "{src_name} -> {dst_name}: packet_number={}, wire_bytes={}",
        packet_number(bytes),
        bytes.len()
    );

    let report = dst.receive(bytes);

    match report {
        Receive::Packet { packet_number } => {
            println!("{dst_name}: accepted packet_number={}", packet_number.get());
        }
        Receive::Ack { packet_number } => {
            println!("{dst_name}: acked packet_number={}", packet_number.get());
        }
        other => {
            panic!("{dst_name}: unexpected receive report: {other:?}");
        }
    }

    report
}

fn drive_passive_until_message_and_ack(
    local_name: &str,
    local: &mut Engine,
    peer_name: &str,
    peer: &mut Engine,
) -> (Message, usize) {
    let mut ack_count = 0;
    let mut tx_buf = [0; MAX_WIRE_BYTES];

    loop {
        match local.poll(&mut tx_buf).expect("poll engine") {
            Poll::Transmit(bytes) => {
                if let Receive::Ack { .. } = deliver_write(local_name, peer_name, peer, bytes) {
                    ack_count += 1;
                }
            }
            Poll::Message(message) => {
                print_message(local_name, message);
                return (message, ack_count);
            }
            Poll::SendFailed(failed) => {
                panic!("{local_name}: unexpected send failure: {failed:?}");
            }
            Poll::Idle => panic!("{local_name}: expected a message event"),
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

fn packet_number(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[10..14].try_into().expect("packet number bytes"))
}
