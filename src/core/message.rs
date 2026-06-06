//! Message identity primitives.

/// Encoded message identifier length in bytes.
pub(crate) const MESSAGE_ID_LEN: usize = core::mem::size_of::<u32>();
/// Encoded complete message length field size in bytes.
pub(crate) const MESSAGE_LEN_LEN: usize = core::mem::size_of::<u16>();
/// Encoded fragment offset field size in bytes.
pub(crate) const FRAGMENT_OFFSET_LEN: usize = core::mem::size_of::<u16>();

/// Message identifier scoped to one engine session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId(pub u32);

impl MessageId {
    /// First message identifier on a channel.
    pub const ZERO: Self = Self(0);

    /// Creates a message identifier from its raw value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw message identifier value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::MessageId;

    #[test]
    fn message_identity_primitives_expose_raw_values() {
        assert_eq!(MessageId::new(7).get(), 7);
    }
}
