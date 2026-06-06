//! Packet header structure.

pub mod ack;
pub mod body;
pub mod data;
pub mod flags;
pub mod len;
pub mod liveness;
pub mod log;

pub use ack::AckHeader;
pub use body::PacketHeaderBody;
pub use data::DataHeader;
pub use flags::Flags;
pub use len::{
    ACK_PACKET_HEADER_LEN, DATA_PACKET_HEADER_LEN, LIVENESS_PACKET_HEADER_LEN,
    LOG_PACKET_HEADER_LEN,
};
pub use liveness::{PingHeader, PongHeader};
pub use log::LogHeader;

use super::{PacketIndex, PacketKey, PacketType};

use crate::core::MessageId;

/// Metadata shared by every protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketHeader {
    /// Packet type.
    pub packet_type: PacketType,
    /// Kind-specific header fields.
    pub body: PacketHeaderBody,
}

impl PacketHeader {
    /// Creates a DATA packet header.
    #[must_use]
    pub const fn data(
        packet_index: PacketIndex,
        flags: Flags,
        message_id: MessageId,
        message_len: usize,
        fragment_offset: usize,
    ) -> Self {
        Self {
            packet_type: PacketType::Data,
            body: PacketHeaderBody::Data {
                header: DataHeader::new(
                    flags,
                    message_id,
                    packet_index,
                    message_len,
                    fragment_offset,
                ),
            },
        }
    }

    /// Creates a DATA packet header from the target DATA header.
    #[must_use]
    pub const fn from_data_header(header: DataHeader) -> Self {
        Self::data(
            header.packet_index,
            header.flags,
            header.message_id,
            header.message_len,
            header.fragment_offset,
        )
    }

    /// Creates a LOG packet header.
    #[must_use]
    pub const fn log(
        packet_index: PacketIndex,
        message_id: MessageId,
        message_len: usize,
        fragment_offset: usize,
    ) -> Self {
        Self {
            packet_type: PacketType::Log,
            body: PacketHeaderBody::Log {
                header: LogHeader::new(message_id, packet_index, message_len, fragment_offset),
            },
        }
    }

    /// Creates a LOG packet header from the target LOG header.
    #[must_use]
    pub const fn from_log_header(header: LogHeader) -> Self {
        Self::log(
            header.packet_index,
            header.message_id,
            header.message_len,
            header.fragment_offset,
        )
    }

    /// Creates an ACK packet header.
    #[must_use]
    pub const fn ack(key: PacketKey) -> Self {
        Self {
            packet_type: PacketType::Ack,
            body: PacketHeaderBody::Ack {
                header: AckHeader::new(key.message_id, key.packet_index),
            },
        }
    }

    /// Creates a PING packet header.
    #[must_use]
    pub const fn ping(message_id: MessageId) -> Self {
        Self {
            packet_type: PacketType::Ping,
            body: PacketHeaderBody::Ping {
                header: PingHeader::new(),
                legacy_message_id: message_id,
            },
        }
    }

    /// Creates a PONG packet header.
    #[must_use]
    pub const fn pong(message_id: MessageId) -> Self {
        Self {
            packet_type: PacketType::Pong,
            body: PacketHeaderBody::Pong {
                header: PongHeader::new(),
                legacy_message_id: message_id,
            },
        }
    }

    /// Returns whether this packet should elicit an acknowledgement.
    #[must_use]
    pub const fn is_ack_eliciting(self) -> bool {
        self.flags().contains(Flags::ACK_ELICITING)
    }

    /// Returns whether this packet kind can carry payload bytes.
    #[must_use]
    pub const fn can_carry_payload(self) -> bool {
        matches!(self.packet_type, PacketType::Data | PacketType::Log)
    }

    /// Returns packet flags, or empty flags for packet kinds that do not carry flags.
    #[must_use]
    pub const fn flags(self) -> Flags {
        match self.body {
            PacketHeaderBody::Data { header, .. } => header.flags,
            PacketHeaderBody::Log { .. } => Flags::EMPTY,
            PacketHeaderBody::Ack { .. }
            | PacketHeaderBody::Ping { .. }
            | PacketHeaderBody::Pong { .. } => Flags::EMPTY,
        }
    }

    /// Returns the message id associated with this packet.
    #[must_use]
    pub const fn message_id(self) -> MessageId {
        match self.body {
            PacketHeaderBody::Data { header, .. } => header.message_id,
            PacketHeaderBody::Log { header, .. } => header.message_id,
            PacketHeaderBody::Ack { header, .. } => header.message_id,
            PacketHeaderBody::Ping {
                legacy_message_id, ..
            }
            | PacketHeaderBody::Pong {
                legacy_message_id, ..
            } => legacy_message_id,
        }
    }

    /// Returns the packet index associated with this packet.
    #[must_use]
    pub const fn packet_index(self) -> PacketIndex {
        match self.body {
            PacketHeaderBody::Data { header, .. } => header.packet_index,
            PacketHeaderBody::Log { header, .. } => header.packet_index,
            PacketHeaderBody::Ack { header, .. } => header.packet_index,
            PacketHeaderBody::Ping { .. } | PacketHeaderBody::Pong { .. } => PacketIndex::ZERO,
        }
    }

    /// Returns the complete message length for fragment packets.
    #[must_use]
    pub const fn message_len(self) -> usize {
        match self.body {
            PacketHeaderBody::Data { header, .. } => header.message_len,
            PacketHeaderBody::Log { header, .. } => header.message_len,
            PacketHeaderBody::Ack { .. }
            | PacketHeaderBody::Ping { .. }
            | PacketHeaderBody::Pong { .. } => 0,
        }
    }

    /// Returns the fragment offset for fragment packets.
    #[must_use]
    pub const fn fragment_offset(self) -> usize {
        match self.body {
            PacketHeaderBody::Data { header, .. } => header.fragment_offset,
            PacketHeaderBody::Log { header, .. } => header.fragment_offset,
            PacketHeaderBody::Ack { .. }
            | PacketHeaderBody::Ping { .. }
            | PacketHeaderBody::Pong { .. } => 0,
        }
    }

    /// Returns this packet's stable message-scoped identity.
    #[must_use]
    pub const fn key(self) -> PacketKey {
        PacketKey::new(self.message_id(), self.packet_index())
    }
}
