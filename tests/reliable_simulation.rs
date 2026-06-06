//! Deterministic long-run reliable transport simulation through the public endpoint API.

use msrt::endpoint::{ClientEndpoint, EndpointPoll, EngineConfig, PassiveEndpoint, ReceiveReport};

const TX_BUF_BYTES: usize = 128;

#[test]
fn reliable_transport_survives_drops_corruption_reordering_and_duplex_load() {
    let mut mac = ClientEndpoint::new(EngineConfig {
        fragment_bytes: 8,
        retransmit_timeout_ms: 1,
        ..EngineConfig::default()
    });
    let mut mcu = PassiveEndpoint::new(EngineConfig {
        fragment_bytes: 9,
        retransmit_timeout_ms: 1,
        ..EngineConfig::default()
    });
    let mac_messages = [
        ExpectedMessage::new(b"mac nav message one"),
        ExpectedMessage::new(b"mac telemetry message two"),
        ExpectedMessage::new(b"mac command message three"),
    ];
    let mcu_messages = [
        ExpectedMessage::new(b"mcu telemetry response one"),
        ExpectedMessage::new(b"mcu nav response two"),
        ExpectedMessage::new(b"mcu status response three"),
    ];
    let mut link = SimLink::new();
    let mut mac_delivered = DeliveredMessages::new();
    let mut mcu_delivered = DeliveredMessages::new();

    mac.connect(0).expect("client connect");
    for message in mac_messages {
        mac.send(message.bytes).expect("queue mac message");
    }

    for _ in 0..64 {
        pump_until_idle(
            &mut mac,
            &mut mcu,
            &mut link,
            &mut mac_delivered,
            &mut mcu_delivered,
        );

        if mcu.peer().is_connected() {
            for message in mcu_messages {
                if !mcu_delivered.contains(message.bytes) {
                    mcu.send(message.bytes).expect("queue mcu message");
                }
            }
        }

        link.flush_reordered(&mut mac, &mut mcu);
        pump_until_idle(
            &mut mac,
            &mut mcu,
            &mut link,
            &mut mac_delivered,
            &mut mcu_delivered,
        );

        if mac_delivered.contains_all(&mcu_messages) && mcu_delivered.contains_all(&mac_messages) {
            assert_no_unexpected_events(&mut mac, &mut mcu);
            return;
        }
    }

    panic!(
        "simulation did not deliver all messages: mac={:?}, mcu={:?}",
        mac_delivered, mcu_delivered
    );
}

fn pump_until_idle(
    mac: &mut ClientEndpoint,
    mcu: &mut PassiveEndpoint,
    link: &mut SimLink,
    mac_delivered: &mut DeliveredMessages,
    mcu_delivered: &mut DeliveredMessages,
) {
    for _ in 0..256 {
        let mut progressed = false;

        progressed |= pump_mac(mac, mcu, &mut link.mac_to_mcu, mac_delivered);
        progressed |= pump_mcu(mcu, mac, &mut link.mcu_to_mac, mcu_delivered);

        if !progressed {
            break;
        }
    }
}

fn pump_mac(
    src: &mut ClientEndpoint,
    dst: &mut PassiveEndpoint,
    direction: &mut SimDirection,
    delivered: &mut DeliveredMessages,
) -> bool {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match src.poll(0, &mut tx_buf).expect("poll client") {
        EndpointPoll::Transmit { bytes, .. } => {
            direction.deliver_to_passive(dst, SimWrite::from_bytes(bytes));
            true
        }
        EndpointPoll::Message(message) => {
            delivered.push(message.as_bytes());
            true
        }
        EndpointPoll::SendFailed(failed) => {
            panic!("reliable simulation should not fail sends: {failed:?}");
        }
        EndpointPoll::Idle => false,
    }
}

fn pump_mcu(
    src: &mut PassiveEndpoint,
    dst: &mut ClientEndpoint,
    direction: &mut SimDirection,
    delivered: &mut DeliveredMessages,
) -> bool {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match src.poll(0, &mut tx_buf).expect("poll passive") {
        EndpointPoll::Transmit { bytes, .. } => {
            direction.deliver_to_client(dst, SimWrite::from_bytes(bytes));
            true
        }
        EndpointPoll::Message(message) => {
            delivered.push(message.as_bytes());
            true
        }
        EndpointPoll::SendFailed(failed) => {
            panic!("reliable simulation should not fail sends: {failed:?}");
        }
        EndpointPoll::Idle => false,
    }
}

