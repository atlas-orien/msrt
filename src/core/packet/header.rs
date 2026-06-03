//! Packet header structure.

pub mod flags;

pub use flags::Flags;

use super::{PacketNumber, PacketType};

/// Encoded packet type length in bytes.
pub(crate) const PACKET_TYPE_LEN: usize = core::mem::size_of::<u8>();
/// Encoded packet flags length in bytes.
pub(crate) const PACKET_FLAGS_LEN: usize = core::mem::size_of::<u8>();
/// Encoded packet number length in bytes.
pub(crate) const PACKET_NUMBER_LEN: usize = core::mem::size_of::<u32>();
/// Encoded packet header length in bytes.
pub(crate) const PACKET_HEADER_LEN: usize = PACKET_TYPE_LEN + PACKET_FLAGS_LEN + PACKET_NUMBER_LEN;

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
