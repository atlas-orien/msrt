//! Packet header structure.

pub mod flags;

pub use flags::Flags;

use super::{PacketNumber, PacketType};

use crate::core::{
    ChannelId, MessageFlags, MessageId,
    frame::message::{
        CHANNEL_ID_LEN, FRAGMENT_FLAGS_LEN, FRAGMENT_OFFSET_LEN, MESSAGE_ID_LEN, MESSAGE_LEN_LEN,
    },
};

/// Encoded packet type length in bytes.
pub(crate) const PACKET_TYPE_LEN: usize = core::mem::size_of::<u8>();
/// Encoded packet flags length in bytes.
pub(crate) const PACKET_FLAGS_LEN: usize = core::mem::size_of::<u8>();
/// Encoded packet number length in bytes.
pub(crate) const PACKET_NUMBER_LEN: usize = core::mem::size_of::<u32>();
/// Encoded packet header length in bytes.
pub(crate) const PACKET_HEADER_LEN: usize = PACKET_TYPE_LEN
    + PACKET_FLAGS_LEN
    + PACKET_NUMBER_LEN
    + CHANNEL_ID_LEN
    + MESSAGE_ID_LEN
    + MESSAGE_LEN_LEN
    + FRAGMENT_OFFSET_LEN
    + FRAGMENT_FLAGS_LEN;

/// Metadata shared by every protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketHeader {
    /// Packet type.
    pub packet_type: PacketType,
    /// Packet number used by acknowledgement and retransmission logic.
    pub packet_number: PacketNumber,
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
        packet_number: PacketNumber,
        flags: Flags,
        channel_id: ChannelId,
        message_id: MessageId,
        message_len: usize,
        fragment_offset: usize,
        fragment_flags: MessageFlags,
    ) -> Self {
        Self {
            packet_type: PacketType::Data,
            packet_number,
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
    pub const fn ack(packet_number: PacketNumber) -> Self {
        Self {
            packet_type: PacketType::Ack,
            packet_number,
            flags: Flags::EMPTY,
            channel_id: ChannelId::DEFAULT,
            message_id: MessageId::ZERO,
            message_len: 0,
            fragment_offset: 0,
            fragment_flags: MessageFlags::EMPTY,
        }
    }

    /// Returns whether this packet should elicit an acknowledgement.
    #[must_use]
    pub const fn is_ack_eliciting(self) -> bool {
        self.flags.contains(Flags::ACK_ELICITING)
    }
}
