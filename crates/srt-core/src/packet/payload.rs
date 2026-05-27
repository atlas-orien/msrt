//! Packet payload view.

/// Borrowed packet payload bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Payload<'a> {
    bytes: &'a [u8],
}

impl<'a> Payload<'a> {
    /// Empty payload.
    pub const EMPTY: Self = Self { bytes: &[] };

    /// Creates a payload view from borrowed bytes.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Returns the borrowed payload bytes.
    #[must_use]
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Returns the payload length in bytes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the payload is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }
}
