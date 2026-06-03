//! ACK primitives.

use crate::core::PacketNumber;
use crate::core::packet::header::PACKET_NUMBER_LEN;

/// Maximum ACK ranges carried by the v1 fixed ACK.
pub const MAX_ACK_RANGES: usize = 4;
/// Encoded ACK range count length in bytes.
pub(crate) const ACK_RANGE_COUNT_LEN: usize = core::mem::size_of::<u8>();
/// Encoded ACK payload length in bytes.
pub(crate) const ACK_LEN: usize = PACKET_NUMBER_LEN
    + ACK_RANGE_COUNT_LEN
    + MAX_ACK_RANGES * 2 * PACKET_NUMBER_LEN;

/// Inclusive packet number range carried by an ACK.
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

/// ACK carrying fixed-capacity acknowledged packet ranges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ack {
    /// Largest acknowledged packet number.
    pub largest_acknowledged: PacketNumber,
    /// Number of valid ranges.
    pub range_count: u8,
    /// Fixed range storage.
    pub ranges: [AckRange; MAX_ACK_RANGES],
}

impl Ack {
    /// Creates an ACK.
    #[must_use]
    pub const fn new(largest_acknowledged: PacketNumber) -> Self {
        let range = AckRange::new(largest_acknowledged, largest_acknowledged);

        Self {
            largest_acknowledged,
            range_count: 1,
            ranges: [range; MAX_ACK_RANGES],
        }
    }

    /// Creates an ACK from fixed ranges.
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

    /// Returns whether this ACK acknowledges `packet_number`.
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
