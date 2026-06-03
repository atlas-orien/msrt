//! Minimal Mac-to-MCU smoke demo for the MSRT facade crate.

use msrt::{Config, Engine, Event, Message, Receive, Write};

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
    while let Some(event) = src.poll_event() {
        match event {
            Event::Write(write) => {
                let _ = deliver_write(src_name, dst_name, dst, write);
            }
            Event::Message(message) => print_message(src_name, message),
            Event::SendFailed(failed) => {
                panic!("{src_name}: unexpected send failure: {failed:?}");
            }
        }
    }
}

fn deliver_write(src_name: &str, dst_name: &str, dst: &mut Engine, write: Write) -> Receive {
    println!(
        "{src_name} -> {dst_name}: packet_number={}, wire_bytes={}",
        write.packet_number.get(),
        write.as_bytes().len()
    );

    let report = dst.receive(write.as_bytes());

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

    loop {
        match local.poll_event() {
            Some(Event::Write(write)) => {
                if let Receive::Ack { .. } = deliver_write(local_name, peer_name, peer, write) {
                    ack_count += 1;
                }
            }
            Some(Event::Message(message)) => {
                print_message(local_name, message);
                return (message, ack_count);
            }
            Some(Event::SendFailed(failed)) => {
                panic!("{local_name}: unexpected send failure: {failed:?}");
            }
            None => panic!("{local_name}: expected a message event"),
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
