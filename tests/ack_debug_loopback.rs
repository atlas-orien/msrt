use msrt::{
    Engine,
    core::PacketType,
    engine::{EnginePoll, MessageEvent, ReceiveReport},
};

const TX_BUF_BYTES: usize = 128;

#[test]
fn default_message_is_acked_without_adapter() {
    let mut host = Engine::default();
    let mut mcu = Engine::default();

    host.send(b"host ping").expect("queue host message");

    let host_write = next_write(&mut host);
    assert!(matches!(
        mcu.receive(host_write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));

    let ack = next_write(&mut mcu);
    assert!(matches!(
        host.receive(ack.as_bytes()),
        ReceiveReport::Ack { .. }
    ));

    let mcu_message = next_message(&mut mcu);
    assert_eq!(mcu_message.packet_type, PacketType::Data);
    assert_eq!(mcu_message.as_bytes(), b"host ping");

    assert_no_send_failed(&mut host);
}

#[test]
fn mcu_can_debug_after_receiving_default_message_without_adapter() {
    let mut host = Engine::default();
    let mut mcu = Engine::default();

    host.send(b"host ping").expect("queue host message");
    deliver_next_write(&mut host, &mut mcu);

    deliver_next_write(&mut mcu, &mut host);

    let mcu_message = next_message(&mut mcu);
    assert_eq!(mcu_message.packet_type, PacketType::Data);
    assert_eq!(mcu_message.as_bytes(), b"host ping");

    mcu.send_log(b"mcu received host message")
        .expect("queue mcu debug");

    deliver_next_write(&mut mcu, &mut host);

    let debug = next_message(&mut host);
    assert_eq!(debug.packet_type, PacketType::Log);
    assert_eq!(debug.as_bytes(), b"mcu received host message");

    assert_no_send_failed(&mut host);
    assert_no_send_failed(&mut mcu);
}

#[test]
fn split_serial_reads_still_ack_and_debug_without_adapter() {
    let mut host = Engine::default();
    let mut mcu = Engine::default();

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

    mcu.send_log(b"mcu received host message")
        .expect("queue mcu debug");
    let debug_write = next_write(&mut mcu);
    for byte in debug_write.as_bytes() {
        let _ = host.receive(core::slice::from_ref(byte));
    }

    let debug = next_message(&mut host);
    assert_eq!(debug.packet_type, PacketType::Log);
    assert_eq!(debug.as_bytes(), b"mcu received host message");

    assert_no_send_failed(&mut host);
    assert_no_send_failed(&mut mcu);
}

#[test]
fn poll_copies_transmit_bytes_into_external_buffer() {
    let mut host = Engine::default();
    let mut mcu = Engine::default();
    let mut host_tx = [0; TX_BUF_BYTES];
    let mut mcu_tx = [0; TX_BUF_BYTES];

    host.send(b"host ping").expect("queue host message");

    let bytes = next_transmit(&mut host, &mut host_tx);
    assert!(matches!(mcu.receive(bytes), ReceiveReport::Packet { .. }));

    let bytes = next_transmit(&mut mcu, &mut mcu_tx);
    assert!(matches!(host.receive(bytes), ReceiveReport::Ack { .. }));

    let message = next_polled_message(&mut mcu, &mut mcu_tx);
    assert_eq!(message.packet_type, PacketType::Data);
    assert_eq!(message.as_bytes(), b"host ping");

    mcu.send_log(b"mcu received host message")
        .expect("queue mcu debug");

    let bytes = next_transmit(&mut mcu, &mut mcu_tx);
    assert!(matches!(host.receive(bytes), ReceiveReport::Packet { .. }));

    let message = next_polled_message(&mut host, &mut host_tx);
    assert_eq!(message.packet_type, PacketType::Log);
    assert_eq!(message.as_bytes(), b"mcu received host message");
}

fn deliver_next_write(src: &mut Engine, dst: &mut Engine) {
    let write = next_write(src);
    match dst.receive(write.as_bytes()) {
        ReceiveReport::Packet { .. }
        | ReceiveReport::Ack { .. }
        | ReceiveReport::Ping
        | ReceiveReport::Pong => {}
        other => panic!("unexpected receive report: {other:?}"),
    }
}

fn next_write(engine: &mut Engine) -> TestWrite {
    let mut tx_buf = [0; TX_BUF_BYTES];
    let bytes = next_transmit(engine, &mut tx_buf);
    TestWrite::from_bytes(bytes)
}

fn next_message(engine: &mut Engine) -> MessageEvent {
    let mut tx_buf = [0; TX_BUF_BYTES];
    next_polled_message(engine, &mut tx_buf)
}

fn assert_no_send_failed(engine: &mut Engine) {
    let mut tx_buf = [0; TX_BUF_BYTES];

    loop {
        match engine.poll(0, &mut tx_buf).expect("poll engine") {
            EnginePoll::SendFailed(failed) => panic!("unexpected send failure: {failed:?}"),
            EnginePoll::Idle => break,
            EnginePoll::Transmit { .. } | EnginePoll::Message(_) => {}
        }
    }
}

fn next_transmit<'a>(engine: &mut Engine, tx_buf: &'a mut [u8]) -> &'a [u8] {
    match engine.poll(0, tx_buf).expect("poll engine") {
        EnginePoll::Transmit { bytes, .. } => bytes,
        other => panic!("engine should produce transmit bytes, got {other:?}"),
    }
}

fn next_polled_message(engine: &mut Engine, tx_buf: &mut [u8]) -> MessageEvent {
    loop {
        match engine.poll(0, tx_buf).expect("poll engine") {
            EnginePoll::Message(message) => return message,
            EnginePoll::Transmit { .. } => {}
            EnginePoll::SendFailed(failed) => panic!("unexpected send failure: {failed:?}"),
            EnginePoll::Idle => panic!("engine should produce a message event"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TestWrite {
    bytes: [u8; TX_BUF_BYTES],
    len: usize,
}

impl TestWrite {
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut stored = [0; TX_BUF_BYTES];
        stored[..bytes.len()].copy_from_slice(bytes);

        Self {
            bytes: stored,
            len: bytes.len(),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}
