//! Packet header structure.

pub mod flags;

pub use flags::Flags;

use super::{PacketNumber, PacketType};

/// Metadata shared by every protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketHeader {
    /// Packet type.
    pub packet_type: PacketType,
    /// Packet number used by acknowledgement and retransmission logic.
    pub packet_number: PacketNumber,
    /// Packet flags.
    pub flags: Flags,
}

impl PacketHeader {
    /// Creates a packet header.
    #[must_use]
    pub const fn new(packet_type: PacketType, packet_number: PacketNumber, flags: Flags) -> Self {
        Self {
            packet_type,
            packet_number,
            flags,
        }
    }

    /// Returns whether this packet should elicit an acknowledgement.
    #[must_use]
    pub const fn is_ack_eliciting(self) -> bool {
        self.flags.contains(Flags::ACK_ELICITING)
    }
}
