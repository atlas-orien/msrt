//! Engine output event queue.

use crate::core::{Error, ErrorKind, Result};

use crate::engine::{EngineOutput, MAX_EVENTS};

/// Fixed-capacity event queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EventQueue {
    events: [Option<EngineOutput>; MAX_EVENTS],
    head: usize,
    len: usize,
}

impl EventQueue {
    pub(crate) const fn new() -> Self {
        Self {
            events: [None; MAX_EVENTS],
            head: 0,
            len: 0,
        }
    }

    pub(crate) fn push(&mut self, event: EngineOutput) -> Result<()> {
        if self.len == MAX_EVENTS {
            return Err(Error::new(ErrorKind::Engine));
        }

        let index = (self.head + self.len) % MAX_EVENTS;
        self.events[index] = Some(event);
        self.len += 1;

        Ok(())
    }

    pub(crate) fn pop(&mut self) -> Option<EngineOutput> {
        if self.len == 0 {
            return None;
        }

        let event = self.events[self.head].take();
        self.head = (self.head + 1) % MAX_EVENTS;
        self.len -= 1;

        event
    }
}
