//! Sliding window boundary.

use srt_core::Seq;

/// Maintains send or receive window state.
pub trait SlidingWindow {
    /// Returns whether `seq` is currently inside the window.
    fn contains(&self, seq: Seq) -> bool;
}
