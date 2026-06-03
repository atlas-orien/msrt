//! Engine output event queue.

use crate::core::{Error, ErrorKind, Result};

use crate::engine::{config::MAX_EVENTS, machine::EngineOutput};

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

        let offset = self.highest_priority_offset();
        let index = self.physical_index(offset);
        let event = self.events[index].take();

        let mut current = offset;
        while current + 1 < self.len {
            let to = self.physical_index(current);
            let from = self.physical_index(current + 1);
            self.events[to] = self.events[from].take();
            current += 1;
        }

        let tail = self.physical_index(self.len - 1);
        self.events[tail] = None;
        self.len -= 1;

        event
    }

    fn highest_priority_offset(&self) -> usize {
        let mut best_offset = 0;
        let mut best_priority =
            EventPriority::for_event(self.events[self.head].as_ref().expect("queue is not empty"));
        let mut offset = 1;

        while offset < self.len {
            let index = self.physical_index(offset);
            let Some(event) = self.events[index].as_ref() else {
                offset += 1;
                continue;
            };
            let priority = EventPriority::for_event(event);

            if priority < best_priority {
                best_offset = offset;
                best_priority = priority;
            }

            offset += 1;
        }

        best_offset
    }

    const fn physical_index(&self, offset: usize) -> usize {
        (self.head + offset) % MAX_EVENTS
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum EventPriority {
    Control,
    Retransmit,
    NewWrite,
    Local,
}

impl EventPriority {
    fn for_event(event: &EngineOutput) -> Self {
        match event {
            EngineOutput::Write(write) => match write.priority {
                crate::engine::machine::WritePriority::Control => Self::Control,
                crate::engine::machine::WritePriority::Retransmit => Self::Retransmit,
                crate::engine::machine::WritePriority::NewData => Self::NewWrite,
            },
            EngineOutput::SendFailed(_) => Self::Local,
            EngineOutput::Message(_) => Self::Local,
        }
    }
}
