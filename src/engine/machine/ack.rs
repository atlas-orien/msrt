//! ACK range collection.

use crate::core::{Ack, AckRange, MAX_ACK_RANGES, PacketNumber};

use crate::engine::config::MAX_ACK_TRACKED_PACKETS;

/// Fixed-capacity observed packet set used to build ACK ranges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AckRanges {
    packets: [Option<PacketNumber>; MAX_ACK_TRACKED_PACKETS],
    next: usize,
    len: usize,
}

impl AckRanges {
    pub(crate) const fn new() -> Self {
        Self {
            packets: [None; MAX_ACK_TRACKED_PACKETS],
            next: 0,
            len: 0,
        }
    }

    pub(crate) fn observe(&mut self, packet_number: PacketNumber) {
        if self
            .packets
            .iter()
            .flatten()
            .any(|current| *current == packet_number)
        {
            return;
        }

        if MAX_ACK_TRACKED_PACKETS == 0 {
            return;
        }

        self.packets[self.next] = Some(packet_number);
        self.next = (self.next + 1) % MAX_ACK_TRACKED_PACKETS;
        self.len = core::cmp::min(self.len + 1, MAX_ACK_TRACKED_PACKETS);
    }

    pub(crate) fn ack(&self) -> Ack {
        let mut numbers = [None; MAX_ACK_TRACKED_PACKETS];

        for (index, packet_number) in self.packets.iter().flatten().enumerate() {
            numbers[index] = Some(*packet_number);
        }

        sort_packet_numbers(&mut numbers, self.len);
        ranges_from_sorted_numbers(numbers, self.len)
    }
}

fn sort_packet_numbers(numbers: &mut [Option<PacketNumber>; MAX_ACK_TRACKED_PACKETS], len: usize) {
    let mut i = 1;

    while i < len {
        let Some(value) = numbers[i] else {
            return;
        };
        let mut j = i;

        while j > 0 {
            let Some(previous) = numbers[j - 1] else {
                break;
            };

            if previous.get() <= value.get() {
                break;
            }

            numbers[j] = numbers[j - 1];
            j -= 1;
        }

        numbers[j] = Some(value);
        i += 1;
    }
}

fn ranges_from_sorted_numbers(
    numbers: [Option<PacketNumber>; MAX_ACK_TRACKED_PACKETS],
    len: usize,
) -> Ack {
    let empty = AckRange::new(PacketNumber::ZERO, PacketNumber::ZERO);
    let mut ranges = [empty; MAX_ACK_RANGES];
    let mut range_count = 0;
    let mut index = len;

    while index > 0 && range_count < MAX_ACK_RANGES {
        index -= 1;
        let Some(end) = numbers[index] else {
            break;
        };
        let mut start = end;

        while index > 0 {
            let Some(previous) = numbers[index - 1] else {
                break;
            };

            if previous.get().wrapping_add(1) != start.get() {
                break;
            }

            start = previous;
            index -= 1;
        }

        ranges[range_count] = AckRange::new(start, end);
        range_count += 1;
    }

    Ack::from_ranges(ranges, range_count as u8)
}

#[cfg(test)]
mod tests {
    use super::AckRanges;
    use crate::core::PacketNumber;
    use crate::engine::config::MAX_ACK_TRACKED_PACKETS;

    #[test]
    fn ack_ranges_evict_least_recent_packet_when_full() {
        let mut ranges = AckRanges::new();

        for packet_number in 0..MAX_ACK_TRACKED_PACKETS as u32 {
            ranges.observe(PacketNumber::new(packet_number));
        }

        ranges.observe(PacketNumber::new(MAX_ACK_TRACKED_PACKETS as u32));

        let ack = ranges.ack();

        assert!(!ack.acknowledges(PacketNumber::new(0)));
        assert!(ack.acknowledges(PacketNumber::new(1)));
        assert!(ack.acknowledges(PacketNumber::new(MAX_ACK_TRACKED_PACKETS as u32)));
    }

    #[test]
    fn ack_ranges_keeps_recently_observed_retransmit_when_full() {
        let mut ranges = AckRanges::new();

        for packet_number in 10..10 + MAX_ACK_TRACKED_PACKETS as u32 {
            ranges.observe(PacketNumber::new(packet_number));
        }

        let retransmit = PacketNumber::new(3);
        ranges.observe(retransmit);

        let ack = ranges.ack();

        assert!(ack.acknowledges(retransmit));
        assert!(!ack.acknowledges(PacketNumber::new(10)));
    }

    #[test]
    fn ack_ranges_prefer_newest_ranges_when_range_capacity_is_full() {
        let mut ranges = AckRanges::new();

        for packet_number in [0, 2, 4, 6, 8] {
            ranges.observe(PacketNumber::new(packet_number));
        }

        let ack = ranges.ack();

        assert_eq!(ack.range_count as usize, crate::core::MAX_ACK_RANGES);
        assert!(ack.acknowledges(PacketNumber::new(8)));
        assert!(ack.acknowledges(PacketNumber::new(6)));
        assert!(ack.acknowledges(PacketNumber::new(4)));
        assert!(ack.acknowledges(PacketNumber::new(2)));
        assert!(!ack.acknowledges(PacketNumber::new(0)));
    }
}
