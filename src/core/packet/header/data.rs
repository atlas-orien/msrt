//! DATA packet header.

use super::Flags;
use crate::core::{MessageId, PacketIndex, PacketType};

/// Header for reliable message fragments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataHeader {
    /// Packet flags.
    pub flags: Flags,
    /// Message this fragment belongs to.
    pub message_id: MessageId,
    /// Packet index scoped to `message_id`.
    pub packet_index: PacketIndex,
    /// Complete message length in bytes.
    pub message_len: usize,
    /// Offset of this fragment inside the complete message.
    pub fragment_offset: usize,
}

impl DataHeader {
    /// Creates a DATA header.
    #[must_use]
    pub const fn new(
        flags: Flags,
        message_id: MessageId,
        packet_index: PacketIndex,
        message_len: usize,
        fragment_offset: usize,
    ) -> Self {
        Self {
            flags,
            message_id,
            packet_index,
            message_len,
            fragment_offset,
        }
    }

    /// Returns the packet type represented by this header.
    #[must_use]
    pub const fn packet_type(self) -> PacketType {
        PacketType::Data
    }

    /// Returns whether this packet should elicit an acknowledgement.
    #[must_use]
    pub const fn is_ack_eliciting(self) -> bool {
        self.flags.contains(Flags::ACK_ELICITING)
    }
}

#[cfg(test)]
mod tests {
    use super::DataHeader;
    use crate::core::{Flags, MessageId, PacketIndex, PacketType};

    #[test]
    fn data_header_tracks_ack_eliciting_flag() {
        let header = DataHeader::new(
            Flags::ACK_ELICITING,
            MessageId::new(7),
            PacketIndex::new(2),
            100,
            20,
        );

        assert_eq!(header.packet_type(), PacketType::Data);
        assert!(header.is_ack_eliciting());
    }
}
