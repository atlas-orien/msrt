//! Message reassembly table.

use crate::core::{Error, ErrorKind, MessageId, PacketType, Result};

use crate::engine::{
    MessageEvent,
    codec::packet::DecodedFragment,
    config::{MAX_MESSAGE_BYTES, MAX_REASSEMBLY_MESSAGES},
};

/// Message reassembly state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReassemblyState {
    slots: [ReassemblySlot; MAX_REASSEMBLY_MESSAGES],
}

impl ReassemblyState {
    pub(crate) const fn new() -> Self {
        Self {
            slots: [ReassemblySlot::new(); MAX_REASSEMBLY_MESSAGES],
        }
    }

    pub(crate) fn observe(
        &mut self,
        fragment: DecodedFragment<'_>,
        now_ms: u64,
    ) -> Result<Option<MessageEvent>> {
        if fragment.message_len > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        let key = MessageKey {
            packet_type: fragment.packet_type,
            message_id: fragment.message_id,
        };
        let index = match self.find_slot(key) {
            Some(index) => index,
            None => self.allocate_slot(key, fragment, now_ms)?,
        };

        self.slots[index].observe(fragment, now_ms)
    }

    pub(crate) fn expire(&mut self, now_ms: u64, timeout_ms: u64) {
        for slot in &mut self.slots {
            if slot.active && now_ms.saturating_sub(slot.updated_at_ms) >= timeout_ms {
                *slot = ReassemblySlot::new();
            }
        }
    }

    fn find_slot(&self, key: MessageKey) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.active && slot.key == key)
    }

    fn allocate_slot(
        &mut self,
        key: MessageKey,
        fragment: DecodedFragment<'_>,
        now_ms: u64,
    ) -> Result<usize> {
        let index = self
            .slots
            .iter()
            .position(|slot| !slot.active)
            .unwrap_or_else(|| self.oldest_slot());

        self.slots[index] = ReassemblySlot::start(key, fragment.message_len, now_ms);

        Ok(index)
    }

    fn oldest_slot(&self) -> usize {
        let mut oldest_index = 0;
        let mut oldest_updated_at = u64::MAX;

        for (index, slot) in self.slots.iter().enumerate() {
            if slot.updated_at_ms < oldest_updated_at {
                oldest_index = index;
                oldest_updated_at = slot.updated_at_ms;
            }
        }

        oldest_index
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReassemblySlot {
    active: bool,
    key: MessageKey,
    expected_len: usize,
    updated_at_ms: u64,
    last_seen: bool,
    received: [bool; MAX_MESSAGE_BYTES],
    bytes: [u8; MAX_MESSAGE_BYTES],
}

impl ReassemblySlot {
    const fn new() -> Self {
        Self {
            active: false,
            key: MessageKey::ZERO,
            expected_len: 0,
            updated_at_ms: 0,
            last_seen: false,
            received: [false; MAX_MESSAGE_BYTES],
            bytes: [0; MAX_MESSAGE_BYTES],
        }
    }

    const fn start(key: MessageKey, expected_len: usize, now_ms: u64) -> Self {
        Self {
            active: true,
            key,
            expected_len,
            updated_at_ms: now_ms,
            last_seen: false,
            received: [false; MAX_MESSAGE_BYTES],
            bytes: [0; MAX_MESSAGE_BYTES],
        }
    }

    fn observe(
        &mut self,
        fragment: DecodedFragment<'_>,
        now_ms: u64,
    ) -> Result<Option<MessageEvent>> {
        if self.expected_len != fragment.message_len {
            return Err(Error::new(ErrorKind::Engine));
        }

        let end = fragment.fragment_offset + fragment.bytes.len();

        if end > self.expected_len || end > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        self.bytes[fragment.fragment_offset..end].copy_from_slice(fragment.bytes);
        self.updated_at_ms = now_ms;

        for received in &mut self.received[fragment.fragment_offset..end] {
            *received = true;
        }

        if end == self.expected_len {
            self.last_seen = true;
        }

        if self.is_complete() {
            let message = MessageEvent {
                packet_type: self.key.packet_type,
                message_id: self.key.message_id,
                bytes: self.bytes,
                len: self.expected_len,
            };
            *self = Self::new();

            Ok(Some(message))
        } else {
            Ok(None)
        }
    }

    fn is_complete(&self) -> bool {
        self.last_seen
            && self.received[..self.expected_len]
                .iter()
                .all(|received| *received)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MessageKey {
    packet_type: PacketType,
    message_id: MessageId,
}

impl MessageKey {
    const ZERO: Self = Self {
        packet_type: PacketType::Data,
        message_id: MessageId::ZERO,
    };
}
