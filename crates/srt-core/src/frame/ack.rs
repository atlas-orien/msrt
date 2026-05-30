//! ACK frame primitives.

use crate::PacketNumber;

/// ACK frame carrying a single acknowledged packet number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AckFrame {
    /// Largest acknowledged packet number.
    pub largest_acknowledged: PacketNumber,
}

impl AckFrame {
    /// Creates an ACK frame.
    #[must_use]
    pub const fn new(largest_acknowledged: PacketNumber) -> Self {
        Self {
            largest_acknowledged,
        }
    }
}
