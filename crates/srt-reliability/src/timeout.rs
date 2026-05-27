//! Timeout boundary.

use srt_core::Seq;

/// Handles packet timeout events.
pub trait TimeoutPolicy {
    /// Records that `seq` timed out.
    fn on_timeout(&mut self, seq: Seq);
}
