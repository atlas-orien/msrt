//! Duplicate packet detection boundary.

use crate::core::{Error, ErrorKind, PacketKey, Result};

/// Decision returned after observing a packet key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DedupDecision {
    /// The packet has not been observed before and may be processed.
    Accept,
    /// The packet is a duplicate and should not be processed again.
    Duplicate,
}

/// Detects duplicate packet keys.
pub trait Dedup {
    /// Observes a packet key and returns whether it should be processed.
    fn observe(&mut self, key: PacketKey) -> DedupDecision;

    /// Returns whether a packet key is already known as observed.
    fn is_duplicate(&self, key: PacketKey) -> bool;
}

/// Fixed-capacity duplicate packet detector.
///
/// This detector is intentionally small and allocation-free. It remembers the
/// most recent packet keys accepted by the engine and treats another packet
/// with the same key as a duplicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketDedup<const N: usize> {
    packets: [Option<PacketKey>; N],
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

    /// Records a packet key or reports that it has already been observed.
    pub fn observe_packet(&mut self, key: PacketKey) -> Result<DedupDecision> {
        if N == 0 {
            return Err(Error::new(ErrorKind::Reliability));
        }

        if self.is_duplicate(key) {
            return Ok(DedupDecision::Duplicate);
        }

        self.packets[self.next] = Some(key);
        self.next = (self.next + 1) % N;
        self.len = core::cmp::min(self.len + 1, N);

        Ok(DedupDecision::Accept)
    }

    /// Returns how many packet keys are currently remembered.
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
    fn observe(&mut self, key: PacketKey) -> DedupDecision {
        self.observe_packet(key).unwrap_or(DedupDecision::Duplicate)
    }

    fn is_duplicate(&self, key: PacketKey) -> bool {
        self.packets.iter().flatten().any(|known| *known == key)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{MessageId, PacketIndex, PacketKey};

    use super::{Dedup, DedupDecision, PacketDedup};

    #[test]
    fn packet_dedup_detects_duplicates() {
        let mut dedup = PacketDedup::<2>::new();
        let key = PacketKey::new(MessageId::new(1), PacketIndex::ZERO);

        assert_eq!(dedup.observe(key), DedupDecision::Accept);
        assert_eq!(dedup.observe(key), DedupDecision::Duplicate);
    }

    #[test]
    fn packet_dedup_forgets_oldest_packet_when_full() {
        let mut dedup = PacketDedup::<2>::new();
        let first = PacketKey::new(MessageId::new(1), PacketIndex::ZERO);
        let second = PacketKey::new(MessageId::new(2), PacketIndex::ZERO);
        let third = PacketKey::new(MessageId::new(3), PacketIndex::ZERO);

        assert_eq!(dedup.observe(first), DedupDecision::Accept);
        assert_eq!(dedup.observe(second), DedupDecision::Accept);
        assert_eq!(dedup.observe(third), DedupDecision::Accept);

        assert!(!dedup.is_duplicate(first));
        assert!(dedup.is_duplicate(second));
        assert!(dedup.is_duplicate(third));
    }
}
