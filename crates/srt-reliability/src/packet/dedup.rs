//! Duplicate packet detection boundary.

use srt_core::PacketNumber;
use srt_core::{Error, ErrorKind, Result};

/// Decision returned after observing a packet number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DedupDecision {
    /// The packet has not been observed before and may be processed.
    Accept,
    /// The packet is a duplicate and should not be processed again.
    Duplicate,
}

/// Detects duplicate packet numbers.
pub trait Dedup {
    /// Observes a packet number and returns whether it should be processed.
    fn observe(&mut self, packet_number: PacketNumber) -> DedupDecision;

    /// Returns whether a packet number is already known as observed.
    fn is_duplicate(&self, packet_number: PacketNumber) -> bool;
}

/// Fixed-capacity duplicate packet detector.
///
/// This detector is intentionally small and allocation-free. It remembers the
/// most recent packet numbers accepted by the engine and treats another packet
/// with the same number as a duplicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketDedup<const N: usize> {
    packets: [Option<PacketNumber>; N],
    next: usize,
    len: usize,
}

impl<const N: usize> PacketDedup<N> {
    /// Creates an empty detector.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            packets: [None; N],
            next: 0,
            len: 0,
        }
    }

    /// Records a packet number or reports that it has already been observed.
    pub fn observe_packet(&mut self, packet_number: PacketNumber) -> Result<DedupDecision> {
        if N == 0 {
            return Err(Error::new(ErrorKind::Reliability));
        }

        if self.is_duplicate(packet_number) {
            return Ok(DedupDecision::Duplicate);
        }

        self.packets[self.next] = Some(packet_number);
        self.next = (self.next + 1) % N;
        self.len = core::cmp::min(self.len + 1, N);

        Ok(DedupDecision::Accept)
    }

    /// Returns how many packet numbers are currently remembered.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the detector is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<const N: usize> Default for PacketDedup<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Dedup for PacketDedup<N> {
    fn observe(&mut self, packet_number: PacketNumber) -> DedupDecision {
        self.observe_packet(packet_number)
            .unwrap_or(DedupDecision::Duplicate)
    }

    fn is_duplicate(&self, packet_number: PacketNumber) -> bool {
        self.packets
            .iter()
            .flatten()
            .any(|known| *known == packet_number)
    }
}

#[cfg(test)]
mod tests {
    use srt_core::PacketNumber;

    use super::{Dedup, DedupDecision, PacketDedup};

    #[test]
    fn packet_dedup_detects_duplicates() {
        let mut dedup = PacketDedup::<2>::new();

        assert_eq!(dedup.observe(PacketNumber::new(1)), DedupDecision::Accept);
        assert_eq!(
            dedup.observe(PacketNumber::new(1)),
            DedupDecision::Duplicate
        );
    }

    #[test]
    fn packet_dedup_forgets_oldest_packet_when_full() {
        let mut dedup = PacketDedup::<2>::new();

        assert_eq!(dedup.observe(PacketNumber::new(1)), DedupDecision::Accept);
        assert_eq!(dedup.observe(PacketNumber::new(2)), DedupDecision::Accept);
        assert_eq!(dedup.observe(PacketNumber::new(3)), DedupDecision::Accept);

        assert!(!dedup.is_duplicate(PacketNumber::new(1)));
        assert!(dedup.is_duplicate(PacketNumber::new(2)));
        assert!(dedup.is_duplicate(PacketNumber::new(3)));
    }
}
