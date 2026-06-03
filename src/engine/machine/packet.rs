//! v1 draft packet byte layout glue.

use crate::core::{
    AckFrame, AckRange, ChannelId, Error, Flags, FrameKind, MAX_ACK_RANGES, MessageFlags,
    MessageId, PacketNumber, PacketType, Result,
    frame::{ack::ACK_FRAME_LEN, message::MESSAGE_FRAME_HEADER_LEN},
    packet::header::PACKET_HEADER_LEN,
};

use crate::engine::config::MAX_WIRE_BYTES;

/// Decoded MVP packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketDecode<'a> {
    /// Data packet carrying one message fragment.
    Data(DecodedFragment<'a>),
    /// ACK packet.
    Ack(DecodedAck),
    /// Malformed packet bytes.
    Malformed,
}

/// Decoded ACK packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedAck {
    pub(crate) frame: AckFrame,
}

/// Decoded message fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedFragment<'a> {
    pub(crate) packet_number: PacketNumber,
    pub(crate) ack_eliciting: bool,
    pub(crate) channel_id: ChannelId,
    pub(crate) message_id: MessageId,
    pub(crate) message_len: usize,
    pub(crate) fragment_offset: usize,
    pub(crate) flags: u8,
    pub(crate) bytes: &'a [u8],
}

/// Owned packet bytes copied out of the streaming decoder buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PacketBytes {
    bytes: [u8; MAX_WIRE_BYTES],
    len: usize,
}

impl PacketBytes {
    pub(crate) fn try_from_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_WIRE_BYTES {
            return Err(Error::buffer_too_small());
        }

        let mut packet = Self {
            bytes: [0; MAX_WIRE_BYTES],
            len: bytes.len(),
        };
        packet.bytes[..bytes.len()].copy_from_slice(bytes);

        Ok(packet)
    }

    pub(crate) const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}

pub(crate) fn decode_packet_bytes(bytes: &[u8]) -> PacketDecode<'_> {
    let Some(header) = packet_header_from_bytes(bytes) else {
        return PacketDecode::Malformed;
    };

    let frame_bytes = &bytes[PACKET_HEADER_LEN..];

    match PacketType::from_code(header.packet_type) {
        Some(PacketType::Data) => fragment_from_packet_bytes(header, frame_bytes)
            .map(PacketDecode::Data)
            .unwrap_or(PacketDecode::Malformed),
        Some(PacketType::Ack) => ack_from_packet_bytes(frame_bytes)
            .map(PacketDecode::Ack)
            .unwrap_or(PacketDecode::Malformed),
        None => PacketDecode::Malformed,
    }
}

pub(crate) const fn fragment_flags(offset: usize, end: usize, message_len: usize) -> u8 {
    let mut flags = 0;

    if offset == 0 {
        flags |= MessageFlags::FIRST.bits();
    }

    if end == message_len {
        flags |= MessageFlags::LAST.bits();
    }

    flags
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedPacketHeader {
    packet_type: u8,
    packet_flags: u8,
    packet_number: PacketNumber,
}

fn packet_header_from_bytes(bytes: &[u8]) -> Option<DecodedPacketHeader> {
    if bytes.len() < PACKET_HEADER_LEN {
        return None;
    }

    Some(DecodedPacketHeader {
        packet_type: *bytes.first()?,
        packet_flags: *bytes.get(1)?,
        packet_number: PacketNumber::new(u32::from_le_bytes(bytes.get(2..6)?.try_into().ok()?)),
    })
}

fn ack_from_packet_bytes(bytes: &[u8]) -> Option<DecodedAck> {
    if bytes.len() != ACK_FRAME_LEN || *bytes.first()? != FrameKind::Ack.code() {
        return None;
    }

    let largest = PacketNumber::new(u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?));
    let range_count = *bytes.get(5)?;

    if range_count as usize > MAX_ACK_RANGES {
        return None;
    }

    let empty = AckRange::new(PacketNumber::ZERO, PacketNumber::ZERO);
    let mut ranges = [empty; MAX_ACK_RANGES];
    let mut offset = 6;
    let mut index = 0;

    while index < MAX_ACK_RANGES {
        let start = PacketNumber::new(u32::from_le_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ));
        let end = PacketNumber::new(u32::from_le_bytes(
            bytes.get(offset + 4..offset + 8)?.try_into().ok()?,
        ));

        if index < range_count as usize {
            ranges[index] = AckRange::new(start, end);
        }

        offset += 8;
        index += 1;
    }

    Some(DecodedAck {
        frame: AckFrame {
            largest_acknowledged: largest,
            range_count,
            ranges,
        },
    })
}

fn fragment_from_packet_bytes(
    header: DecodedPacketHeader,
    bytes: &[u8],
) -> Option<DecodedFragment<'_>> {
    if bytes.len() < MESSAGE_FRAME_HEADER_LEN || *bytes.first()? != FrameKind::Message.code() {
        return None;
    }

    let channel_id = ChannelId::new(*bytes.get(1)?);
    let message_id = MessageId::new(u32::from_le_bytes(bytes.get(2..6)?.try_into().ok()?));
    let message_len = u16::from_le_bytes(bytes.get(6..8)?.try_into().ok()?) as usize;
    let fragment_offset = u16::from_le_bytes(bytes.get(8..10)?.try_into().ok()?) as usize;
    let flags = *bytes.get(10)?;
    let fragment = bytes.get(MESSAGE_FRAME_HEADER_LEN..)?;

    let end = fragment_offset.checked_add(fragment.len())?;

    if end > message_len {
        return None;
    }

    Some(DecodedFragment {
        packet_number: header.packet_number,
        ack_eliciting: header.packet_flags & Flags::ACK_ELICITING.bits() != 0,
        channel_id,
        message_id,
        message_len,
        fragment_offset,
        flags,
        bytes: fragment,
    })
}
