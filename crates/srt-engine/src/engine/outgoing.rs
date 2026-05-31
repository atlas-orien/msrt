//! Outgoing message fragmentation and ACK encoding.

use srt_core::{
    ChannelId, Error, ErrorKind, Flags, FrameKind, MessageId, PacketNumber, PacketType, Result,
};
use srt_wire::{Checksum, Crc16, EnvelopeHeader, EnvelopeMagic, WireFlags};

use crate::{
    Engine, EngineOutput, MAX_MESSAGE_BYTES, MAX_WIRE_BYTES, WriteEvent,
    engine::{inflight::InFlightPacket, packet::fragment_flags},
    layout::{ACK_PACKET_LEN, CHECKSUM_LEN, MESSAGE_FRAME_HEADER_LEN, PACKET_HEADER_LEN},
};

impl Engine {
    /// Queues a complete message for non-blocking protocol transmission.
    ///
    /// The caller submits the complete message once. The engine splits it into
    /// packet-sized write events internally.
    pub fn send(&mut self, message: &[u8]) -> Result<MessageId> {
        let fragment_bytes = self.fragment_bytes.clamp(1, max_fragment_bytes());
        let message_id = self.next_message_id;
        self.send_message_fragments(message_id, message, fragment_bytes)?;
        self.next_message_id = MessageId::new(self.next_message_id.get().wrapping_add(1));

        Ok(message_id)
    }

    pub(crate) fn queue_ack(&mut self, acknowledged: PacketNumber) -> Result<()> {
        let packet_number = self.next_packet_number;
        let mut wire = [0; MAX_WIRE_BYTES];
        let written = encode_ack_packet(packet_number, acknowledged, &mut wire, &Crc16)?;

        self.events.push(EngineOutput::Write(WriteEvent {
            packet_number,
            bytes: wire,
            len: written,
        }))?;
        self.next_packet_number = self.next_packet_number.next();

        Ok(())
    }

    fn send_message_fragments(
        &mut self,
        message_id: MessageId,
        message: &[u8],
        fragment_bytes: usize,
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
                channel_id: ChannelId::CONTROL,
                message_id,
                message_len: message.len(),
                fragment_offset: offset,
                flags,
                fragment,
            };
            let written = encode_message_fragment(encoded, &mut wire, &Crc16)?;

            self.events.push(EngineOutput::Write(WriteEvent {
                packet_number,
                bytes: wire,
                len: written,
            }))?;
            self.in_flight.track(InFlightPacket {
                packet_number,
                bytes: wire,
                len: written,
            })?;
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
}

fn encode_message_fragment(
    fragment_to_encode: FragmentToEncode<'_>,
    out: &mut [u8],
    checksum: &impl Checksum,
) -> Result<usize> {
    let packet_len =
        PACKET_HEADER_LEN + MESSAGE_FRAME_HEADER_LEN + fragment_to_encode.fragment.len();
    let packet_len = u16::try_from(packet_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let channel_id = fragment_to_encode.channel_id.get();
    let message_len =
        u16::try_from(fragment_to_encode.message_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let fragment_offset = u16::try_from(fragment_to_encode.fragment_offset)
        .map_err(|_| Error::new(ErrorKind::Engine))?;
    let envelope_header = EnvelopeHeader::new(packet_len, WireFlags::CHECKSUM_PRESENT);
    let total_len = envelope_header.total_len();

    if out.len() < total_len {
        return Err(Error::buffer_too_small());
    }

    out[..2].copy_from_slice(&EnvelopeMagic::SRT.bytes());
    out[2] = envelope_header.version;
    out[3] = envelope_header.header_len;
    out[4..6].copy_from_slice(&envelope_header.packet_len.to_le_bytes());
    out[6] = envelope_header.flags.bits();
    out[7] = envelope_header.reserved;
    out[8] = PacketType::Data.code();
    out[9] = Flags::ACK_ELICITING.bits();
    out[10..14].copy_from_slice(&fragment_to_encode.packet_number.get().to_le_bytes());
    out[14] = FrameKind::Message.code();
    out[15..17].copy_from_slice(&channel_id.to_le_bytes());
    out[17..21].copy_from_slice(&fragment_to_encode.message_id.get().to_le_bytes());
    out[21..23].copy_from_slice(&message_len.to_le_bytes());
    out[23..25].copy_from_slice(&fragment_offset.to_le_bytes());
    out[25] = fragment_to_encode.flags;
    out[26..26 + fragment_to_encode.fragment.len()].copy_from_slice(fragment_to_encode.fragment);

    let checksum_value = checksum.calculate(&out[..total_len - CHECKSUM_LEN]);
    out[total_len - CHECKSUM_LEN..total_len].copy_from_slice(&checksum_value.to_le_bytes());

    Ok(total_len)
}

fn encode_ack_packet(
    packet_number: PacketNumber,
    acknowledged: PacketNumber,
    out: &mut [u8],
    checksum: &impl Checksum,
) -> Result<usize> {
    let packet_len = u16::try_from(ACK_PACKET_LEN).map_err(|_| Error::new(ErrorKind::Engine))?;
    let envelope_header = EnvelopeHeader::new(packet_len, WireFlags::CHECKSUM_PRESENT);
    let total_len = envelope_header.total_len();

    if out.len() < total_len {
        return Err(Error::buffer_too_small());
    }

    out[..2].copy_from_slice(&EnvelopeMagic::SRT.bytes());
    out[2] = envelope_header.version;
    out[3] = envelope_header.header_len;
    out[4..6].copy_from_slice(&envelope_header.packet_len.to_le_bytes());
    out[6] = envelope_header.flags.bits();
    out[7] = envelope_header.reserved;
    out[8] = PacketType::Ack.code();
    out[9] = 0;
    out[10..14].copy_from_slice(&packet_number.get().to_le_bytes());
    out[14] = FrameKind::Ack.code();
    out[15..19].copy_from_slice(&acknowledged.get().to_le_bytes());

    let checksum_value = checksum.calculate(&out[..total_len - CHECKSUM_LEN]);
    out[total_len - CHECKSUM_LEN..total_len].copy_from_slice(&checksum_value.to_le_bytes());

    Ok(total_len)
}

const fn max_fragment_bytes() -> usize {
    MAX_WIRE_BYTES
        - srt_wire::WIRE_HEADER_LEN
        - PACKET_HEADER_LEN
        - MESSAGE_FRAME_HEADER_LEN
        - CHECKSUM_LEN
}
