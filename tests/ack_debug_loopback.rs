use msrt::endpoint::{ClientEndpoint, EndpointPoll, MessageEvent, PassiveEndpoint, ReceiveReport};

const TX_BUF_BYTES: usize = 128;

#[test]
fn default_message_is_acked_without_adapter() {
    let mut host = ClientEndpoint::default();
    let mut mcu = PassiveEndpoint::default();

    host.connect(0).expect("connect host");
    complete_hello(&mut host, &mut mcu);

    host.send(b"host ping").expect("queue host message");
    deliver_next_host_write(&mut host, &mut mcu);
    deliver_next_mcu_write(&mut mcu, &mut host);

    let mcu_message = next_mcu_message(&mut mcu);
    assert_eq!(mcu_message.as_bytes(), b"host ping");

    assert_no_send_failed(&mut host, &mut mcu);
}

#[test]
fn mcu_can_reply_after_receiving_default_message_without_adapter() {
    let mut host = ClientEndpoint::default();
    let mut mcu = PassiveEndpoint::default();

    host.connect(0).expect("connect host");
    complete_hello(&mut host, &mut mcu);

    host.send(b"host ping").expect("queue host message");
    deliver_next_host_write(&mut host, &mut mcu);
    deliver_next_mcu_write(&mut mcu, &mut host);
    let mcu_message = next_mcu_message(&mut mcu);
    assert_eq!(mcu_message.as_bytes(), b"host ping");

    mcu.send(b"mcu received host message")
        .expect("queue mcu reply");
    deliver_next_mcu_write(&mut mcu, &mut host);

    let reply = next_host_message(&mut host);
    assert_eq!(reply.as_bytes(), b"mcu received host message");

    assert_no_send_failed(&mut host, &mut mcu);
}

#[test]
fn split_serial_reads_still_ack_and_deliver_message_without_adapter() {
    let mut host = ClientEndpoint::default();
    let mut mcu = PassiveEndpoint::default();

    host.connect(0).expect("connect host");
    complete_hello(&mut host, &mut mcu);

    host.send(b"host ping").expect("queue host message");
    let host_write = next_host_write(&mut host);
    for byte in host_write.as_bytes() {
        let _ = mcu.receive(0, core::slice::from_ref(byte));
    }

    let ack = next_mcu_write(&mut mcu);
    for byte in ack.as_bytes() {
        let _ = host.receive(0, core::slice::from_ref(byte));
    }

    let mcu_message = next_mcu_message(&mut mcu);
    assert_eq!(mcu_message.as_bytes(), b"host ping");

    mcu.send(b"mcu received host message")
        .expect("queue mcu reply");
    let reply_write = next_mcu_write(&mut mcu);
    for byte in reply_write.as_bytes() {
        let _ = host.receive(0, core::slice::from_ref(byte));
    }

    let reply = next_host_message(&mut host);
    assert_eq!(reply.as_bytes(), b"mcu received host message");

    assert_no_send_failed(&mut host, &mut mcu);
}

fn complete_hello(host: &mut ClientEndpoint, mcu: &mut PassiveEndpoint) {
    deliver_next_host_write(host, mcu);
    deliver_next_mcu_write(mcu, host);
    assert_eq!(next_mcu_message(mcu).as_bytes(), &[0]);
}

fn deliver_next_host_write(src: &mut ClientEndpoint, dst: &mut PassiveEndpoint) {
    let write = next_host_write(src);
    receive_ok(dst.receive(0, write.as_bytes()));
}

fn deliver_next_mcu_write(src: &mut PassiveEndpoint, dst: &mut ClientEndpoint) {
    let write = next_mcu_write(src);
    receive_ok(dst.receive(0, write.as_bytes()));
}

fn next_host_write(endpoint: &mut ClientEndpoint) -> TestWrite {
    let mut tx_buf = [0; TX_BUF_BYTES];
    let bytes = next_host_transmit(endpoint, &mut tx_buf);
    TestWrite::from_bytes(bytes)
}

fn next_mcu_write(endpoint: &mut PassiveEndpoint) -> TestWrite {
    let mut tx_buf = [0; TX_BUF_BYTES];
    let bytes = next_mcu_transmit(endpoint, &mut tx_buf);
    TestWrite::from_bytes(bytes)
}

fn next_host_message(endpoint: &mut ClientEndpoint) -> MessageEvent {
    let mut tx_buf = [0; TX_BUF_BYTES];
    loop {
        match endpoint.poll(0, &mut tx_buf).expect("poll client") {
            EndpointPoll::Message(message) => return message,
            EndpointPoll::Transmit { .. } => {}
            EndpointPoll::SendFailed(failed) => panic!("unexpected send failure: {failed:?}"),
            EndpointPoll::Idle => panic!("client should produce a message event"),
        }
    }
}

fn next_mcu_message(endpoint: &mut PassiveEndpoint) -> MessageEvent {
    let mut tx_buf = [0; TX_BUF_BYTES];
    loop {
        match endpoint.poll(0, &mut tx_buf).expect("poll passive") {
            EndpointPoll::Message(message) => return message,
            EndpointPoll::Transmit { .. } => {}
            EndpointPoll::SendFailed(failed) => panic!("unexpected send failure: {failed:?}"),
            EndpointPoll::Idle => panic!("passive endpoint should produce a message event"),
        }
    }
}

fn assert_no_send_failed(host: &mut ClientEndpoint, mcu: &mut PassiveEndpoint) {
    let mut tx_buf = [0; TX_BUF_BYTES];

    loop {
        match host.poll(0, &mut tx_buf).expect("poll client") {
            EndpointPoll::SendFailed(failed) => panic!("unexpected host send failure: {failed:?}"),
            EndpointPoll::Idle => break,
            EndpointPoll::Transmit { .. } | EndpointPoll::Message(_) => {}
        }
    }

    loop {
        match mcu.poll(0, &mut tx_buf).expect("poll passive") {
            EndpointPoll::SendFailed(failed) => panic!("unexpected mcu send failure: {failed:?}"),
            EndpointPoll::Idle => break,
            EndpointPoll::Transmit { .. } | EndpointPoll::Message(_) => {}
        }
    }
}

fn next_host_transmit<'a>(endpoint: &mut ClientEndpoint, tx_buf: &'a mut [u8]) -> &'a [u8] {
    match endpoint.poll(0, tx_buf).expect("poll client") {
        EndpointPoll::Transmit { bytes, .. } => bytes,
        other => panic!("client should produce transmit bytes, got {other:?}"),
    }
}

fn next_mcu_transmit<'a>(endpoint: &mut PassiveEndpoint, tx_buf: &'a mut [u8]) -> &'a [u8] {
    match endpoint.poll(0, tx_buf).expect("poll passive") {
        EndpointPoll::Transmit { bytes, .. } => bytes,
        other => panic!("passive endpoint should produce transmit bytes, got {other:?}"),
    }
}

fn receive_ok(report: ReceiveReport) {
    match report {
        ReceiveReport::Packet { .. }
        | ReceiveReport::Ack { .. }
        | ReceiveReport::Duplicate { .. }
        | ReceiveReport::Ping
        | ReceiveReport::Pong => {}
        other => panic!("unexpected receive report: {other:?}"),
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
