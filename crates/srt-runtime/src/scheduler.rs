//! Runtime scheduling boundaries.

use crate::time::Instant;

/// Runtime scheduling decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Schedule {
    /// No scheduled wakeup is required.
    Idle,
    /// Runtime should be ticked again at the given instant.
    WakeAt(Instant),
}

impl Schedule {
    /// Returns whether this schedule is idle.
    #[must_use]
    pub const fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}

/// Schedules future runtime progress.
pub trait Scheduler {
    /// Requests a future runtime wakeup.
    fn schedule(&mut self, schedule: Schedule);

    /// Returns the next scheduled runtime wakeup.
    fn next_wake(&self) -> Schedule;
}
