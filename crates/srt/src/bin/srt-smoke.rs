//! Minimal smoke test binary for the SRT facade crate.

use srt::{Endpoint, EndpointConfig, EndpointEvent, ReceiveReport};

fn main() {
    let mut sender = Endpoint::new(EndpointConfig::default());
    let mut receiver = Endpoint::new(EndpointConfig::default());

    let message = b"hello world from srt; send once, receive many packets";
    let message_id = sender.send(message).expect("queue message");

    println!("queued message_id={}", message_id.get());

    while let Some(event) = sender.poll_event() {
        let EndpointEvent::Write(write) = event else {
            continue;
        };

        println!(
            "sender produced packet_number={}, wire_bytes={}",
            write.packet_number.get(),
            write.as_bytes().len()
        );

        match receiver.receive(write.as_bytes()) {
            ReceiveReport::Packet { packet_number } => {
                println!("receiver accepted packet_number={}", packet_number.get());
            }
            other => panic!("unexpected receive report: {other:?}"),
        }
    }

    let Some(EndpointEvent::Message(message)) = receiver.poll_event() else {
        panic!("receiver did not emit a complete message");
    };

    let text = core::str::from_utf8(message.as_bytes()).expect("utf-8 message");

    println!(
        "srt smoke ok: message_id={}, message={text}",
        message.message_id.get()
    );

    assert_eq!(
        message.as_bytes(),
        b"hello world from srt; send once, receive many packets"
    );
}
