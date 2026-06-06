//! v1 draft packet byte layout glue.

use crate::core::{
    ChannelId, Error, Flags, MessageId, PacketIndex, PacketType, Result,
    packet::header::{PACKET_HEADER_LEN, PacketHeader},
};

use crate::engine::config::MAX_WIRE_BYTES;

/// Decoded MVP packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketDecode<'a> {
    /// Data packet carrying one message fragment.
    Data(DecodedFragment<'a>),
    /// ACK packet.
    Ack(DecodedAck),
    /// PING packet.
    Ping(DecodedLiveness),
    /// PONG packet.
    Pong(DecodedLiveness),
    /// Malformed packet bytes.
    Malformed,
}

/// Decoded ACK packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedAck {
    pub(crate) header: PacketHeader,
}

/// Decoded liveness packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedLiveness {
    pub(crate) header: PacketHeader,
}

/// Decoded message fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedFragment<'a> {
    pub(crate) header: PacketHeader,
    pub(crate) channel_id: ChannelId,
    pub(crate) message_id: MessageId,
    pub(crate) message_len: usize,
    pub(crate) fragment_offset: usize,
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

    pub(crate) fn decode(&self) -> PacketDecode<'_> {
        decode_packet_bytes(self.as_bytes())
    }
}

fn decode_packet_bytes(bytes: &[u8]) -> PacketDecode<'_> {
    let Some(header) = packet_header_from_bytes(bytes) else {
        return PacketDecode::Malformed;
    };

    let payload_bytes = &bytes[PACKET_HEADER_LEN..];

    match header.packet_type {
        PacketType::Data => fragment_from_packet_bytes(header, bytes)
            .map(PacketDecode::Data)
            .unwrap_or(PacketDecode::Malformed),
        PacketType::Log => PacketDecode::Malformed,
        PacketType::Ack => ack_from_packet_bytes(header, payload_bytes)
            .map(PacketDecode::Ack)
            .unwrap_or(PacketDecode::Malformed),
        PacketType::Ping => liveness_from_packet_bytes(header, payload_bytes)
            .map(PacketDecode::Ping)
            .unwrap_or(PacketDecode::Malformed),
        PacketType::Pong => liveness_from_packet_bytes(header, payload_bytes)
            .map(PacketDecode::Pong)
            .unwrap_or(PacketDecode::Malformed),
    }
}

fn packet_header_from_bytes(bytes: &[u8]) -> Option<PacketHeader> {
    if bytes.len() < PACKET_HEADER_LEN {
        return None;
    }

    let packet_type = PacketType::from_code(*bytes.first()?)?;
    let flags = Flags::from_bits(*bytes.get(1)?);
    let channel_id = ChannelId::new(*bytes.get(2)?);
    let message_id = MessageId::new(u32::from_le_bytes(bytes.get(3..7)?.try_into().ok()?));
    let packet_index = PacketIndex::new(u16::from_le_bytes(bytes.get(7..9)?.try_into().ok()?));
    let message_len = u16::from_le_bytes(bytes.get(9..11)?.try_into().ok()?) as usize;
    let fragment_offset = u16::from_le_bytes(bytes.get(11..13)?.try_into().ok()?) as usize;

    match packet_type {
        PacketType::Data => Some(PacketHeader::data(
            packet_index,
            flags,
            channel_id,
            message_id,
            message_len,
            fragment_offset,
        )),
        PacketType::Log => None,
        PacketType::Ack => {
            if flags != Flags::EMPTY || message_len != 0 || fragment_offset != 0 {
                return None;
            }

            Some(PacketHeader::ack(crate::core::PacketKey::new(
                channel_id,
                message_id,
                packet_index,
            )))
        }
        PacketType::Ping => {
            if flags != Flags::EMPTY
                || !channel_id.is_liveness()
                || message_len != 0
                || fragment_offset != 0
            {
                return None;
            }

            Some(PacketHeader::ping(message_id))
        }
        PacketType::Pong => {
            if flags != Flags::EMPTY
                || !channel_id.is_liveness()
                || message_len != 0
                || fragment_offset != 0
            {
                return None;
            }

            Some(PacketHeader::pong(message_id))
        }
    }
}

fn ack_from_packet_bytes(header: PacketHeader, bytes: &[u8]) -> Option<DecodedAck> {
    if !bytes.is_empty()
        || header.flags() != Flags::EMPTY
        || header.message_len() != 0
        || header.fragment_offset() != 0
    {
        return None;
    }

    Some(DecodedAck { header })
}

fn fragment_from_packet_bytes(header: PacketHeader, bytes: &[u8]) -> Option<DecodedFragment<'_>> {
    if bytes.len() < PACKET_HEADER_LEN {
        return None;
    }

    let fragment = bytes.get(PACKET_HEADER_LEN..)?;

    let end = header.fragment_offset().checked_add(fragment.len())?;

    if end > header.message_len() {
        return None;
    }

    Some(DecodedFragment {
        header,
        channel_id: header.channel_id(),
        message_id: header.message_id(),
        message_len: header.message_len(),
        fragment_offset: header.fragment_offset(),
        bytes: fragment,
    })
}

fn liveness_from_packet_bytes(header: PacketHeader, bytes: &[u8]) -> Option<DecodedLiveness> {
    if !bytes.is_empty()
        || !header.channel_id().is_liveness()
        || header.message_len() != 0
        || header.fragment_offset() != 0
    {
        return None;
    }

    Some(DecodedLiveness { header })
}
