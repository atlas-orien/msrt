//! Deduplication boundary.

use srt_core::Seq;

/// Detects duplicate packets.
pub trait Dedup {
    /// Returns whether `seq` has already been observed.
    fn is_duplicate(&self, seq: Seq) -> bool;
}
