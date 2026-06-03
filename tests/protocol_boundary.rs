//! Integration tests for the no_std MSRT protocol facade.

const TX_BUF_BYTES: usize = 128;

use msrt::{
    Engine,
    core::{
        ChannelId, Flags, MessageFlags, MessageFrame, MessageId, Packet, PacketHeader,
        PacketNumber, PacketType,
    },
    engine::EnginePoll,
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

    assert_eq!(EnvelopeMagic::MSRT.bytes(), *b"MS");
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
    let mut engine = Engine::default();
    let mut tx_buf = [0; TX_BUF_BYTES];
    let message_id = engine.send(b"hello").unwrap();

    assert_eq!(message_id, MessageId::ZERO);

    let EnginePoll::Transmit(bytes) = engine.poll(0, &mut tx_buf).unwrap() else {
        panic!("engine should produce transmit bytes");
    };

    assert_eq!(
        PacketNumber::new(u32::from_le_bytes(bytes[10..14].try_into().unwrap())),
        PacketNumber::ZERO
    );
    assert!(!bytes.is_empty());
}

#[test]
fn facade_exposes_reliability_policy_types() {
    let channel_id = ChannelId::new(3);
    let policy = ChannelReliability::new(channel_id, ReliabilityMode::LatestOnly, 1, Some(100));

    assert_eq!(policy.channel_id, channel_id);
    assert_eq!(policy.mode, ReliabilityMode::LatestOnly);
}
