//! MVP packet byte layout glue.

use srt_core::{Error, MessageId, PacketNumber, Result};

use crate::{
    MAX_WIRE_BYTES,
    layout::{ACK_PACKET_LEN, FRAGMENT_FIRST, FRAGMENT_LAST, PACKET_META_LEN, PACKET_NUMBER_LEN},
};

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
    pub(crate) acknowledged: PacketNumber,
}

/// Decoded message fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedFragment<'a> {
    pub(crate) packet_number: PacketNumber,
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
    if bytes.len() == ACK_PACKET_LEN {
        return ack_from_packet_bytes(bytes)
            .map(PacketDecode::Ack)
            .unwrap_or(PacketDecode::Malformed);
    }

    fragment_from_packet_bytes(bytes)
        .map(PacketDecode::Data)
        .unwrap_or(PacketDecode::Malformed)
}

pub(crate) const fn fragment_flags(offset: usize, end: usize, message_len: usize) -> u8 {
    let mut flags = 0;

    if offset == 0 {
        flags |= FRAGMENT_FIRST;
    }

    if end == message_len {
        flags |= FRAGMENT_LAST;
    }

    flags
}

fn ack_from_packet_bytes(bytes: &[u8]) -> Option<DecodedAck> {
    let start = PACKET_NUMBER_LEN;
    let end = start + PACKET_NUMBER_LEN;
    let raw = bytes.get(start..end)?;
    let raw = u32::from_le_bytes(raw.try_into().ok()?);

    Some(DecodedAck {
        acknowledged: PacketNumber::new(raw),
    })
}

fn fragment_from_packet_bytes(bytes: &[u8]) -> Option<DecodedFragment<'_>> {
    if bytes.len() < PACKET_META_LEN {
        return None;
    }

    let packet_number = packet_number_from_packet_bytes(bytes)?;
    let message_id = MessageId::new(u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?));
    let message_len = u16::from_le_bytes(bytes.get(8..10)?.try_into().ok()?) as usize;
    let fragment_offset = u16::from_le_bytes(bytes.get(10..12)?.try_into().ok()?) as usize;
    let flags = *bytes.get(12)?;
    let fragment = bytes.get(PACKET_META_LEN..)?;

    let end = fragment_offset.checked_add(fragment.len())?;

    if end > message_len {
        return None;
    }

    Some(DecodedFragment {
        packet_number,
        message_id,
        message_len,
        fragment_offset,
        flags,
        bytes: fragment,
    })
}

fn packet_number_from_packet_bytes(bytes: &[u8]) -> Option<PacketNumber> {
    let raw = bytes.get(..PACKET_NUMBER_LEN)?;
    let raw = u32::from_le_bytes(raw.try_into().ok()?);

    Some(PacketNumber::new(raw))
}
