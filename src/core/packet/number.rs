//! Message-scoped packet index primitives.

use crate::core::MessageId;

/// A packet index scoped to one message.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PacketIndex(pub u16);

impl PacketIndex {
    /// First packet index in every message.
    pub const ZERO: Self = Self(0);

    /// Creates a packet index from its raw value.
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    /// Returns the raw packet index value.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// Returns the next packet index using wrapping arithmetic.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// Stable identity of a packet fragment inside one message.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PacketKey {
    /// Message this packet belongs to.
    pub message_id: MessageId,
    /// Packet index scoped to `message_id`.
    pub packet_index: PacketIndex,
}

impl PacketKey {
    /// Creates a packet key.
    #[must_use]
    pub const fn new(message_id: MessageId, packet_index: PacketIndex) -> Self {
        Self {
            message_id,
            packet_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PacketIndex;

    #[test]
    fn next_wraps_at_u16_max() {
        assert_eq!(PacketIndex::new(u16::MAX).next(), PacketIndex::ZERO);
    }
}
