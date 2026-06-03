//! Outgoing message fragmentation and ACK encoding.

use crate::core::{
    AckFrame, ChannelId, Error, ErrorKind, Flags, FrameKind, MAX_ACK_RANGES, MessageId,
    PacketNumber, PacketType, Result,
    frame::{ack::ACK_FRAME_LEN, message::MESSAGE_FRAME_HEADER_LEN},
    packet::header::PACKET_HEADER_LEN,
};
use crate::reliability::ReliabilityMode;
use crate::wire::{
    Checksum, Crc16, EnvelopeHeader, EnvelopeMagic, WIRE_HEADER_CRC_OFFSET, WIRE_HEADER_LEN,
    WIRE_MAGIC_LEN, WIRE_PACKET_LEN_OFFSET, checksum::CHECKSUM_LEN,
};

use crate::engine::{
    EngineConfig,
    config::{MAX_MESSAGE_BYTES, MAX_WIRE_BYTES},
    machine::{
        EngineOutput, Machine, WriteEvent, inflight::InFlightPacket, packet::fragment_flags,
    },
};

const ACK_PACKET_LEN: usize = PACKET_HEADER_LEN + ACK_FRAME_LEN;

impl Machine {
    pub(super) fn send_on_impl(
        &mut self,
        config: &EngineConfig,
        channel_id: ChannelId,
        message: &[u8],
    ) -> Result<MessageId> {
        let fragment_bytes = config.fragment_bytes.clamp(1, max_fragment_bytes());
        let message_id = self.next_message_id;
        let mode = config.reliability_mode(channel_id);
        self.send_message_fragments(channel_id, message_id, message, fragment_bytes, mode)?;
        self.next_message_id = MessageId::new(self.next_message_id.get().wrapping_add(1));

        Ok(message_id)
    }

    pub(super) fn queue_ack(&mut self, acknowledged: PacketNumber) -> Result<()> {
        self.ack_ranges.observe(acknowledged);
        let frame = self.ack_ranges.frame();
        let packet_number = self.next_packet_number;
        let mut wire = [0; MAX_WIRE_BYTES];
        let written = encode_ack_packet(packet_number, frame, &mut wire, &Crc16)?;

        self.events.push(EngineOutput::Write(WriteEvent {
            packet_number,
            bytes: wire,
            len: written,
            attempts: 0,
        }))?;
        self.next_packet_number = self.next_packet_number.next();

        Ok(())
    }

