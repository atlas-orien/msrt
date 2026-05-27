//! Acknowledgement boundary.

use srt_core::Seq;

/// Tracks acknowledgement state.
pub trait AckTracker {
    /// Records that `seq` was acknowledged.
    fn on_ack(&mut self, seq: Seq);
}
