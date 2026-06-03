use crate::engine::machine::{EngineOutput, Machine, WriteEvent};
use crate::engine::{
    ChannelProfile, ChannelSpec, Engine, EngineConfig, EnginePoll, MessageEvent, ReceiveReport,
    SendFailedEvent, SendFailureReason,
};
use crate::reliability::ChannelReliability;

#[test]
fn engine_sends_one_message_as_multiple_write_events() {
    let mut engine = Engine::new(EngineConfig {
        fragment_bytes: 5,
        ..EngineConfig::default()
    });

    let message_id = engine.send(b"hello msrt testing").unwrap();
    let mut writes = 0;

    while let Some(event) = Machine::poll_event(&mut engine) {
        match event {
            EngineOutput::Write(_) => writes += 1,
            EngineOutput::Message(_) => panic!("sender should not receive its own message"),
            EngineOutput::SendFailed(failed) => {
                panic!("sender should not fail in this test: {failed:?}");
            }
        }
    }

    assert_eq!(message_id.get(), 0);
    assert_eq!(writes, 4);
}

#[test]
fn engine_receives_fragments_as_one_message_event() {
    let mut a = Engine::new(EngineConfig {
        fragment_bytes: 5,
        ..EngineConfig::default()
    });
    let mut b = Engine::new(EngineConfig::default());

    a.send(b"hello msrt testing").unwrap();

    while let Some(event) = Machine::poll_event(&mut a) {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        assert!(matches!(
            b.receive(write.as_bytes()),
            ReceiveReport::Packet { .. } | ReceiveReport::Ack { .. }
        ));
    }

    while let Some(event) = Machine::poll_event(&mut b) {
        if let EngineOutput::Message(message) = event {
            assert_eq!(message.as_bytes(), b"hello msrt testing");
            return;
        }
    }

    panic!("receiver should emit a complete message");
}

#[test]
fn engine_reassembles_interleaved_messages() {
    let mut a = Engine::new(EngineConfig {
        fragment_bytes: 2,
        ..EngineConfig::default()
    });
    let mut b = Engine::new(EngineConfig::default());
    let mut writes = [None; 4];
    let mut write_count = 0;

    a.send(b"abcd").unwrap();
    a.send(b"wxyz").unwrap();

    while let Some(event) = Machine::poll_event(&mut a) {
        let EngineOutput::Write(write) = event else {
            continue;
        };
        writes[write_count] = Some(write);
        write_count += 1;
    }

    assert_eq!(write_count, 4);

    for index in [0, 2, 1, 3] {
        let write = writes[index].expect("write should be captured");
        assert!(matches!(
            b.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));
    }

    assert_message(&mut b, b"abcd");
    assert_message(&mut b, b"wxyz");
}

#[test]
fn engine_rejects_fragment_when_reassembly_budget_is_full() {
    let mut a = Engine::new(EngineConfig {
        fragment_bytes: 2,
        ..EngineConfig::default()
    });
    let mut b = Engine::new(EngineConfig::default());
    let writes = first_fragments_for_five_messages(&mut a);

    for write in &writes[..crate::engine::config::MAX_REASSEMBLY_MESSAGES] {
        assert!(matches!(
            b.receive(write.expect("fragment").as_bytes()),
            ReceiveReport::Packet { .. }
        ));
    }

    assert!(matches!(
        b.receive(
            writes[crate::engine::config::MAX_REASSEMBLY_MESSAGES]
                .expect("extra fragment")
                .as_bytes()
        ),
        ReceiveReport::Error(_)
    ));
}

#[test]
fn engine_reassembly_timeout_releases_slot() {
    let mut a = Engine::new(EngineConfig {
        fragment_bytes: 2,
        ..EngineConfig::default()
    });
    let mut b = Engine::new(EngineConfig {
        reassembly_timeout_ms: 10,
        ..EngineConfig::default()
    });
    let writes = first_fragments_for_five_messages(&mut a);

    for write in &writes[..crate::engine::config::MAX_REASSEMBLY_MESSAGES] {
        assert!(matches!(
            b.receive(write.expect("fragment").as_bytes()),
            ReceiveReport::Packet { .. }
        ));
    }

    let _ = next_polled_write(&mut b, 10);

    assert!(matches!(
        b.receive(
            writes[crate::engine::config::MAX_REASSEMBLY_MESSAGES]
                .expect("extra fragment")
                .as_bytes()
        ),
        ReceiveReport::Packet { .. }
    ));
}

