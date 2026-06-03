use msrt::{ChannelId, Config, Engine, Event, Receive};

#[test]
fn default_message_is_acked_without_adapter() {
    let mut host = Engine::new(Config::default());
    let mut mcu = Engine::new(Config::default());

    host.send(b"host ping").expect("queue host message");

    let host_write = next_write(&mut host);
    assert!(matches!(
        mcu.receive(host_write.as_bytes()),
        Receive::Packet { .. }
    ));

    let ack = next_write(&mut mcu);
    assert!(matches!(host.receive(ack.as_bytes()), Receive::Ack { .. }));

    let mcu_message = next_message(&mut mcu);
    assert_eq!(mcu_message.channel_id, ChannelId::DEFAULT);
    assert_eq!(mcu_message.as_bytes(), b"host ping");

    host.tick(msrt::DEFAULT_RETRANSMIT_TIMEOUT_MS);
    assert_no_send_failed(&mut host);
}

#[test]
fn mcu_can_debug_after_receiving_default_message_without_adapter() {
    let mut host = Engine::new(Config::default());
    let mut mcu = Engine::new(Config::default());

    host.send(b"host ping").expect("queue host message");
    deliver_next_write(&mut host, &mut mcu);

    deliver_next_write(&mut mcu, &mut host);

    let mcu_message = next_message(&mut mcu);
    assert_eq!(mcu_message.channel_id, ChannelId::DEFAULT);
    assert_eq!(mcu_message.as_bytes(), b"host ping");

    mcu.send_on(ChannelId::LOG, b"mcu received host message")
        .expect("queue mcu debug");

    deliver_next_write(&mut mcu, &mut host);

    let debug = next_message(&mut host);
    assert_eq!(debug.channel_id, ChannelId::LOG);
    assert_eq!(debug.as_bytes(), b"mcu received host message");

    assert_no_send_failed(&mut host);
    assert_no_send_failed(&mut mcu);
}

#[test]
fn split_serial_reads_still_ack_and_debug_without_adapter() {
    let mut host = Engine::new(Config::default());
    let mut mcu = Engine::new(Config::default());

    host.send(b"host ping").expect("queue host message");

    let host_write = next_write(&mut host);
    for byte in host_write.as_bytes() {
        let _ = mcu.receive(core::slice::from_ref(byte));
    }

    let ack = next_write(&mut mcu);
    for byte in ack.as_bytes() {
        let _ = host.receive(core::slice::from_ref(byte));
    }

    let mcu_message = next_message(&mut mcu);
    assert_eq!(mcu_message.as_bytes(), b"host ping");

    mcu.send_on(ChannelId::LOG, b"mcu received host message")
        .expect("queue mcu debug");
    let debug_write = next_write(&mut mcu);
    for byte in debug_write.as_bytes() {
        let _ = host.receive(core::slice::from_ref(byte));
    }

    let debug = next_message(&mut host);
    assert_eq!(debug.channel_id, ChannelId::LOG);
    assert_eq!(debug.as_bytes(), b"mcu received host message");

    assert_no_send_failed(&mut host);
    assert_no_send_failed(&mut mcu);
}

fn deliver_next_write(src: &mut Engine, dst: &mut Engine) {
    let write = next_write(src);
    match dst.receive(write.as_bytes()) {
        Receive::Packet { .. } | Receive::Ack { .. } => {}
        other => panic!("unexpected receive report: {other:?}"),
    }
}

fn next_write(engine: &mut Engine) -> msrt::Write {
    let Some(Event::Write(write)) = engine.poll_event() else {
        panic!("engine should produce a write event");
    };

    write
}

fn next_message(engine: &mut Engine) -> msrt::Message {
    loop {
        let Some(event) = engine.poll_event() else {
            panic!("engine should produce a message event");
        };

        match event {
            Event::Message(message) => return message,
            Event::Write(_) => {}
            Event::SendFailed(failed) => panic!("unexpected send failure: {failed:?}"),
        }
    }
}

fn assert_no_send_failed(engine: &mut Engine) {
    while let Some(event) = engine.poll_event() {
        if let Event::SendFailed(failed) = event {
            panic!("unexpected send failure: {failed:?}");
        }
    }
}