fn assert_no_unexpected_events(mac: &mut ClientEndpoint, mcu: &mut PassiveEndpoint) {
    let mut tx_buf = [0; TX_BUF_BYTES];

    loop {
        match mac.poll(0, &mut tx_buf).expect("poll client") {
            EndpointPoll::Transmit { .. } | EndpointPoll::Message(_) => {}
            EndpointPoll::SendFailed(failed) => {
                panic!("simulation completed but found client send failure: {failed:?}");
            }
            EndpointPoll::Idle => break,
        }
    }

    loop {
        match mcu.poll(0, &mut tx_buf).expect("poll passive") {
            EndpointPoll::Transmit { .. } | EndpointPoll::Message(_) => {}
            EndpointPoll::SendFailed(failed) => {
                panic!("simulation completed but found passive send failure: {failed:?}");
            }
            EndpointPoll::Idle => break,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedMessage {
    bytes: &'static [u8],
}

impl ExpectedMessage {
    const fn new(bytes: &'static [u8]) -> Self {
        Self { bytes }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeliveredMessages {
    messages: [([u8; 64], usize); 8],
    len: usize,
}

impl DeliveredMessages {
    const fn new() -> Self {
        Self {
            messages: [([0; 64], 0); 8],
            len: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        assert!(
            self.len < self.messages.len(),
            "delivered message buffer full"
        );
        assert!(bytes.len() <= 64, "delivered message too large for test");

        if self.contains(bytes) {
            return;
        }

        let mut stored = [0; 64];
        stored[..bytes.len()].copy_from_slice(bytes);
        self.messages[self.len] = (stored, bytes.len());
        self.len += 1;
    }

    fn contains_all(&self, expected: &[ExpectedMessage]) -> bool {
        expected.iter().all(|message| self.contains(message.bytes))
    }

    fn contains(&self, bytes: &[u8]) -> bool {
        self.messages[..self.len]
            .iter()
            .any(|(current_bytes, current_len)| &current_bytes[..*current_len] == bytes)
    }
}

#[derive(Debug)]
struct SimLink {
    mac_to_mcu: SimDirection,
    mcu_to_mac: SimDirection,
}

impl SimLink {
    const fn new() -> Self {
        Self {
            mac_to_mcu: SimDirection::new(),
            mcu_to_mac: SimDirection::new(),
        }
    }

    fn flush_reordered(&mut self, mac: &mut ClientEndpoint, mcu: &mut PassiveEndpoint) {
        self.mac_to_mcu.flush_to_passive(mcu);
        self.mcu_to_mac.flush_to_client(mac);
    }
}

#[derive(Debug)]
struct SimDirection {
    held: [Option<SimWrite>; 8],
    held_len: usize,
    delivered: usize,
}

impl SimDirection {
    const fn new() -> Self {
        Self {
            held: [None; 8],
            held_len: 0,
            delivered: 0,
        }
    }

    fn deliver_to_passive(&mut self, dst: &mut PassiveEndpoint, write: SimWrite) {
        self.delivered += 1;

        if self.delivered.is_multiple_of(7) {
            self.hold(write);
            return;
        }

        receive_ok(dst.receive(0, write.as_bytes()));
    }

    fn deliver_to_client(&mut self, dst: &mut ClientEndpoint, write: SimWrite) {
        self.delivered += 1;

        if self.delivered.is_multiple_of(7) {
            self.hold(write);
            return;
        }

        receive_ok(dst.receive(0, write.as_bytes()));
    }

    fn hold(&mut self, write: SimWrite) {
        assert!(self.held_len < self.held.len(), "held packet buffer full");

        self.held[self.held_len] = Some(write);
        self.held_len += 1;
    }

    fn flush_to_passive(&mut self, dst: &mut PassiveEndpoint) {
        while self.held_len > 0 {
            self.held_len -= 1;
            let write = self.held[self.held_len].take().expect("held packet");

            receive_ok(dst.receive(0, write.as_bytes()));
        }
    }

    fn flush_to_client(&mut self, dst: &mut ClientEndpoint) {
        while self.held_len > 0 {
            self.held_len -= 1;
            let write = self.held[self.held_len].take().expect("held packet");

            receive_ok(dst.receive(0, write.as_bytes()));
        }
    }
}

fn receive_ok(report: ReceiveReport) {
    match report {
        ReceiveReport::Packet { .. }
        | ReceiveReport::Duplicate { .. }
        | ReceiveReport::Ack { .. }
        | ReceiveReport::Ping
        | ReceiveReport::Pong => {}
        other => panic!("unexpected receive report in simulation: {other:?}"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SimWrite {
    bytes: [u8; TX_BUF_BYTES],
    len: usize,
}

impl SimWrite {
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