#[test]
fn engine_send_on_uses_channel_id() {
    let mut a = Engine::new(EngineConfig::default());
    let mut b = Engine::new(EngineConfig::default());
    let channel_id = crate::core::ChannelId::new(7);

    a.send_on(channel_id, b"hello").unwrap();

    let write = next_write(&mut a);
    let bytes = write.as_bytes();

    assert_eq!(bytes[15], channel_id.get());
    assert!(matches!(
        b.receive(write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));

    let message = next_message(&mut b);

    assert_eq!(message.channel_id, channel_id);
    assert_eq!(message.as_bytes(), b"hello");
}

#[test]
fn engine_ack_range_clears_multiple_in_flight_packets() {
    let mut a = Engine::new(EngineConfig {
        fragment_bytes: 2,
        ..EngineConfig::default()
    });
    let mut b = Engine::new(EngineConfig::default());
    let mut last_ack = None;

    a.send(b"abcdef").unwrap();

    while let Some(event) = Machine::poll_event(&mut a) {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        assert!(matches!(
            b.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));
    }

    while let Some(event) = Machine::poll_event(&mut b) {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        last_ack = Some(write);
    }

    let last_ack = last_ack.expect("receiver should emit ACK range");

    assert!(matches!(
        a.receive(last_ack.as_bytes()),
        ReceiveReport::Ack { .. }
    ));

    poll_idle(&mut a, 1);
}

#[test]
fn engine_ack_range_gap_retransmits_only_missing_packet() {
    let mut engine = Engine::new(EngineConfig {
        fragment_bytes: 2,
        retransmit_timeout_ms: 1,
        ..EngineConfig::default()
    });

    engine.send(b"abcdefgh").unwrap();

    for expected in 0..4 {
        assert_eq!(next_write(&mut engine).packet_number.get(), expected);
    }

    let ack = ack_packet_for_ranges(
        &[
            (
                crate::core::PacketNumber::new(0),
                crate::core::PacketNumber::new(0),
            ),
            (
                crate::core::PacketNumber::new(2),
                crate::core::PacketNumber::new(3),
            ),
        ],
        crate::core::PacketNumber::new(100),
    );

    assert!(matches!(
        engine.receive(ack.as_bytes()),
        ReceiveReport::Ack { .. }
    ));

    assert_eq!(next_polled_write(&mut engine, 1).packet_number.get(), 1);
    assert!(Machine::poll_event(&mut engine).is_none());
}

#[test]
fn engine_tick_waits_for_retransmit_timeout() {
    let mut engine = Engine::new(EngineConfig {
        retransmit_timeout_ms: 10,
        ..EngineConfig::default()
    });

    engine.send(b"hello").unwrap();
    let first = next_write(&mut engine);

    poll_idle(&mut engine, 9);

    assert_eq!(
        next_polled_write(&mut engine, 10).packet_number,
        first.packet_number
    );

    poll_idle(&mut engine, 19);

    assert_eq!(
        next_polled_write(&mut engine, 20).packet_number,
        first.packet_number
    );
}

#[test]
fn engine_default_retransmit_timeout_does_not_retry_after_one_tick() {
    let mut engine = Engine::new(EngineConfig::default());

    engine.send(b"hello").unwrap();
    let _ = next_write(&mut engine);

    poll_idle(&mut engine, 1);
}

#[test]
fn engine_receives_half_packet() {
    let mut a = Engine::new(EngineConfig::default());
    let mut b = Engine::new(EngineConfig::default());

    a.send(b"hello").unwrap();
    let write = next_write(&mut a);
    let split = 3;

    assert_eq!(
        b.receive(&write.as_bytes()[..split]),
        ReceiveReport::Incomplete {
            needed: Some(crate::wire::WIRE_HEADER_LEN - split)
        }
    );
    assert!(matches!(
        b.receive(&write.as_bytes()[split..]),
        ReceiveReport::Packet { .. }
    ));
    assert_message(&mut b, b"hello");
}

#[test]
fn engine_receives_sticky_packets_and_multiple_packets_per_receive() {
    let mut a = Engine::new(EngineConfig {
        fragment_bytes: 5,
        ..EngineConfig::default()
    });
    let mut b = Engine::new(EngineConfig::default());
    let mut bytes = [0; crate::engine::config::MAX_WIRE_BYTES * 4];
    let mut len = 0;

    a.send(b"hello msrt testing").unwrap();

    while let Some(event) = Machine::poll_event(&mut a) {
        let EngineOutput::Write(write) = event else {
            continue;
        };
        let end = len + write.as_bytes().len();
        bytes[len..end].copy_from_slice(write.as_bytes());
        len = end;
    }

    assert!(matches!(
        b.receive(&bytes[..len]),
        ReceiveReport::Packet { .. }
    ));
    assert_message(&mut b, b"hello msrt testing");
}

#[test]
fn engine_acknowledges_duplicate_without_delivering_twice() {
    let mut a = Engine::new(EngineConfig::default());
    let mut b = Engine::new(EngineConfig::default());

    a.send(b"hello").unwrap();
    let write = next_write(&mut a);

    assert!(matches!(
        b.receive(write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));
    assert_message(&mut b, b"hello");
    assert!(matches!(
        b.receive(write.as_bytes()),
        ReceiveReport::Duplicate { .. }
    ));

    let mut duplicate_messages = 0;
    while let Some(event) = Machine::poll_event(&mut b) {
        if matches!(event, EngineOutput::Message(_)) {
            duplicate_messages += 1;
        }
    }

    assert_eq!(duplicate_messages, 0);
}

#[test]
fn engine_uses_greedy_fragmentation() {
    let mut engine = Engine::new(EngineConfig {
        fragment_bytes: 10,
        ..EngineConfig::default()
    });
    let mut fragment_lengths = [0; 2];
    let mut fragment_count = 0;

    engine.send(b"hello world").unwrap();

    while let Some(event) = Machine::poll_event(&mut engine) {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        fragment_lengths[fragment_count] = fragment_len_from_wire(write.as_bytes());
        fragment_count += 1;
    }

    assert_eq!(&fragment_lengths[..fragment_count], &[10, 1]);
}

#[test]
fn engine_encodes_v1_draft_packet_and_frame_headers() {
    let mut engine = Engine::new(EngineConfig::default());

    engine.send(b"hello").unwrap();

    let write = next_write(&mut engine);
    let bytes = write.as_bytes();

    assert_eq!(&bytes[..2], &crate::wire::EnvelopeMagic::MSRT.bytes());
    assert_eq!(bytes[8], crate::core::PacketType::Data.code());
    assert_eq!(bytes[9], crate::core::Flags::ACK_ELICITING.bits());
    assert_eq!(
        u32::from_le_bytes(bytes[10..14].try_into().unwrap()),
        write.packet_number.get()
    );
    assert_eq!(bytes[14], crate::core::FrameKind::Message.code());
    assert_eq!(bytes[15], crate::core::ChannelId::DEFAULT.get());
    assert_eq!(
        bytes[24],
        crate::core::MessageFlags::FIRST.bits() | crate::core::MessageFlags::LAST.bits()
    );
}

#[test]
fn engine_best_effort_channel_does_not_track_in_flight() {
    let channel_id = crate::core::ChannelId::new(9);
    let mut engine = Engine::new(EngineConfig {
        channel_policies: [
            Some(ChannelReliability::best_effort(channel_id)),
            None,
            None,
            None,
        ],
        ..EngineConfig::default()
    });

    engine.send_on(channel_id, b"hello best effort").unwrap();

    while let Some(event) = Machine::poll_event(&mut engine) {
        let EngineOutput::Write(_) = event else {
            panic!("best-effort send should only produce writes before tick");
        };
    }

    assert_eq!(engine.machine.in_flight.packets().count(), 0);

    poll_idle(&mut engine, 1);
}

#[test]
fn engine_best_effort_packet_is_not_ack_eliciting() {
    let channel_id = crate::core::ChannelId::new(9);
    let mut engine = Engine::new(EngineConfig {
        channel_policies: [
            Some(ChannelReliability::best_effort(channel_id)),
            None,
            None,
            None,
        ],
        ..EngineConfig::default()
    });

    engine.send_on(channel_id, b"hello").unwrap();

    let write = next_write(&mut engine);

    assert_eq!(write.as_bytes()[9], crate::core::Flags::EMPTY.bits());
}

#[test]
fn engine_receives_best_effort_without_ack() {
    let channel_id = crate::core::ChannelId::new(9);
    let mut sender = Engine::new(EngineConfig {
        channel_policies: [
            Some(ChannelReliability::best_effort(channel_id)),
            None,
            None,
            None,
        ],
        ..EngineConfig::default()
    });
    let mut receiver = Engine::new(EngineConfig::default());

    sender.send_on(channel_id, b"hello best effort").unwrap();

    let write = next_write(&mut sender);

    assert!(matches!(
        receiver.receive(write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));

    let Some(event) = Machine::poll_event(&mut receiver) else {
        panic!("receiver should emit the complete best-effort message");
    };
    let EngineOutput::Message(message) = event else {
        panic!("best-effort packet should not emit ACK before message");
    };

    assert_eq!(message.channel_id, channel_id);
    assert_eq!(message.profile, ChannelProfile::Data);
    assert_eq!(message.as_bytes(), b"hello best effort");
    assert!(Machine::poll_event(&mut receiver).is_none());
}

#[test]
fn engine_send_uses_default_application_channel() {
    let mut engine = Engine::new(EngineConfig::default());

    engine.send(b"hello default").unwrap();

    let write = next_write(&mut engine);
    let bytes = write.as_bytes();

    assert_eq!(bytes[15], crate::core::ChannelId::DEFAULT.get());
}

#[test]
fn engine_log_channel_defaults_to_best_effort_and_log_profile() {
    let mut sender = Engine::new(EngineConfig::default());
    let mut receiver = Engine::new(EngineConfig::default());

    sender
        .send_on(crate::core::ChannelId::LOG, b"log line")
        .unwrap();

    let write = next_write(&mut sender);

    assert_eq!(write.as_bytes()[9], crate::core::Flags::EMPTY.bits());
    assert_eq!(sender.machine.in_flight.packets().count(), 0);

    assert!(matches!(
        receiver.receive(write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));

    let message = next_message(&mut receiver);

    assert_eq!(message.channel_id, crate::core::ChannelId::LOG);
    assert_eq!(message.profile, ChannelProfile::Log);
    assert_eq!(message.as_bytes(), b"log line");
    assert!(Machine::poll_event(&mut receiver).is_none());
}

#[test]
fn engine_channel_spec_overrides_profile_and_reliability() {
    let channel_id = crate::core::ChannelId::new(16);
    let mut sender = Engine::new(EngineConfig {
        channel_specs: [Some(ChannelSpec::log(channel_id)), None, None, None],
        ..EngineConfig::default()
    });
    let mut receiver = Engine::new(EngineConfig {
        channel_specs: [Some(ChannelSpec::log(channel_id)), None, None, None],
        ..EngineConfig::default()
    });

    sender.send_on(channel_id, b"adapter log").unwrap();

    let write = next_write(&mut sender);

    assert_eq!(write.as_bytes()[9], crate::core::Flags::EMPTY.bits());
    assert_eq!(sender.machine.in_flight.packets().count(), 0);

    assert!(matches!(
        receiver.receive(write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));

    let message = next_message(&mut receiver);

    assert_eq!(message.channel_id, channel_id);
    assert_eq!(message.profile, ChannelProfile::Log);
    assert_eq!(message.as_bytes(), b"adapter log");
}

#[test]
fn engine_reports_send_failed_after_retry_limit() {
    let mut engine = Engine::new(EngineConfig {
        max_retransmit_attempts: 1,
        retransmit_timeout_ms: 1,
        ..EngineConfig::default()
    });

    let message_id = engine.send(b"hello").unwrap();
    let first = next_write(&mut engine);

    assert_eq!(first.packet_number.get(), 0);

    let retry = next_polled_write(&mut engine, 1);

    assert_eq!(retry.packet_number, first.packet_number);

    let failed = next_send_failed(&mut engine, 2);

    assert_eq!(failed.message_id, message_id);
    assert_eq!(failed.channel_id, crate::core::ChannelId::DEFAULT);
    assert_eq!(failed.reason, SendFailureReason::RetryLimitReached);
}

#[test]
fn engine_send_failed_is_message_scoped() {
    let mut engine = Engine::new(EngineConfig {
        fragment_bytes: 2,
        max_retransmit_attempts: 1,
        retransmit_timeout_ms: 1,
        ..EngineConfig::default()
    });

    let message_id = engine.send(b"hello").unwrap();
    let first = next_write(&mut engine);
    let second = next_write(&mut engine);
    let third = next_write(&mut engine);

    assert_eq!(first.packet_number.get(), 0);
    assert_eq!(second.packet_number.get(), 1);
    assert_eq!(third.packet_number.get(), 2);

    assert_eq!(
        next_polled_write(&mut engine, 1).packet_number,
        first.packet_number
    );
    assert_eq!(next_write(&mut engine).packet_number, second.packet_number);
    assert_eq!(next_write(&mut engine).packet_number, third.packet_number);

    let failed = next_send_failed(&mut engine, 2);

    assert_eq!(failed.message_id, message_id);
    assert_eq!(failed.channel_id, crate::core::ChannelId::DEFAULT);
    assert_eq!(failed.reason, SendFailureReason::RetryLimitReached);
    assert_eq!(engine.machine.in_flight.packets().count(), 0);
    assert!(Machine::poll_event(&mut engine).is_none());
}

#[test]
fn engine_send_failed_suppresses_same_tick_message_retransmits() {
    let mut engine = Engine::new(EngineConfig {
        fragment_bytes: 2,
        max_retransmit_attempts: 1,
        retransmit_timeout_ms: 1,
        ..EngineConfig::default()
    });

    let message_id = engine.send(b"hello").unwrap();
    let first = next_write(&mut engine);
    let second = next_write(&mut engine);
    let third = next_write(&mut engine);

    assert_eq!(first.packet_number.get(), 0);
    assert_eq!(second.packet_number.get(), 1);
    assert_eq!(third.packet_number.get(), 2);

    assert_eq!(
        next_polled_write(&mut engine, 1).packet_number,
        first.packet_number
    );
    assert_eq!(next_write(&mut engine).packet_number, second.packet_number);
    assert_eq!(next_write(&mut engine).packet_number, third.packet_number);

    let ack = ack_packet_for_ranges(
        &[(
            crate::core::PacketNumber::new(0),
            crate::core::PacketNumber::new(0),
        )],
        crate::core::PacketNumber::new(100),
    );

    assert!(matches!(
        engine.receive(ack.as_bytes()),
        ReceiveReport::Ack { .. }
    ));

    let failed = next_send_failed(&mut engine, 2);

    assert_eq!(failed.message_id, message_id);
    assert_eq!(failed.channel_id, crate::core::ChannelId::DEFAULT);
    assert_eq!(failed.reason, SendFailureReason::RetryLimitReached);
    assert_eq!(engine.machine.in_flight.packets().count(), 0);
    assert!(Machine::poll_event(&mut engine).is_none());
}

fn fragment_len_from_wire(bytes: &[u8]) -> usize {
    let packet_len = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;

    packet_len
        - crate::core::packet::header::PACKET_HEADER_LEN
        - crate::core::frame::message::MESSAGE_FRAME_HEADER_LEN
}

fn next_write(engine: &mut Engine) -> WriteEvent {
    let Some(EngineOutput::Write(write)) = Machine::poll_event(engine) else {
        panic!("engine should produce a write event");
    };

    write
}

fn next_polled_write(engine: &mut Engine, now_ms: u64) -> WriteEvent {
    let mut tx_buf = [0; crate::engine::config::MAX_WIRE_BYTES];

    let EnginePoll::Transmit { bytes, attempts } = engine.poll(now_ms, &mut tx_buf).unwrap() else {
        panic!("engine should produce transmit bytes");
    };

    let mut stored = [0; crate::engine::config::MAX_WIRE_BYTES];
    stored[..bytes.len()].copy_from_slice(bytes);

    WriteEvent {
        packet_number: crate::core::PacketNumber::new(u32::from_le_bytes(
            bytes[10..14].try_into().unwrap(),
        )),
        bytes: stored,
        len: bytes.len(),
        attempts,
    }
}

fn poll_idle(engine: &mut Engine, now_ms: u64) {
    let mut tx_buf = [0; crate::engine::config::MAX_WIRE_BYTES];

    assert_eq!(engine.poll(now_ms, &mut tx_buf).unwrap(), EnginePoll::Idle);
}

fn next_send_failed(engine: &mut Engine, now_ms: u64) -> SendFailedEvent {
    let mut tx_buf = [0; crate::engine::config::MAX_WIRE_BYTES];

    let EnginePoll::SendFailed(failed) = engine.poll(now_ms, &mut tx_buf).unwrap() else {
        panic!("engine should report send failure");
    };

    failed
}

fn ack_packet_for_ranges(
    ranges: &[(crate::core::PacketNumber, crate::core::PacketNumber)],
    packet_number: crate::core::PacketNumber,
) -> WriteEvent {
    let mut bytes = [0; crate::engine::config::MAX_WIRE_BYTES];
    let packet_len = (crate::core::packet::header::PACKET_HEADER_LEN
        + crate::core::frame::ack::ACK_FRAME_LEN) as u16;
    let total_len = crate::wire::WIRE_HEADER_LEN + usize::from(packet_len) + 2;

    bytes[..2].copy_from_slice(&crate::wire::EnvelopeMagic::MSRT.bytes());
    bytes[2] = 1;
    bytes[3] = crate::wire::WIRE_HEADER_LEN as u8;
    bytes[4..6].copy_from_slice(&packet_len.to_le_bytes());
    bytes[6] = crate::wire::WireFlags::CHECKSUM_PRESENT.bits();
    bytes[7] = 0;
    bytes[8] = crate::core::PacketType::Ack.code();
    bytes[9] = 0;
    bytes[10..14].copy_from_slice(&packet_number.get().to_le_bytes());
    bytes[14] = crate::core::FrameKind::Ack.code();
    bytes[15..19].copy_from_slice(&ranges[ranges.len() - 1].1.get().to_le_bytes());
    bytes[19] = ranges.len() as u8;

    let mut offset = 20;
    for (start, end) in ranges {
        bytes[offset..offset + 4].copy_from_slice(&start.get().to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&end.get().to_le_bytes());
        offset += 8;
    }

    let checksum = crate::wire::Checksum::calculate(&crate::wire::Crc16, &bytes[..total_len - 2]);
    bytes[total_len - 2..total_len].copy_from_slice(&checksum.to_le_bytes());

    WriteEvent {
        packet_number,
        bytes,
        len: total_len,
        attempts: 0,
    }
}

fn first_fragments_for_five_messages(engine: &mut Engine) -> [Option<WriteEvent>; 5] {
    let mut fragments = [None; 5];
    let mut write_index = 0;

    for message in [b"aa00", b"bb11", b"cc22", b"dd33", b"ee44"] {
        engine.send(message).unwrap();
    }

    while let Some(event) = Machine::poll_event(engine) {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        if write_index % 2 == 0 {
            fragments[write_index / 2] = Some(write);
        }
        write_index += 1;
    }

    fragments
}

fn assert_message(engine: &mut Engine, expected: &[u8]) {
    let message = next_message(engine);

    assert_eq!(message.as_bytes(), expected);
}

fn next_message(engine: &mut Engine) -> MessageEvent {
    while let Some(event) = Machine::poll_event(engine) {
        if let EngineOutput::Message(message) = event {
            return message;
        }
    }

    panic!("engine should produce a complete message");
}
