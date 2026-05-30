//! Runtime time primitives.

/// A monotonic protocol time value supplied by the embedding environment.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Instant(pub u64);

impl Instant {
    /// The zero instant.
    pub const ZERO: Self = Self(0);

    /// Creates an instant from raw ticks.
    #[must_use]
    pub const fn from_ticks(ticks: u64) -> Self {
        Self(ticks)
    }

    /// Returns raw ticks.
    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.0
    }

    /// Returns a later instant using saturating arithmetic.
    #[must_use]
    pub const fn saturating_add(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration.0))
    }

    /// Returns the duration since an earlier instant using saturating arithmetic.
    #[must_use]
    pub const fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration(self.0.saturating_sub(earlier.0))
    }
}

/// A protocol duration value supplied by the embedding environment.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Duration(pub u64);

impl Duration {
    /// The zero duration.
    pub const ZERO: Self = Self(0);

    /// Creates a duration from raw ticks.
    #[must_use]
    pub const fn from_ticks(ticks: u64) -> Self {
        Self(ticks)
    }

    /// Returns raw ticks.
    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.0
    }

    /// Returns whether the duration is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::{Duration, Instant};

    #[test]
    fn instant_uses_saturating_arithmetic() {
        assert_eq!(
            Instant::from_ticks(5).saturating_duration_since(Instant::from_ticks(9)),
            Duration::ZERO
        );
        assert_eq!(
            Instant::from_ticks(u64::MAX).saturating_add(Duration::from_ticks(1)),
            Instant::from_ticks(u64::MAX)
        );
    }
}
