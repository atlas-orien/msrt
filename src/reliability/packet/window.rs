//! Sliding window boundary.

use crate::core::PacketKey;

/// Decision returned by a send or receive window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowDecision {
    /// The packet key is inside the current window.
    Accept,
    /// The packet key is outside the window for an unspecified reason.
    Reject,
    /// The packet key is older than the window.
    TooOld,
    /// The packet key is newer than the window currently allows.
    TooNew,
}

/// Maintains send or receive packet window state.
pub trait SlidingWindow {
    /// Returns whether a packet key is inside the current window.
    fn contains(&self, key: PacketKey) -> bool;

    /// Checks a packet key against the current window.
    fn check(&self, key: PacketKey) -> WindowDecision;

    /// Advances the window base to a new packet key.
    fn advance_to(&mut self, key: PacketKey);
}
