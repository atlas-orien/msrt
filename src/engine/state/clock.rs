//! Monotonic protocol time state.

/// Last engine time observed through `poll`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClockState {
    now_ms: u64,
}

impl ClockState {
    pub(crate) const fn new() -> Self {
        Self { now_ms: 0 }
    }

    pub(crate) const fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub(crate) fn update(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }
}