    fn send_message_fragments(
        &mut self,
        channel_id: ChannelId,
        message_id: MessageId,
        message: &[u8],
        fragment_bytes: usize,
        mode: ReliabilityMode,
    ) -> Result<()> {
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        let mut offset = 0;

        while offset < message.len() || (message.is_empty() && offset == 0) {
            let end = core::cmp::min(offset + fragment_bytes, message.len());
            let fragment = &message[offset..end];
            let packet_number = self.next_packet_number;
            let mut wire = [0; MAX_WIRE_BYTES];
            let flags = fragment_flags(offset, end, message.len());
            let encoded = FragmentToEncode {
                packet_number,
                channel_id,
                message_id,
                message_len: message.len(),
                fragment_offset: offset,
                flags,
                fragment,
                ack_eliciting: matches!(mode, ReliabilityMode::Reliable),
            };
            let written = encode_message_fragment(encoded, &mut wire, &Crc16)?;

            self.events.push(EngineOutput::Write(WriteEvent {
                packet_number,
                bytes: wire,
                len: written,
                attempts: 0,
            }))?;
            if matches!(mode, ReliabilityMode::Reliable) {
                self.in_flight.track(InFlightPacket {
                    packet_number,
                    channel_id,
                    message_id,
                    bytes: wire,
                    len: written,
                    attempts: 0,
                    last_sent_ms: self.now_ms,
                })?;
            }
            self.next_packet_number = self.next_packet_number.next();

            if message.is_empty() {
                break;
            }

            offset = end;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FragmentToEncode<'a> {
    packet_number: PacketNumber,
    channel_id: ChannelId,
    message_id: MessageId,
    message_len: usize,
    fragment_offset: usize,
    flags: u8,
    fragment: &'a [u8],
    ack_eliciting: bool,
}

fn encode_message_fragment(
    fragment_to_encode: FragmentToEncode<'_>,
    out: &mut [u8],
    checksum: &impl Checksum,
) -> Result<usize> {
    let packet_len =
        PACKET_HEADER_LEN + MESSAGE_FRAME_HEADER_LEN + fragment_to_encode.fragment.len();
    let packet_len = u8::try_from(packet_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let channel_id = fragment_to_encode.channel_id.get();
    let message_len =
        u16::try_from(fragment_to_encode.message_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let fragment_offset = u16::try_from(fragment_to_encode.fragment_offset)
        .map_err(|_| Error::new(ErrorKind::Engine))?;
    let envelope_header = EnvelopeHeader::new(packet_len);
    let total_len = envelope_header.total_len();

    if out.len() < total_len {
        return Err(Error::buffer_too_small());
    }

    out[..WIRE_MAGIC_LEN].copy_from_slice(&EnvelopeMagic::MSRT.bytes());
    out[WIRE_PACKET_LEN_OFFSET] = envelope_header.packet_len;
    out[WIRE_HEADER_CRC_OFFSET] = envelope_header.header_crc;
    let packet = &mut out[WIRE_HEADER_LEN..];
    packet[0] = PacketType::Data.code();
    packet[1] = if fragment_to_encode.ack_eliciting {
        Flags::ACK_ELICITING.bits()
    } else {
        Flags::EMPTY.bits()
    };
    packet[2..6].copy_from_slice(&fragment_to_encode.packet_number.get().to_le_bytes());
    packet[6] = FrameKind::Message.code();
    packet[7] = channel_id;
    packet[8..12].copy_from_slice(&fragment_to_encode.message_id.get().to_le_bytes());
    packet[12..14].copy_from_slice(&message_len.to_le_bytes());
    packet[14..16].copy_from_slice(&fragment_offset.to_le_bytes());
    packet[16] = fragment_to_encode.flags;
    packet[17..17 + fragment_to_encode.fragment.len()].copy_from_slice(fragment_to_encode.fragment);

    let checksum_value = checksum.calculate(&out[..total_len - CHECKSUM_LEN]);
    out[total_len - CHECKSUM_LEN..total_len].copy_from_slice(&checksum_value.to_le_bytes());

    Ok(total_len)
}

fn encode_ack_packet(
    packet_number: PacketNumber,
    frame: AckFrame,
    out: &mut [u8],
    checksum: &impl Checksum,
) -> Result<usize> {
    let packet_len = u8::try_from(ACK_PACKET_LEN).map_err(|_| Error::new(ErrorKind::Engine))?;
    let envelope_header = EnvelopeHeader::new(packet_len);
    let total_len = envelope_header.total_len();

    if out.len() < total_len {
        return Err(Error::buffer_too_small());
    }

    out[..WIRE_MAGIC_LEN].copy_from_slice(&EnvelopeMagic::MSRT.bytes());
    out[WIRE_PACKET_LEN_OFFSET] = envelope_header.packet_len;
    out[WIRE_HEADER_CRC_OFFSET] = envelope_header.header_crc;
    let packet = &mut out[WIRE_HEADER_LEN..];
    packet[0] = PacketType::Ack.code();
    packet[1] = 0;
    packet[2..6].copy_from_slice(&packet_number.get().to_le_bytes());
    packet[6] = FrameKind::Ack.code();
    packet[7..11].copy_from_slice(&frame.largest_acknowledged.get().to_le_bytes());
    packet[11] = frame.range_count;

    let mut offset = 12;
    let mut index = 0;

    while index < MAX_ACK_RANGES {
        packet[offset..offset + 4].copy_from_slice(&frame.ranges[index].start.get().to_le_bytes());
        packet[offset + 4..offset + 8]
            .copy_from_slice(&frame.ranges[index].end.get().to_le_bytes());
        offset += 8;
        index += 1;
    }

    let checksum_value = checksum.calculate(&out[..total_len - CHECKSUM_LEN]);
    out[total_len - CHECKSUM_LEN..total_len].copy_from_slice(&checksum_value.to_le_bytes());

    Ok(total_len)
}

const fn max_fragment_bytes() -> usize {
    MAX_WIRE_BYTES
        - crate::wire::WIRE_HEADER_LEN
        - PACKET_HEADER_LEN
        - MESSAGE_FRAME_HEADER_LEN
        - CHECKSUM_LEN
}
