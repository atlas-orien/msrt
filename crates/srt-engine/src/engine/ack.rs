//! ACK range collection.

use srt_core::{AckFrame, AckRange, MAX_ACK_RANGES, PacketNumber};

use crate::MAX_ACK_TRACKED_PACKETS;

/// Fixed-capacity observed packet set used to build ACK ranges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AckRanges {
    packets: [Option<PacketNumber>; MAX_ACK_TRACKED_PACKETS],
}

impl AckRanges {
    pub(crate) const fn new() -> Self {
        Self {
            packets: [None; MAX_ACK_TRACKED_PACKETS],
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

        if let Some(slot) = self.packets.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(packet_number);
        }
    }

    pub(crate) fn frame(&self) -> AckFrame {
        let mut numbers = [None; MAX_ACK_TRACKED_PACKETS];
        let mut len = 0;

        for packet_number in self.packets.iter().flatten() {
            numbers[len] = Some(*packet_number);
            len += 1;
        }

        sort_packet_numbers(&mut numbers, len);
        ranges_from_sorted_numbers(numbers, len)
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
) -> AckFrame {
    let empty = AckRange::new(PacketNumber::ZERO, PacketNumber::ZERO);
    let mut ranges = [empty; MAX_ACK_RANGES];
    let mut range_count = 0;
    let mut index = 0;

    while index < len && range_count < MAX_ACK_RANGES {
        let Some(start) = numbers[index] else {
            break;
        };
        let mut end = start;
        index += 1;

        while index < len {
            let Some(next) = numbers[index] else {
                break;
            };

            if next.get() != end.get().wrapping_add(1) {
                break;
            }

            end = next;
            index += 1;
        }

        ranges[range_count] = AckRange::new(start, end);
        range_count += 1;
    }

    AckFrame::from_ranges(ranges, range_count as u8)
}
