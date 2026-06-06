//! LOG packet header.

use crate::core::{MessageId, PacketIndex};

/// Header for best-effort log fragments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogHeader {
    /// Message this fragment belongs to.
    pub message_id: MessageId,
    /// Packet index scoped to `message_id`.
    pub packet_index: PacketIndex,
    /// Complete message length in bytes.
    pub message_len: usize,
    /// Offset of this fragment inside the complete message.
    pub fragment_offset: usize,
}

impl LogHeader {
    /// Creates a LOG header.
    #[must_use]
    pub const fn new(
        message_id: MessageId,
        packet_index: PacketIndex,
        message_len: usize,
        fragment_offset: usize,
    ) -> Self {
        Self {
            message_id,
            packet_index,
            message_len,
            fragment_offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LogHeader;
    use crate::core::{MessageId, PacketIndex};

    #[test]
    fn log_header_has_no_ack_flags() {
        let header = LogHeader::new(MessageId::new(7), PacketIndex::new(2), 100, 20);

        assert_eq!(header.message_id, MessageId::new(7));
        assert_eq!(header.packet_index, PacketIndex::new(2));
        assert_eq!(header.message_len, 100);
        assert_eq!(header.fragment_offset, 20);
    }
}
