//! Minimal Mac-to-MCU smoke demo for the SRT facade crate.

use srt::{Config, Engine, Event, Message, Receive, Write};

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
    mac.send(b"hello srt").expect("queue mac hello");
    pump_link("mac", &mut mac, "mcu", &mut mcu);

    let hello = drive_until_message("mcu", &mut mcu, "mac", &mut mac);
    assert_eq!(hello.as_bytes(), b"hello srt");

    println!("mcu: send pong");
    mcu.send(b"pong srt").expect("queue mcu pong");
    pump_link("mcu", &mut mcu, "mac", &mut mac);

    let pong = drive_until_message("mac", &mut mac, "mcu", &mut mcu);
    assert_eq!(pong.as_bytes(), b"pong srt");

    println!("srt smoke ok: hello/pong message exchange");
}

fn pump_link(src_name: &str, src: &mut Engine, dst_name: &str, dst: &mut Engine) {
    while let Some(event) = src.poll_event() {
        match event {
            Event::Write(write) => deliver_write(src_name, dst_name, dst, write),
            Event::Message(message) => print_message(src_name, message),
            Event::SendFailed(failed) => {
                panic!("{src_name}: unexpected send failure: {failed:?}");
            }
        }
    }
}

fn deliver_write(src_name: &str, dst_name: &str, dst: &mut Engine, write: Write) {
    println!(
        "{src_name} -> {dst_name}: packet_number={}, wire_bytes={}",
        write.packet_number.get(),
        write.as_bytes().len()
    );

    match dst.receive(write.as_bytes()) {
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
}

fn drive_until_message(
    local_name: &str,
    local: &mut Engine,
    peer_name: &str,
    peer: &mut Engine,
) -> Message {
    loop {
        match local.poll_event() {
            Some(Event::Write(write)) => deliver_write(local_name, peer_name, peer, write),
            Some(Event::Message(message)) => {
                print_message(local_name, message);
                return message;
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
