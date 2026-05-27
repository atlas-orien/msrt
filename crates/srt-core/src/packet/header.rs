//! Packet header structure.

pub mod flags;
pub mod seq;
pub mod stream_id;

pub use flags::Flags;
pub use seq::Seq;
pub use stream_id::StreamId;

use super::PacketKind;

/// Metadata shared by every protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketHeader {
    /// Packet category.
    pub kind: PacketKind,
    /// Logical stream that owns the packet.
    pub stream_id: StreamId,
    /// Packet sequence number.
    pub seq: Seq,
    /// Packet flags.
    pub flags: Flags,
}

impl PacketHeader {
    /// Creates a packet header.
    #[must_use]
    pub const fn new(kind: PacketKind, stream_id: StreamId, seq: Seq, flags: Flags) -> Self {
        Self {
            kind,
            stream_id,
            seq,
            flags,
        }
    }

    /// Returns whether this packet should elicit an acknowledgement.
    #[must_use]
    pub const fn is_ack_eliciting(self) -> bool {
        self.flags.contains(Flags::ACK_ELICITING)
    }
}
