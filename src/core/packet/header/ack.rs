//! ACK packet header.

use crate::core::{MessageId, PacketIndex};

/// Header for acknowledging one packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AckHeader {
    /// Message containing the acknowledged packet.
    pub message_id: MessageId,
    /// Acknowledged packet index scoped to `message_id`.
    pub packet_index: PacketIndex,
}

impl AckHeader {
    /// Creates an ACK header.
    #[must_use]
    pub const fn new(message_id: MessageId, packet_index: PacketIndex) -> Self {
        Self {
            message_id,
            packet_index,
        }
    }
}
