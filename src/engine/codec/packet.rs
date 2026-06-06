//! v1 draft packet byte layout glue.

use crate::core::{
    Error, Flags, MessageId, PacketIndex, PacketKey, PacketType, Result,
    packet::header::{
        ACK_PACKET_HEADER_LEN, DATA_PACKET_HEADER_LEN, LIVENESS_PACKET_HEADER_LEN,
        LOG_PACKET_HEADER_LEN, PacketHeader,
    },
};

use crate::engine::config::MAX_WIRE_BYTES;

/// Decoded MVP packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketDecode<'a> {
    /// Data packet carrying one message fragment.
    Data(DecodedFragment<'a>),
    /// Log packet carrying one best-effort log fragment.
    Log(DecodedFragment<'a>),
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
    pub(crate) key: PacketKey,
}

/// Decoded liveness packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedLiveness {
    pub(crate) packet_type: PacketType,
}

/// Decoded message fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedFragment<'a> {
    pub(crate) header: PacketHeader,
    pub(crate) packet_type: PacketType,
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
    if matches!(
        bytes.first().and_then(|code| PacketType::from_code(*code)),
        Some(PacketType::Ack)
    ) {
        return ack_from_packet_bytes(bytes)
            .map(PacketDecode::Ack)
            .unwrap_or(PacketDecode::Malformed);
    }

    if matches!(
        bytes.first().and_then(|code| PacketType::from_code(*code)),
        Some(PacketType::Ping | PacketType::Pong)
    ) {
        return liveness_from_packet_bytes(bytes)
            .map(|liveness| match liveness.packet_type {
                PacketType::Ping => PacketDecode::Ping(liveness),
                PacketType::Pong => PacketDecode::Pong(liveness),
                PacketType::Data | PacketType::Log | PacketType::Ack => PacketDecode::Malformed,
            })
            .unwrap_or(PacketDecode::Malformed);
    }

    let Some(header) = packet_header_from_bytes(bytes) else {
        return PacketDecode::Malformed;
    };

    match header.packet_type {
        PacketType::Data => fragment_from_packet_bytes(header, bytes)
            .map(PacketDecode::Data)
            .unwrap_or(PacketDecode::Malformed),
        PacketType::Log => fragment_from_packet_bytes(header, bytes)
            .map(PacketDecode::Log)
            .unwrap_or(PacketDecode::Malformed),
        PacketType::Ack => PacketDecode::Malformed,
        PacketType::Ping | PacketType::Pong => PacketDecode::Malformed,
    }
}

fn packet_header_from_bytes(bytes: &[u8]) -> Option<PacketHeader> {
    let packet_type = PacketType::from_code(*bytes.first()?)?;

    match packet_type {
        PacketType::Data => data_header_from_bytes(bytes),
        PacketType::Log => log_header_from_bytes(bytes),
        PacketType::Ack => None,
        PacketType::Ping | PacketType::Pong => None,
    }
}

fn data_header_from_bytes(bytes: &[u8]) -> Option<PacketHeader> {
    if bytes.len() < DATA_PACKET_HEADER_LEN {
        return None;
    }

    let flags = Flags::from_bits(*bytes.get(1)?);
    let message_id = MessageId::new(u32::from_le_bytes(bytes.get(2..6)?.try_into().ok()?));
    let packet_index = PacketIndex::new(u16::from_le_bytes(bytes.get(6..8)?.try_into().ok()?));
    let message_len = u16::from_le_bytes(bytes.get(8..10)?.try_into().ok()?) as usize;
    let fragment_offset = u16::from_le_bytes(bytes.get(10..12)?.try_into().ok()?) as usize;

    Some(PacketHeader::data(
        packet_index,
        flags,
        message_id,
        message_len,
        fragment_offset,
    ))
}

fn log_header_from_bytes(bytes: &[u8]) -> Option<PacketHeader> {
    if bytes.len() < LOG_PACKET_HEADER_LEN {
        return None;
    }

    let message_id = MessageId::new(u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?));
    let packet_index = PacketIndex::new(u16::from_le_bytes(bytes.get(5..7)?.try_into().ok()?));
    let message_len = u16::from_le_bytes(bytes.get(7..9)?.try_into().ok()?) as usize;
    let fragment_offset = u16::from_le_bytes(bytes.get(9..11)?.try_into().ok()?) as usize;

    Some(PacketHeader::log(
        packet_index,
        message_id,
        message_len,
        fragment_offset,
    ))
}

fn ack_from_packet_bytes(bytes: &[u8]) -> Option<DecodedAck> {
    if bytes.len() != ACK_PACKET_HEADER_LEN || *bytes.first()? != PacketType::Ack.code() {
        return None;
    }

    Some(DecodedAck {
        key: PacketKey::new(
            MessageId::new(u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?)),
            PacketIndex::new(u16::from_le_bytes(bytes.get(5..7)?.try_into().ok()?)),
        ),
    })
}

fn fragment_from_packet_bytes(header: PacketHeader, bytes: &[u8]) -> Option<DecodedFragment<'_>> {
    let header_len = match header.packet_type {
        PacketType::Data => DATA_PACKET_HEADER_LEN,
        PacketType::Log => LOG_PACKET_HEADER_LEN,
        PacketType::Ack | PacketType::Ping | PacketType::Pong => return None,
    };

    if bytes.len() < header_len {
        return None;
    }

    let fragment = bytes.get(header_len..)?;

    let end = header.fragment_offset().checked_add(fragment.len())?;

    if end > header.message_len() {
        return None;
    }

    Some(DecodedFragment {
        header,
        packet_type: header.packet_type,
        message_id: header.message_id(),
        message_len: header.message_len(),
        fragment_offset: header.fragment_offset(),
        bytes: fragment,
    })
}

fn liveness_from_packet_bytes(bytes: &[u8]) -> Option<DecodedLiveness> {
    if bytes.len() != LIVENESS_PACKET_HEADER_LEN {
        return None;
    }

    let packet_type = PacketType::from_code(*bytes.first()?)?;
    if !matches!(packet_type, PacketType::Ping | PacketType::Pong) {
        return None;
    }

    Some(DecodedLiveness { packet_type })
}
