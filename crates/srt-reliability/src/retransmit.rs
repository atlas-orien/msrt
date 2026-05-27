//! Retransmission boundary.

use srt_core::Seq;

/// Chooses packets eligible for retransmission.
pub trait RetransmitPolicy {
    /// Returns whether `seq` should be retransmitted.
    fn should_retransmit(&self, seq: Seq) -> bool;
}
