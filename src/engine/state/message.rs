//! Complete message delivery queue.

use crate::engine::{MessageEvent, config::MAX_MESSAGE_EVENTS};

/// Complete application messages waiting for [`crate::engine::Engine::poll`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MessageState {
    messages: [Option<MessageEvent>; MAX_MESSAGE_EVENTS],
    head: usize,
    len: usize,
}

impl MessageState {
    pub(crate) const fn new() -> Self {
        Self {
            messages: [None; MAX_MESSAGE_EVENTS],
            head: 0,
            len: 0,
        }
    }

    pub(crate) fn push(&mut self, message: MessageEvent) {
        if self.len == MAX_MESSAGE_EVENTS {
            self.messages[self.head] = Some(message);
            self.head = (self.head + 1) % MAX_MESSAGE_EVENTS;
            return;
        }

        let index = (self.head + self.len) % MAX_MESSAGE_EVENTS;
        self.messages[index] = Some(message);
        self.len += 1;
    }

    pub(crate) fn pop(&mut self) -> Option<MessageEvent> {
        if self.len == 0 {
            return None;
        }

        let message = self.messages[self.head].take();
        self.head = (self.head + 1) % MAX_MESSAGE_EVENTS;
        self.len -= 1;

        message
    }

    #[cfg(feature = "tracing")]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }
}
