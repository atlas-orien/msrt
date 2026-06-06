//! Packet and message identifier allocation.

use crate::core::MessageId;

/// Monotonic message identifier allocator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NumberState {
    next_message_id: MessageId,
}

impl NumberState {
    pub(crate) const fn new(initial_message_id: MessageId) -> Self {
        Self {
            next_message_id: initial_message_id,
        }
    }

    pub(crate) fn alloc_message_id(&mut self) -> MessageId {
        let message_id = self.next_message_id;
        self.next_message_id = MessageId::new(self.next_message_id.get().wrapping_add(1));
        message_id
    }
}
