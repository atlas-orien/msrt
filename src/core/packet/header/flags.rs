//! Packet header flag primitives.

/// Protocol flags carried by packets.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Flags(pub u8);

impl Flags {
    /// Empty flag set.
    pub const EMPTY: Self = Self(0);

    /// Packet is ack-eliciting.
    pub const ACK_ELICITING: Self = Self(1 << 0);

    /// Packet contains realtime-sensitive frames.
    pub const REALTIME: Self = Self(1 << 1);

    /// Creates flags from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Returns the raw flag bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Returns whether all bits from `other` are set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns a new flag set with `other` included.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Flags;

    #[test]
    fn union_contains_combined_flags() {
        let flags = Flags::ACK_ELICITING.union(Flags::REALTIME);

        assert!(flags.contains(Flags::ACK_ELICITING));
        assert!(flags.contains(Flags::REALTIME));
        assert!(!flags.contains(Flags::from_bits(1 << 7)));
    }
}
