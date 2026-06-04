//! Packet and message identifier allocation.

use crate::core::{MessageId, PacketNumber};

/// Monotonic packet and message number allocator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NumberState {
    next_packet_number: PacketNumber,
    next_message_id: MessageId,
}

impl NumberState {
    pub(crate) const fn new(
        initial_packet_number: PacketNumber,
        initial_message_id: MessageId,
    ) -> Self {
        Self {
            next_packet_number: initial_packet_number,
            next_message_id: initial_message_id,
        }
    }

    pub(crate) fn alloc_packet_number(&mut self) -> PacketNumber {
        let packet_number = self.next_packet_number;
        self.next_packet_number = self.next_packet_number.next();
        packet_number
    }

    pub(crate) fn alloc_message_id(&mut self) -> MessageId {
        let message_id = self.next_message_id;
        self.next_message_id = MessageId::new(self.next_message_id.get().wrapping_add(1));
        message_id
    }
}
