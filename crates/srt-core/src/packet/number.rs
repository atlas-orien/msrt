//! Packet number primitive.

/// A packet number used by ack, deduplication, and retransmission logic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PacketNumber(pub u32);

impl PacketNumber {
    /// First packet number used by a fresh endpoint.
    pub const ZERO: Self = Self(0);

    /// Creates a packet number from its raw value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw packet number value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the next packet number using wrapping arithmetic.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

#[cfg(test)]
mod tests {
    use super::PacketNumber;

    #[test]
    fn next_wraps_at_u32_max() {
        assert_eq!(PacketNumber::new(u32::MAX).next(), PacketNumber::ZERO);
    }
}
