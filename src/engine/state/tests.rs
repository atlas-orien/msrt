use crate::engine::state::{EngineOutput, EngineState, WriteEvent};
use crate::engine::{
    Engine, EngineConfig, EnginePoll, MessageEvent, ReceiveReport, SendFailedEvent,
    SendFailureReason,
};
use crate::integrity::IntegrityConfig;
#[cfg(feature = "dynamic-recovery")]
use crate::reliability::DynamicRecoveryConfig;

#[test]
fn engine_sends_one_message_as_multiple_write_events() {
    let mut engine = Engine::new(EngineConfig {
        fragment_bytes: 5,
        ..EngineConfig::default()
    });

    let message_id = engine.send(b"hello msrt testing").unwrap();
    let mut writes = 0;

    while let Some(event) = EngineState::poll_event(&mut engine.state) {
        match event {
            EngineOutput::Write(_) => writes += 1,
            EngineOutput::SendFailed(failed) => {
                panic!("sender should not fail in this test: {failed:?}");
            }
        }
    }

    assert_ne!(message_id.get(), 0);
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

    while let Some(event) = EngineState::poll_event(&mut a.state) {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        assert!(matches!(
            b.receive(write.as_bytes()),
            ReceiveReport::Packet { .. } | ReceiveReport::Ack { .. }
        ));
    }

    assert_message(&mut b, b"hello msrt testing");
}

