//! Pending ACK packet-key queue.

use crate::core::PacketKey;
use crate::engine::config::MAX_PENDING_ACKS;

/// ACK state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AckState {
    keys: [Option<PacketKey>; MAX_PENDING_ACKS],
    head: usize,
    len: usize,
}

impl AckState {
    pub(crate) const fn new() -> Self {
        Self {
            keys: [None; MAX_PENDING_ACKS],
            head: 0,
            len: 0,
        }
    }

    pub(crate) fn observe(&mut self, key: PacketKey) -> bool {
        if self.len == MAX_PENDING_ACKS {
            return false;
        }

        let index = (self.head + self.len) % MAX_PENDING_ACKS;
        self.keys[index] = Some(key);
        self.len += 1;
        true
    }

    pub(crate) const fn is_pending(&self) -> bool {
        self.len != 0
    }

    #[cfg(feature = "tracing")]
    pub(crate) const fn pending_len(&self) -> usize {
        self.len
    }

    pub(crate) fn pop(&mut self) -> Option<PacketKey> {
        if self.len == 0 {
            return None;
        }

        let key = self.keys[self.head].take();
        self.head = (self.head + 1) % MAX_PENDING_ACKS;
        self.len -= 1;

        if self.len == 0 {
            self.head = 0;
        }

        key
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{MessageId, PacketIndex, PacketKey};

    use super::AckState;

    #[test]
    fn ack_state_keeps_duplicate_ack_keys() {
        let key = PacketKey::new(MessageId::new(7), PacketIndex::new(1));
        let mut state = AckState::new();

        assert!(state.observe(key));
        assert!(state.observe(key));

        assert_eq!(state.pop(), Some(key));
        assert_eq!(state.pop(), Some(key));
        assert_eq!(state.pop(), None);
    }
}
