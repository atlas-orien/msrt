//! Integration tests for the no_std SRT protocol facade.

use srt::{
    Config, Engine, Event,
    core::{
        ChannelId, Flags, MessageFlags, MessageFrame, MessageId, Packet, PacketHeader,
        PacketNumber, PacketType,
    },
    reliability::{
        ChannelReliability, FragmentRange, MessageFragment, MessageKey, ReliabilityMode,
    },
    wire::{EnvelopeHeader, EnvelopeMagic, WireEnvelope, WireFlags},
};

#[test]
fn facade_exposes_core_packet_and_wire_envelope() {
    let payload = [0x01, 0x02, 0x03];
    let header = PacketHeader::new(
        PacketType::Data,
        PacketNumber::new(42),
        Flags::ACK_ELICITING,
    );
    let packet = Packet::new(header, &payload);

    let envelope_header =
        EnvelopeHeader::new(packet.payload_len() as u16, WireFlags::CHECKSUM_PRESENT);
    let envelope = WireEnvelope::new(envelope_header, packet.payload.as_bytes(), 0x1234);

    assert_eq!(EnvelopeMagic::SRT.bytes(), *b"SR");
    assert_eq!(envelope.packet_bytes, &payload);
    assert!(envelope.has_valid_len());
    assert_eq!(envelope.header.packet_len, 3);
}

#[test]
fn facade_exposes_reliability_fragment_view() {
    let bytes = [1, 2, 3, 4];
    let frame = MessageFrame::new(
        ChannelId::new(7),
        MessageId::new(9),
        8,
        2,
        MessageFlags::FIRST,
        &bytes,
    );

    let fragment = MessageFragment::try_from_message_frame(frame).unwrap();

    assert_eq!(
        fragment.key,
        MessageKey::new(ChannelId::new(7), MessageId::new(9))
    );
    assert_eq!(fragment.range, FragmentRange::new(2, 4));
}

#[test]
fn facade_exposes_concrete_engine_api() {
    let mut engine = Engine::new(Config::default());
    let message_id = engine.send(b"hello").unwrap();

    assert_eq!(message_id, MessageId::ZERO);

    let Some(Event::Write(write)) = engine.poll_event() else {
        panic!("engine should produce a write event");
    };

    assert_eq!(write.packet_number, PacketNumber::ZERO);
    assert!(!write.as_bytes().is_empty());
}

#[test]
fn facade_exposes_reliability_policy_types() {
    let channel_id = ChannelId::new(3);
    let policy = ChannelReliability::new(channel_id, ReliabilityMode::LatestOnly, 1, Some(100));

    assert_eq!(policy.channel_id, channel_id);
    assert_eq!(policy.mode, ReliabilityMode::LatestOnly);
}
