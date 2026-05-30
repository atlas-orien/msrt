//! Deduplication boundary.

use srt_core::PacketNumber;

/// Detects duplicate packets.
pub trait Dedup {
    /// Returns whether `seq` has already been observed.
    fn is_duplicate(&self, packet_number: PacketNumber) -> bool;
}
