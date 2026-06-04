//! Packet header structure.

pub mod flags;

pub use flags::Flags;

use super::{PacketIndex, PacketKey, PacketType};

use crate::core::{
    ChannelId, MessageFlags, MessageId,
    message::{
        CHANNEL_ID_LEN, FRAGMENT_FLAGS_LEN, FRAGMENT_OFFSET_LEN, MESSAGE_ID_LEN, MESSAGE_LEN_LEN,
    },
};

/// Encoded packet type length in bytes.
pub(crate) const PACKET_TYPE_LEN: usize = core::mem::size_of::<u8>();
/// Encoded packet flags length in bytes.
pub(crate) const PACKET_FLAGS_LEN: usize = core::mem::size_of::<u8>();
/// Encoded message-scoped packet index length in bytes.
pub(crate) const PACKET_INDEX_LEN: usize = core::mem::size_of::<u16>();
/// Encoded packet header length in bytes.
pub(crate) const PACKET_HEADER_LEN: usize = PACKET_TYPE_LEN
    + PACKET_FLAGS_LEN
    + CHANNEL_ID_LEN
    + MESSAGE_ID_LEN
    + PACKET_INDEX_LEN
    + MESSAGE_LEN_LEN
    + FRAGMENT_OFFSET_LEN
    + FRAGMENT_FLAGS_LEN;

/// Metadata shared by every protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketHeader {
    /// Packet type.
    pub packet_type: PacketType,
    /// Packet index scoped to `message_id`.
    pub packet_index: PacketIndex,
    /// Packet flags.
    pub flags: Flags,
    /// Logical channel carrying this message fragment.
    pub channel_id: ChannelId,
    /// Message this fragment belongs to.
    pub message_id: MessageId,
    /// Complete message length in bytes.
    pub message_len: usize,
    /// Offset of this fragment inside the complete message.
    pub fragment_offset: usize,
    /// Message fragment flags.
    pub fragment_flags: MessageFlags,
}

impl PacketHeader {
    /// Creates a DATA packet header.
    #[must_use]
    pub const fn data(
        packet_index: PacketIndex,
        flags: Flags,
        channel_id: ChannelId,
        message_id: MessageId,
        message_len: usize,
        fragment_offset: usize,
        fragment_flags: MessageFlags,
    ) -> Self {
        Self {
            packet_type: PacketType::Data,
            packet_index,
            flags,
            channel_id,
            message_id,
            message_len,
            fragment_offset,
            fragment_flags,
        }
    }

    /// Creates an ACK packet header.
    #[must_use]
    pub const fn ack(key: PacketKey) -> Self {
        Self {
            packet_type: PacketType::Ack,
            flags: Flags::EMPTY,
            channel_id: key.channel_id,
            message_id: key.message_id,
            packet_index: key.packet_index,
            message_len: 0,
            fragment_offset: 0,
            fragment_flags: MessageFlags::EMPTY,
        }
    }

    /// Creates a PING packet header.
    #[must_use]
    pub const fn ping(message_id: MessageId) -> Self {
        Self::liveness(PacketType::Ping, message_id)
    }

    /// Creates a PONG packet header.
    #[must_use]
    pub const fn pong(message_id: MessageId) -> Self {
        Self::liveness(PacketType::Pong, message_id)
    }

    /// Returns whether this packet should elicit an acknowledgement.
    #[must_use]
    pub const fn is_ack_eliciting(self) -> bool {
        self.flags.contains(Flags::ACK_ELICITING)
    }

    const fn liveness(packet_type: PacketType, message_id: MessageId) -> Self {
        Self {
            packet_type,
            flags: Flags::EMPTY,
            channel_id: ChannelId::LIVENESS,
            message_id,
            packet_index: PacketIndex::ZERO,
            message_len: 0,
            fragment_offset: 0,
            fragment_flags: MessageFlags::EMPTY,
        }
    }

    /// Returns this packet's stable message-scoped identity.
    #[must_use]
    pub const fn key(self) -> PacketKey {
        PacketKey::new(self.channel_id, self.message_id, self.packet_index)
    }
}