#[test]
fn engine_integrity_config_accepts_sip_tag_packets() {
    let config = EngineConfig {
        integrity: IntegrityConfig::sip_tag(),
        ..EngineConfig::default()
    };
    let mut a = Engine::new(config);
    let mut b = Engine::new(config);

    a.send(b"hello").unwrap();
    let write = next_write(&mut a);

    assert!(matches!(
        b.receive(write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));
    assert_message(&mut b, b"hello");
}

#[test]
fn engine_integrity_config_rejects_different_sip_tag_keys() {
    let mut a = Engine::new(EngineConfig {
        integrity: IntegrityConfig::sip_tag_with_key([1; crate::integrity::SipTag::KEY_LEN]),
        ..EngineConfig::default()
    });
    let mut b = Engine::new(EngineConfig {
        integrity: IntegrityConfig::sip_tag_with_key([2; crate::integrity::SipTag::KEY_LEN]),
        ..EngineConfig::default()
    });

    a.send(b"hello").unwrap();
    let write = next_write(&mut a);

    assert_eq!(b.receive(write.as_bytes()), ReceiveReport::Corrupted);
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

    while let Some(event) = EngineState::poll_event(&mut a.state) {
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
fn engine_evicts_oldest_reassembly_slot_when_budget_is_full() {
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
        ReceiveReport::Packet { .. }
    ));
}

#[test]
fn engine_receive_keeps_protocol_alive_when_message_queue_is_full() {
    let mut sender = Engine::new(EngineConfig::default());
    let mut receiver = Engine::new(EngineConfig::default());

    for index in 0..crate::engine::config::MAX_MESSAGE_EVENTS + 1 {
        sender.send(&[index as u8]).unwrap();
        let write = next_write(&mut sender);

        assert!(matches!(
            receiver.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));

        let ack = next_write(&mut receiver);
        assert!(matches!(
            sender.receive(ack.as_bytes()),
            ReceiveReport::Ack { .. }
        ));
    }

    assert_eq!(sender.state.recovery.in_flight_len(), 0);
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
fn engine_ack_clears_one_in_flight_packet() {
    let mut a = Engine::new(EngineConfig {
        fragment_bytes: 2,
        ..EngineConfig::default()
    });
    let mut b = Engine::new(EngineConfig::default());

    a.send(b"abcdef").unwrap();

    while let Some(event) = EngineState::poll_event(&mut a.state) {
        let EngineOutput::Write(write) = event else {
            continue;
        };

        assert!(matches!(
            b.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));
    }

    let last_ack = next_polled_write(&mut b, 1);

    assert!(matches!(
        a.receive(last_ack.as_bytes()),
        ReceiveReport::Ack { .. }
    ));

    poll_idle(&mut a, 1);
}

#[test]
fn engine_polls_pending_ack_before_queued_data() {
    let mut sender = Engine::new(EngineConfig::default());
    let mut receiver = Engine::new(EngineConfig::default());
    let mut peer = Engine::new(EngineConfig::default());

    receiver.send(b"queued local data").unwrap();
    sender.send(b"ack this first").unwrap();
    let incoming = next_write(&mut sender);

    assert!(matches!(
        receiver.receive(incoming.as_bytes()),
        ReceiveReport::Packet { .. }
    ));

    let write = next_write(&mut receiver);

    assert_eq!(
        packet_type_from_wire(write.as_bytes()),
        crate::core::PacketType::Ack
    );
    assert!(matches!(
        peer.receive(write.as_bytes()),
        ReceiveReport::Ack { .. }
    ));
}

#[test]
fn engine_polls_message_before_queued_new_data_when_no_ack_is_pending() {
    let mut sender = Engine::new(EngineConfig::default());
    let mut receiver = Engine::new(EngineConfig::default());

    sender.send(b"incoming").unwrap();
    receiver.send(b"queued local data").unwrap();
    let incoming = next_write(&mut sender);

    assert!(matches!(
        receiver.receive(incoming.as_bytes()),
        ReceiveReport::Packet { .. }
    ));
    let _ack = next_write(&mut receiver);

    let mut tx_buf = [0; crate::engine::config::MAX_WIRE_BYTES];
    let EnginePoll::Message(message) = receiver.poll(1, &mut tx_buf).unwrap() else {
        panic!("receiver should deliver message before queued new data");
    };

    assert_eq!(message.as_bytes(), b"incoming");
}

#[test]
fn engine_single_ack_retransmits_unacked_packets() {
    let mut engine = Engine::new(test_retransmit_config(2, 5, 1));

    engine.send(b"abcdefgh").unwrap();

    let first = next_write(&mut engine);
    let second = next_write(&mut engine);
    let third = next_write(&mut engine);
    let fourth = next_write(&mut engine);

    assert_eq!(first.key.packet_index.get(), 0);
    assert_eq!(second.key.packet_index.get(), 1);
    assert_eq!(third.key.packet_index.get(), 2);
    assert_eq!(fourth.key.packet_index.get(), 3);

    let ack = ack_packet_for_key(first.key);

    assert!(matches!(
        engine.receive(ack.as_bytes()),
        ReceiveReport::Ack { .. }
    ));

    assert_eq!(next_polled_write(&mut engine, 1).key, second.key);
}

#[test]
fn engine_tick_waits_for_retransmit_timeout() {
    let mut engine = Engine::new(test_retransmit_config(
        EngineConfig::default().fragment_bytes,
        5,
        10,
    ));

    engine.send(b"hello").unwrap();
    let first = next_write(&mut engine);

    poll_idle(&mut engine, 9);

    assert_eq!(next_polled_write(&mut engine, 10).key, first.key);

    poll_idle(&mut engine, 19);

    assert_eq!(next_polled_write(&mut engine, 20).key, first.key);
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
    let split = crate::wire::WIRE_MAGIC_LEN + 1;

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
fn engine_treats_semantically_malformed_packet_as_corrupted() {
    let mut engine = Engine::new(EngineConfig::default());
    let mut malformed = ack_packet_for_key(crate::core::PacketKey::new(
        crate::core::MessageId::new(7),
        crate::core::PacketIndex::ZERO,
    ));
    let integrity = EngineConfig::default().integrity;
    let tag_len = crate::integrity::Integrity::tag_len(&integrity);
    let packet_len = (crate::core::ACK_PACKET_HEADER_LEN + 1) as u8;
    let total_len = crate::wire::WIRE_HEADER_LEN + usize::from(packet_len) + tag_len;
    malformed.len = total_len;
    malformed.bytes[crate::wire::WIRE_PACKET_LEN_OFFSET] = packet_len;
    malformed.bytes[crate::wire::WIRE_HEADER_CRC_OFFSET] = crate::wire::header_crc(packet_len);
    malformed.bytes[crate::wire::WIRE_HEADER_LEN + crate::core::ACK_PACKET_HEADER_LEN] = 1;
    let (authenticated, tag) = malformed.bytes[..total_len].split_at_mut(total_len - tag_len);
    crate::integrity::Integrity::seal(&integrity, authenticated, tag);

    assert_eq!(
        engine.receive(malformed.as_bytes()),
        ReceiveReport::Corrupted
    );
}

#[test]
fn engine_treats_reassembly_conflict_as_corrupted() {
    let mut sender = Engine::new(EngineConfig {
        fragment_bytes: 2,
        ..EngineConfig::default()
    });
    let mut receiver = Engine::new(EngineConfig::default());

    sender.send(b"abcd").unwrap();
    let first = next_write(&mut sender);
    let mut second = next_write(&mut sender);

    assert!(matches!(
        receiver.receive(first.as_bytes()),
        ReceiveReport::Packet { .. }
    ));

    rewrite_data_message_len(&mut second, 3);

    assert_eq!(
        receiver.receive(second.as_bytes()),
        ReceiveReport::Corrupted
    );
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

    while let Some(event) = EngineState::poll_event(&mut a.state) {
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

    let _duplicate_ack = next_write(&mut b);
    poll_idle(&mut b, 1);
}

#[test]
fn engine_reacknowledges_each_duplicate_data_packet() {
    let mut sender = Engine::new(EngineConfig::default());
    let mut receiver = Engine::new(EngineConfig::default());

    sender.send(b"hello").unwrap();
    let write = next_write(&mut sender);

    assert!(matches!(
        receiver.receive(write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));
    let first_ack = next_write(&mut receiver);

    assert!(matches!(
        receiver.receive(write.as_bytes()),
        ReceiveReport::Duplicate { .. }
    ));
    let duplicate_ack = next_write(&mut receiver);

    assert_eq!(
        packet_type_from_wire(first_ack.as_bytes()),
        crate::core::PacketType::Ack
    );
    assert_eq!(
        packet_type_from_wire(duplicate_ack.as_bytes()),
        crate::core::PacketType::Ack
    );
    assert_eq!(first_ack.key, duplicate_ack.key);
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

    while let Some(event) = EngineState::poll_event(&mut engine.state) {
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

    let packet_len = usize::from(bytes[crate::wire::WIRE_PACKET_LEN_OFFSET]);
    let packet = &bytes[crate::wire::WIRE_HEADER_LEN..crate::wire::WIRE_HEADER_LEN + packet_len];

    assert_eq!(
        &bytes[..crate::wire::WIRE_MAGIC_LEN],
        &crate::wire::EnvelopeMagic::MSRT.bytes()
    );
    assert_eq!(packet[0], crate::core::PacketType::Data.code());
    assert_eq!(packet[1], crate::core::Flags::ACK_ELICITING.bits());
    assert_eq!(
        u32::from_le_bytes(packet[2..6].try_into().unwrap()),
        write.key.message_id.get()
    );
    assert_eq!(
        u16::from_le_bytes(packet[6..8].try_into().unwrap()),
        write.key.packet_index.get()
    );
    assert_eq!(u16::from_le_bytes(packet[8..10].try_into().unwrap()), 5);
    assert_eq!(u16::from_le_bytes(packet[10..12].try_into().unwrap()), 0);
    assert_eq!(&packet[crate::core::DATA_PACKET_HEADER_LEN..], b"hello");
}

#[test]
fn engine_send_uses_data_packet_kind() {
    let mut engine = Engine::new(EngineConfig::default());

    engine.send(b"hello default").unwrap();

    let write = next_write(&mut engine);

    assert_eq!(
        packet_type_from_wire(write.as_bytes()),
        crate::core::PacketType::Data
    );
}

#[test]
fn engine_send_log_uses_log_packet_kind() {
    let mut sender = Engine::new(EngineConfig::default());
    let mut receiver = Engine::new(EngineConfig::default());

    sender.send_log(b"log via api").unwrap();

    let write = next_write(&mut sender);

    assert_eq!(
        packet_type_from_wire(write.as_bytes()),
        crate::core::PacketType::Log
    );
    assert_eq!(sender.state.recovery.in_flight_len(), 0);

    assert!(matches!(
        receiver.receive(write.as_bytes()),
        ReceiveReport::Packet { .. }
    ));

    let message = next_message(&mut receiver);

    assert_eq!(message.packet_type, crate::core::PacketType::Log);
    assert_eq!(message.as_bytes(), b"log via api");
    assert!(EngineState::poll_event(&mut receiver.state).is_none());
}

#[test]
fn engine_reports_send_failed_after_retry_limit() {
    let mut engine = Engine::new(test_retransmit_config(
        EngineConfig::default().fragment_bytes,
        1,
        1,
    ));

    let message_id = engine.send(b"hello").unwrap();
    let first = next_write(&mut engine);

    assert_eq!(first.key.packet_index.get(), 0);

    let retry = next_polled_write(&mut engine, 1);

    assert_eq!(retry.key, first.key);

    let failed = next_send_failed(&mut engine, 2);

    assert_eq!(failed.message_id, message_id);
    assert_eq!(failed.packet_type, crate::core::PacketType::Data);
    assert_eq!(failed.reason, SendFailureReason::RetryLimitReached);
}

#[test]
fn engine_send_failed_is_message_scoped() {
    let mut engine = Engine::new(test_retransmit_config(2, 1, 1));

    let message_id = engine.send(b"hello").unwrap();
    let first = next_write(&mut engine);
    let second = next_write(&mut engine);
    let third = next_write(&mut engine);

    assert_eq!(first.key.packet_index.get(), 0);
    assert_eq!(second.key.packet_index.get(), 1);
    assert_eq!(third.key.packet_index.get(), 2);

    assert_eq!(next_polled_write(&mut engine, 1).key, first.key);
    assert_eq!(next_write(&mut engine).key, second.key);
    assert_eq!(next_write(&mut engine).key, third.key);

    let failed = next_send_failed(&mut engine, 2);

    assert_eq!(failed.message_id, message_id);
    assert_eq!(failed.packet_type, crate::core::PacketType::Data);
    assert_eq!(failed.reason, SendFailureReason::RetryLimitReached);
    assert_eq!(engine.state.recovery.in_flight_len(), 0);
    assert!(EngineState::poll_event(&mut engine.state).is_none());
}

#[test]
fn engine_send_failed_suppresses_same_tick_message_retransmits() {
    let mut engine = Engine::new(test_retransmit_config(2, 1, 1));

    let message_id = engine.send(b"hello").unwrap();
    let first = next_write(&mut engine);
    let second = next_write(&mut engine);
    let third = next_write(&mut engine);

    assert_eq!(first.key.packet_index.get(), 0);
    assert_eq!(second.key.packet_index.get(), 1);
    assert_eq!(third.key.packet_index.get(), 2);

    assert_eq!(next_polled_write(&mut engine, 1).key, first.key);
    assert_eq!(next_write(&mut engine).key, second.key);
    assert_eq!(next_write(&mut engine).key, third.key);

    let ack = ack_packet_for_key(first.key);

    assert!(matches!(
        engine.receive(ack.as_bytes()),
        ReceiveReport::Ack { .. }
    ));

    let failed = next_send_failed(&mut engine, 2);

    assert_eq!(failed.message_id, message_id);
    assert_eq!(failed.packet_type, crate::core::PacketType::Data);
    assert_eq!(failed.reason, SendFailureReason::RetryLimitReached);
    assert_eq!(engine.state.recovery.in_flight_len(), 0);
    assert!(EngineState::poll_event(&mut engine.state).is_none());
}

fn fragment_len_from_wire(bytes: &[u8]) -> usize {
    let packet_len = bytes[crate::wire::WIRE_PACKET_LEN_OFFSET] as usize;
    let header_len = match packet_type_from_wire(bytes) {
        crate::core::PacketType::Data => crate::core::DATA_PACKET_HEADER_LEN,
        crate::core::PacketType::Log => crate::core::LOG_PACKET_HEADER_LEN,
        crate::core::PacketType::Ack => crate::core::ACK_PACKET_HEADER_LEN,
        crate::core::PacketType::Ping | crate::core::PacketType::Pong => {
            crate::core::LIVENESS_PACKET_HEADER_LEN
        }
    };

    packet_len - header_len
}

fn packet_type_from_wire(bytes: &[u8]) -> crate::core::PacketType {
    crate::core::PacketType::from_code(bytes[crate::wire::WIRE_HEADER_LEN])
        .expect("packet type should decode")
}

fn packet_key_from_wire(bytes: &[u8]) -> crate::core::PacketKey {
    let packet = &bytes[crate::wire::WIRE_HEADER_LEN..];
    match packet_type_from_wire(bytes) {
        crate::core::PacketType::Data => crate::core::PacketKey::new(
            crate::core::MessageId::new(u32::from_le_bytes(packet[2..6].try_into().unwrap())),
            crate::core::PacketIndex::new(u16::from_le_bytes(packet[6..8].try_into().unwrap())),
        ),
        crate::core::PacketType::Log | crate::core::PacketType::Ack => crate::core::PacketKey::new(
            crate::core::MessageId::new(u32::from_le_bytes(packet[1..5].try_into().unwrap())),
            crate::core::PacketIndex::new(u16::from_le_bytes(packet[5..7].try_into().unwrap())),
        ),
        crate::core::PacketType::Ping | crate::core::PacketType::Pong => {
            crate::core::PacketKey::new(
                crate::core::MessageId::ZERO,
                crate::core::PacketIndex::ZERO,
            )
        }
    }
}

fn next_write(engine: &mut Engine) -> WriteEvent {
    next_polled_write(engine, 0)
}

fn test_retransmit_config(
    fragment_bytes: usize,
    max_retransmit_attempts: u8,
    retransmit_timeout_ms: u64,
) -> EngineConfig {
    EngineConfig {
        fragment_bytes,
        max_retransmit_attempts,
        retransmit_timeout_ms,
        #[cfg(feature = "dynamic-recovery")]
        dynamic_recovery: DynamicRecoveryConfig {
            initial_rtt_ms: 0,
            max_ack_delay_ms: 0,
            timer_granularity_ms: retransmit_timeout_ms,
            max_backoff_exponent: 0,
        },
        ..EngineConfig::default()
    }
}

fn next_polled_write(engine: &mut Engine, now_ms: u64) -> WriteEvent {
    let mut tx_buf = [0; crate::engine::config::MAX_WIRE_BYTES];

    let EnginePoll::Transmit { bytes, attempts } = engine.poll(now_ms, &mut tx_buf).unwrap() else {
        panic!("engine should produce transmit bytes");
    };

    let mut stored = [0; crate::engine::config::MAX_WIRE_BYTES];
    stored[..bytes.len()].copy_from_slice(bytes);

    WriteEvent {
        key: packet_key_from_wire(bytes),
        bytes: stored,
        len: bytes.len(),
        attempts,
        priority: crate::engine::state::scheduler::WritePriority::NewData,
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

fn ack_packet_for_key(key: crate::core::PacketKey) -> WriteEvent {
    let mut bytes = [0; crate::engine::config::MAX_WIRE_BYTES];
    let packet_len = crate::core::ACK_PACKET_HEADER_LEN as u8;
    let integrity = EngineConfig::default().integrity;
    let tag_len = crate::integrity::Integrity::tag_len(&integrity);
    let total_len = crate::wire::WIRE_HEADER_LEN + usize::from(packet_len) + tag_len;

    bytes[..crate::wire::WIRE_MAGIC_LEN].copy_from_slice(&crate::wire::EnvelopeMagic::MSRT.bytes());
    bytes[crate::wire::WIRE_PACKET_LEN_OFFSET] = packet_len;
    bytes[crate::wire::WIRE_HEADER_CRC_OFFSET] = crate::wire::header_crc(packet_len);
    let packet = &mut bytes[crate::wire::WIRE_HEADER_LEN..];
    packet[0] = crate::core::PacketType::Ack.code();
    packet[1..5].copy_from_slice(&key.message_id.get().to_le_bytes());
    packet[5..7].copy_from_slice(&key.packet_index.get().to_le_bytes());

    let (authenticated, tag) = bytes[..total_len].split_at_mut(total_len - tag_len);
    crate::integrity::Integrity::seal(&integrity, authenticated, tag);

    WriteEvent {
        key,
        bytes,
        len: total_len,
        attempts: 0,
        priority: crate::engine::state::scheduler::WritePriority::Control,
    }
}

fn rewrite_data_message_len(write: &mut WriteEvent, message_len: u16) {
    let integrity = EngineConfig::default().integrity;
    let tag_len = crate::integrity::Integrity::tag_len(&integrity);
    let total_len = write.len;
    let packet = &mut write.bytes[crate::wire::WIRE_HEADER_LEN..];

    assert_eq!(packet[0], crate::core::PacketType::Data.code());
    packet[8..10].copy_from_slice(&message_len.to_le_bytes());

    let (authenticated, tag) = write.bytes[..total_len].split_at_mut(total_len - tag_len);
    crate::integrity::Integrity::seal(&integrity, authenticated, tag);
}

fn first_fragments_for_five_messages(engine: &mut Engine) -> [Option<WriteEvent>; 5] {
    let mut fragments = [None; 5];
    let mut write_index = 0;

    for message in [b"aa00", b"bb11", b"cc22", b"dd33", b"ee44"] {
        engine.send(message).unwrap();
    }

    while let Some(event) = EngineState::poll_event(&mut engine.state) {
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
    let mut tx_buf = [0; crate::engine::config::MAX_WIRE_BYTES];

    loop {
        match engine.poll(0, &mut tx_buf).unwrap() {
            EnginePoll::Transmit { .. } | EnginePoll::SendFailed(_) => continue,
            EnginePoll::Message(message) => return message,
            EnginePoll::Idle => panic!("engine should produce a complete message"),
        }
    }
}
