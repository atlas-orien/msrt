//! ACK frame primitives.

use crate::PacketNumber;

/// Maximum ACK ranges carried by the v1 fixed ACK frame.
pub const MAX_ACK_RANGES: usize = 4;

/// Inclusive packet number range carried by an ACK frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AckRange {
    /// First acknowledged packet number in the range.
    pub start: PacketNumber,
    /// Last acknowledged packet number in the range.
    pub end: PacketNumber,
}

impl AckRange {
    /// Creates an ACK range.
    #[must_use]
    pub const fn new(start: PacketNumber, end: PacketNumber) -> Self {
        Self { start, end }
    }

    /// Returns whether this range contains `packet_number`.
    #[must_use]
    pub const fn contains(self, packet_number: PacketNumber) -> bool {
        self.start.get() <= packet_number.get() && packet_number.get() <= self.end.get()
    }
}

/// ACK frame carrying fixed-capacity acknowledged packet ranges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AckFrame {
    /// Largest acknowledged packet number.
    pub largest_acknowledged: PacketNumber,
    /// Number of valid ranges.
    pub range_count: u8,
    /// Fixed range storage.
    pub ranges: [AckRange; MAX_ACK_RANGES],
}

impl AckFrame {
    /// Creates an ACK frame.
    #[must_use]
    pub const fn new(largest_acknowledged: PacketNumber) -> Self {
        let range = AckRange::new(largest_acknowledged, largest_acknowledged);

        Self {
            largest_acknowledged,
            range_count: 1,
            ranges: [range; MAX_ACK_RANGES],
        }
    }

    /// Creates an ACK frame from fixed ranges.
    #[must_use]
    pub const fn from_ranges(ranges: [AckRange; MAX_ACK_RANGES], range_count: u8) -> Self {
        let mut largest = PacketNumber::ZERO;
        let mut index = 0;

        while index < MAX_ACK_RANGES {
            if index < range_count as usize && ranges[index].end.get() > largest.get() {
                largest = ranges[index].end;
            }
            index += 1;
        }

        Self {
            largest_acknowledged: largest,
            range_count,
            ranges,
        }
    }

    /// Returns valid ACK ranges.
    #[must_use]
    pub const fn ranges(self) -> [AckRange; MAX_ACK_RANGES] {
        self.ranges
    }

    /// Returns whether this ACK frame acknowledges `packet_number`.
    #[must_use]
    pub const fn acknowledges(self, packet_number: PacketNumber) -> bool {
        let mut index = 0;

        while index < MAX_ACK_RANGES {
            if index < self.range_count as usize && self.ranges[index].contains(packet_number) {
                return true;
            }
            index += 1;
        }

        false
    }
}
