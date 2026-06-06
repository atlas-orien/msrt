//! Fixed-capacity scheduler queues.

use crate::core::{Error, ErrorKind, Result};
use crate::engine::config::MAX_EVENTS;

use super::event::{EngineOutput, WriteEvent, WritePriority};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EventQueue {
    events: [Option<EngineOutput>; MAX_EVENTS],
    head: usize,
    len: usize,
}

impl EventQueue {
    pub(super) const fn new() -> Self {
        Self {
            events: [None; MAX_EVENTS],
            head: 0,
            len: 0,
        }
    }

    pub(super) fn push(&mut self, event: EngineOutput) -> Result<()> {
        if self.len == MAX_EVENTS {
            return Err(Error::new(ErrorKind::Engine));
        }

        let index = (self.head + self.len) % MAX_EVENTS;
        self.events[index] = Some(event);
        self.len += 1;

        Ok(())
    }

    pub(super) fn pop(&mut self) -> Option<EngineOutput> {
        if self.len == 0 {
            return None;
        }

        let index = self.head;
        let event = self.events[index].take();

        self.head = (self.head + 1) % MAX_EVENTS;
        self.len -= 1;

        event
    }

    pub(super) const fn physical_index(&self, offset: usize) -> usize {
        (self.head + offset) % MAX_EVENTS
    }

    pub(super) fn replace_redundant_write(&mut self, event: EngineOutput) -> bool {
        let EngineOutput::Write(write) = event else {
            return false;
        };

        let mut offset = 0;
        while offset < self.len {
            let index = self.physical_index(offset);
            let Some(EngineOutput::Write(current)) = self.events[index] else {
                offset += 1;
                continue;
            };

            if is_redundant_write(current, write) {
                self.events[index] = Some(EngineOutput::Write(write));
                return true;
            }

            offset += 1;
        }

        false
    }

    pub(super) const fn len(&self) -> usize {
        self.len
    }

    #[cfg(feature = "tracing")]
    pub(super) fn log_snapshot(&self, now_ms: u64, name: &str) {
        let mut offset = 0;
        while offset < self.len {
            let index = self.physical_index(offset);
            if let Some(event) = self.events[index].as_ref() {
                super::log::log_event(now_ms, name, offset, event);
            }
            offset += 1;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QueueKind {
    Control,
    Retransmit,
    NewData,
    Local,
}

pub(super) const fn queue_for_event(event: &EngineOutput) -> QueueKind {
    match event {
        EngineOutput::Write(write) => match write.priority {
            WritePriority::Control => QueueKind::Control,
            WritePriority::Retransmit => QueueKind::Retransmit,
            WritePriority::NewData => QueueKind::NewData,
        },
        EngineOutput::SendFailed(_) => QueueKind::Local,
    }
}

fn is_redundant_write(current: WriteEvent, incoming: WriteEvent) -> bool {
    current.key == incoming.key
}
