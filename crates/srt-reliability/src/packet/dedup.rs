//! Duplicate packet detection boundary.

use srt_core::PacketNumber;

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
