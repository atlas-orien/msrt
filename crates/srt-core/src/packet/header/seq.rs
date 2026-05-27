//! Packet header sequence number primitives.

/// A packet sequence number.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Seq(pub u32);

impl Seq {
    /// First sequence number used by a fresh endpoint.
    pub const ZERO: Self = Self(0);

    /// Creates a sequence number from its raw value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw sequence number value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the next sequence number using wrapping arithmetic.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

#[cfg(test)]
mod tests {
    use super::Seq;

    #[test]
    fn next_wraps_at_u32_max() {
        assert_eq!(Seq::new(u32::MAX).next(), Seq::ZERO);
    }
}
