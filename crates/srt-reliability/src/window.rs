//! Sliding window boundary.

use srt_core::PacketNumber;

/// Maintains send or receive window state.
pub trait SlidingWindow {
    /// Returns whether `seq` is currently inside the window.
    fn contains(&self, packet_number: PacketNumber) -> bool;
}
