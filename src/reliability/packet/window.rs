//! Sliding window boundary.

use crate::core::PacketNumber;

/// Decision returned by a send or receive window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowDecision {
    /// The packet number is inside the current window.
    Accept,
    /// The packet number is outside the window for an unspecified reason.
    Reject,
    /// The packet number is older than the window.
    TooOld,
    /// The packet number is newer than the window currently allows.
    TooNew,
}

/// Maintains send or receive packet window state.
pub trait SlidingWindow {
    /// Returns whether a packet number is inside the current window.
    fn contains(&self, packet_number: PacketNumber) -> bool;

    /// Checks a packet number against the current window.
    fn check(&self, packet_number: PacketNumber) -> WindowDecision;

    /// Advances the window base to a new packet number.
    fn advance_to(&mut self, packet_number: PacketNumber);
}
