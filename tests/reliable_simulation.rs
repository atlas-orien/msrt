//! Deterministic long-run reliable transport simulation.

use msrt::{ChannelId, Config, Engine, Event, MAX_WIRE_BYTES, Receive, Write, core::PacketType};

#[test]
fn reliable_transport_survives_drops_corruption_reordering_and_duplex_load() {
    let mut mac = Engine::new(Config {
        fragment_bytes: 8,
        retransmit_timeout_ms: 1,
        ..Config::default()
    });
    let mut mcu = Engine::new(Config {
        fragment_bytes: 9,
        retransmit_timeout_ms: 1,
        ..Config::default()
    });
    let nav = ChannelId::new(16);
    let telemetry = ChannelId::new(17);
    let mac_messages = [
        ExpectedMessage::new(nav, b"mac nav message one"),
        ExpectedMessage::new(telemetry, b"mac telemetry message two"),
        ExpectedMessage::new(nav, b"mac command message three"),
    ];
    let mcu_messages = [
        ExpectedMessage::new(telemetry, b"mcu telemetry response one"),
        ExpectedMessage::new(nav, b"mcu nav response two"),
        ExpectedMessage::new(telemetry, b"mcu status response three"),
    ];
    let mut link = SimLink::new();
    let mut mac_delivered = DeliveredMessages::new();
    let mut mcu_delivered = DeliveredMessages::new();

    for message in mac_messages {
        mac.send_on(message.channel_id, message.bytes)
            .expect("queue mac message");
    }

    for message in mcu_messages {
        mcu.send_on(message.channel_id, message.bytes)
            .expect("queue mcu message");
    }

    for now_ms in 0..2_000 {
        pump_until_idle(
            &mut mac,
            &mut mcu,
            &mut link,
            &mut mac_delivered,
            &mut mcu_delivered,
        );
        link.flush_reordered(&mut mac, &mut mcu);
        pump_until_idle(
            &mut mac,
            &mut mcu,
            &mut link,
            &mut mac_delivered,
            &mut mcu_delivered,
        );

        if mac_delivered.contains_all(&mcu_messages) && mcu_delivered.contains_all(&mac_messages) {
            assert_no_unexpected_events(&mut mac);
            assert_no_unexpected_events(&mut mcu);
            return;
        }

        mac.tick(now_ms + 1);
        mcu.tick(now_ms + 1);
    }

    panic!(
        "simulation did not deliver all messages: mac={:?}, mcu={:?}",
        mac_delivered, mcu_delivered
    );
}

fn pump_until_idle(
    mac: &mut Engine,
    mcu: &mut Engine,
    link: &mut SimLink,
    mac_delivered: &mut DeliveredMessages,
    mcu_delivered: &mut DeliveredMessages,
) {
    for _ in 0..256 {
        let mut progressed = false;

        progressed |= pump_one(mac, mcu, &mut link.mac_to_mcu, mac_delivered);
        progressed |= pump_one(mcu, mac, &mut link.mcu_to_mac, mcu_delivered);

        if !progressed {
            break;
        }
    }
}

fn pump_one(
    src: &mut Engine,
    dst: &mut Engine,
    direction: &mut SimDirection,
    delivered: &mut DeliveredMessages,
) -> bool {
    match src.poll_event() {
        Some(Event::Write(write)) => {
            direction.deliver(dst, write);
            true
        }
        Some(Event::Message(message)) => {
            delivered.push(message.channel_id, message.as_bytes());
            true
        }
        Some(Event::SendFailed(failed)) => {
            panic!("reliable simulation should not fail sends: {failed:?}");
        }
        None => false,
    }
}

