//! Message reassembly buffer.

use srt_core::{Error, ErrorKind, MessageId, Result};

use crate::{
    MAX_MESSAGE_BYTES, MessageEvent,
    engine::packet::DecodedFragment,
    layout::{FRAGMENT_FIRST, FRAGMENT_LAST},
};

/// Fixed-capacity MVP message reassembly buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReassemblyBuffer {
    active: bool,
    message_id: MessageId,
    expected_len: usize,
    last_seen: bool,
    received: [bool; MAX_MESSAGE_BYTES],
    bytes: [u8; MAX_MESSAGE_BYTES],
}

impl ReassemblyBuffer {
    pub(crate) const fn new() -> Self {
        Self {
            active: false,
            message_id: MessageId::ZERO,
            expected_len: 0,
            last_seen: false,
            received: [false; MAX_MESSAGE_BYTES],
            bytes: [0; MAX_MESSAGE_BYTES],
        }
    }

    pub(crate) fn observe(
        &mut self,
        fragment: DecodedFragment<'_>,
    ) -> Result<Option<MessageEvent>> {
        if fragment.message_len > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        if !self.active {
            if fragment.flags & FRAGMENT_FIRST == 0 {
                return Ok(None);
            }

            self.active = true;
            self.message_id = fragment.message_id;
            self.expected_len = fragment.message_len;
            self.last_seen = false;
            self.received = [false; MAX_MESSAGE_BYTES];
            self.bytes = [0; MAX_MESSAGE_BYTES];
        } else if self.message_id != fragment.message_id && fragment.flags & FRAGMENT_FIRST != 0 {
            self.active = true;
            self.message_id = fragment.message_id;
            self.expected_len = fragment.message_len;
            self.last_seen = false;
            self.received = [false; MAX_MESSAGE_BYTES];
            self.bytes = [0; MAX_MESSAGE_BYTES];
        }

        if self.message_id != fragment.message_id || self.expected_len != fragment.message_len {
            return Err(Error::new(ErrorKind::Engine));
        }

        let end = fragment.fragment_offset + fragment.bytes.len();

        if end > self.expected_len || end > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        self.bytes[fragment.fragment_offset..end].copy_from_slice(fragment.bytes);

        for received in &mut self.received[fragment.fragment_offset..end] {
            *received = true;
        }

        if fragment.flags & FRAGMENT_LAST != 0 {
            self.last_seen = true;
        }

        if self.last_seen
            && self.received[..self.expected_len]
                .iter()
                .all(|received| *received)
        {
            let message = MessageEvent {
                message_id: self.message_id,
                bytes: self.bytes,
                len: self.expected_len,
            };
            *self = Self::new();

            Ok(Some(message))
        } else {
            Ok(None)
        }
    }
}
