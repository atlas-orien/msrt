//! Outgoing message fragmentation and ACK encoding.

use crate::core::{
    AckFrame, ChannelId, Error, ErrorKind, Flags, FrameKind, MAX_ACK_RANGES, MessageId,
    MessageFlags, PacketNumber, Result,
    frame::ack::ACK_FRAME_LEN,
    packet::header::{PACKET_HEADER_LEN, PacketHeader},
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
            let fragment_flags = MessageFlags::from_bits(fragment_flags(offset, end, message.len()));
            let header = PacketHeader::data(
                packet_number,
                if matches!(mode, ReliabilityMode::Reliable) {
                    Flags::ACK_ELICITING
                } else {
                    Flags::EMPTY
                },
                channel_id,
                message_id,
                message.len(),
                offset,
                fragment_flags,
            );
            let written = encode_message_fragment(header, fragment, &mut wire, &Crc16)?;

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

fn encode_message_fragment(
    header: PacketHeader,
    fragment: &[u8],
    out: &mut [u8],
    checksum: &impl Checksum,
) -> Result<usize> {
    let packet_len = PACKET_HEADER_LEN + fragment.len();
    let packet_len = u8::try_from(packet_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let message_len =
        u16::try_from(header.message_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let fragment_offset = u16::try_from(header.fragment_offset)
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
    packet[0] = header.packet_type.code();
    packet[1] = header.flags.bits();
    packet[2..6].copy_from_slice(&header.packet_number.get().to_le_bytes());
    packet[6] = header.channel_id.get();
    packet[7..11].copy_from_slice(&header.message_id.get().to_le_bytes());
    packet[11..13].copy_from_slice(&message_len.to_le_bytes());
    packet[13..15].copy_from_slice(&fragment_offset.to_le_bytes());
    packet[15] = header.fragment_flags.bits();
    packet[16..16 + fragment.len()].copy_from_slice(fragment);

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
    let header = PacketHeader::ack(packet_number);
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
    packet[0] = header.packet_type.code();
    packet[1] = header.flags.bits();
    packet[2..6].copy_from_slice(&header.packet_number.get().to_le_bytes());
    packet[6] = header.channel_id.get();
    packet[7..11].copy_from_slice(&header.message_id.get().to_le_bytes());
    packet[11..13].copy_from_slice(&(header.message_len as u16).to_le_bytes());
    packet[13..15].copy_from_slice(&(header.fragment_offset as u16).to_le_bytes());
    packet[15] = header.fragment_flags.bits();
    packet[16] = FrameKind::Ack.code();
    packet[17..21].copy_from_slice(&frame.largest_acknowledged.get().to_le_bytes());
    packet[21] = frame.range_count;

    let mut offset = 22;
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
        - CHECKSUM_LEN
}