fn assert_no_unexpected_events(engine: &mut Engine) {
    while let Some(event) = engine.poll_event() {
        match event {
            Event::Write(_) | Event::Message(_) => {}
            Event::SendFailed(failed) => {
                panic!("simulation completed but found send failure: {failed:?}");
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedMessage {
    channel_id: ChannelId,
    bytes: &'static [u8],
}

impl ExpectedMessage {
    const fn new(channel_id: ChannelId, bytes: &'static [u8]) -> Self {
        Self { channel_id, bytes }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeliveredMessages {
    messages: [(ChannelId, [u8; 64], usize); 8],
    len: usize,
}

impl DeliveredMessages {
    const fn new() -> Self {
        Self {
            messages: [(ChannelId::DEFAULT, [0; 64], 0); 8],
            len: 0,
        }
    }

    fn push(&mut self, channel_id: ChannelId, bytes: &[u8]) {
        assert!(
            self.len < self.messages.len(),
            "delivered message buffer full"
        );
        assert!(bytes.len() <= 64, "delivered message too large for test");

        let mut stored = [0; 64];
        stored[..bytes.len()].copy_from_slice(bytes);
        self.messages[self.len] = (channel_id, stored, bytes.len());
        self.len += 1;
    }

    fn contains_all(&self, expected: &[ExpectedMessage]) -> bool {
        expected
            .iter()
            .all(|message| self.contains(message.channel_id, message.bytes))
    }

    fn contains(&self, channel_id: ChannelId, bytes: &[u8]) -> bool {
        self.messages[..self.len]
            .iter()
            .any(|(current_channel, current_bytes, current_len)| {
                *current_channel == channel_id && &current_bytes[..*current_len] == bytes
            })
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

    fn flush_reordered(&mut self, mac: &mut Engine, mcu: &mut Engine) {
        self.mac_to_mcu.flush(mcu);
        self.mcu_to_mac.flush(mac);
    }
}

#[derive(Debug)]
struct SimDirection {
    seen_data: [bool; 64],
    held: [Option<Write>; 8],
    held_len: usize,
}

impl SimDirection {
    const fn new() -> Self {
        Self {
            seen_data: [false; 64],
            held: [None; 8],
            held_len: 0,
        }
    }

    fn deliver(&mut self, dst: &mut Engine, write: Write) {
        if !is_data(write) {
            receive_ok(dst, write.as_bytes());
            return;
        }

        let packet_number = write.packet_number.get() as usize;
        let first_seen = packet_number < self.seen_data.len() && !self.seen_data[packet_number];

        if packet_number < self.seen_data.len() {
            self.seen_data[packet_number] = true;
        }

        if first_seen && packet_number % 7 == 2 {
            let mut corrupted = [0; MAX_WIRE_BYTES];
            let len = write.as_bytes().len();

            corrupted[..len].copy_from_slice(write.as_bytes());
            corrupted[len - 1] ^= 0xff;
            assert!(matches!(dst.receive(&corrupted[..len]), Receive::Corrupted));
            return;
        }

        if first_seen && packet_number % 5 == 1 {
            return;
        }

        if first_seen && packet_number % 4 == 3 {
            self.hold(write);
            return;
        }

        receive_ok(dst, write.as_bytes());
    }

    fn hold(&mut self, write: Write) {
        assert!(self.held_len < self.held.len(), "held packet buffer full");

        self.held[self.held_len] = Some(write);
        self.held_len += 1;
    }

    fn flush(&mut self, dst: &mut Engine) {
        while self.held_len > 0 {
            self.held_len -= 1;
            let write = self.held[self.held_len].take().expect("held packet");

            receive_ok(dst, write.as_bytes());
        }
    }
}

fn receive_ok(dst: &mut Engine, bytes: &[u8]) {
    match dst.receive(bytes) {
        Receive::Packet { .. } | Receive::Duplicate { .. } | Receive::Ack { .. } => {}
        other => panic!("unexpected receive report in simulation: {other:?}"),
    }
}

fn is_data(write: Write) -> bool {
    write.as_bytes().get(8).copied() == Some(PacketType::Data.code())
}
