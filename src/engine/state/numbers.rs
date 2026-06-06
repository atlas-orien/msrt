//! Packet and message identifier allocation.

use crate::core::MessageId;

const MESSAGE_ID_MULTIPLIER: u32 = 1_664_525;
const MESSAGE_ID_INCREMENT: u32 = 1_013_904_223;

/// Pseudo-random message identifier allocator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NumberState {
    message_id_state: u32,
}

impl NumberState {
    pub(crate) const fn new(initial_message_id: MessageId) -> Self {
        Self {
            message_id_state: initial_message_id.get(),
        }
    }

    pub(crate) fn alloc_message_id(&mut self) -> MessageId {
        self.message_id_state = self
            .message_id_state
            .wrapping_mul(MESSAGE_ID_MULTIPLIER)
            .wrapping_add(MESSAGE_ID_INCREMENT);
        MessageId::new(self.message_id_state)
    }
}

#[cfg(test)]
mod tests {
    use super::NumberState;
    use crate::core::MessageId;

    #[test]
    fn message_ids_are_pseudo_random_u32_values() {
        let mut numbers = NumberState::new(MessageId::ZERO);
        let first = numbers.alloc_message_id();
        let second = numbers.alloc_message_id();
        let third = numbers.alloc_message_id();

        assert_eq!(first.get(), 1_013_904_223);
        assert_ne!(second.get(), first.get().wrapping_add(1));
        assert_ne!(third.get(), second.get().wrapping_add(1));
    }

    #[test]
    fn message_id_seed_changes_sequence() {
        let mut first = NumberState::new(MessageId::new(1));
        let mut second = NumberState::new(MessageId::new(2));

        assert_ne!(first.alloc_message_id(), second.alloc_message_id());
    }
}
