//! Wire envelope flags.

/// Flags carried by the wire envelope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WireFlags(pub u8);

impl WireFlags {
    /// Empty wire flags.
    pub const EMPTY: Self = Self(0);

    /// The envelope checksum is present.
    pub const CHECKSUM_PRESENT: Self = Self(1 << 0);

    /// Creates wire flags from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Returns raw bits.
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
