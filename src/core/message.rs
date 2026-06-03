//! Message identity primitives.

/// Encoded channel identifier length in bytes.
pub(crate) const CHANNEL_ID_LEN: usize = core::mem::size_of::<u8>();
/// Encoded message identifier length in bytes.
pub(crate) const MESSAGE_ID_LEN: usize = core::mem::size_of::<u32>();
/// Encoded complete message length field size in bytes.
pub(crate) const MESSAGE_LEN_LEN: usize = core::mem::size_of::<u16>();
/// Encoded fragment offset field size in bytes.
pub(crate) const FRAGMENT_OFFSET_LEN: usize = core::mem::size_of::<u16>();
/// Encoded fragment flags field size in bytes.
pub(crate) const FRAGMENT_FLAGS_LEN: usize = core::mem::size_of::<u8>();
/// A logical channel identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChannelId(pub u8);

impl ChannelId {
    /// Default application channel.
    pub const DEFAULT: Self = Self(0);
    /// Log channel reserved for diagnostic output.
    pub const LOG: Self = Self(1);
    /// Liveness channel reserved for automatic Ping/Pong packets.
    pub const LIVENESS: Self = Self(2);
    /// First channel available for application-defined routing.
    pub const FIRST_APPLICATION_DEFINED: Self = Self(16);

    /// Creates a channel identifier from its raw value.
    #[must_use]
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    /// Returns the raw channel identifier value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// Returns whether this is the default application channel.
    #[must_use]
    pub const fn is_default(self) -> bool {
        self.0 == Self::DEFAULT.0
    }

    /// Returns whether this is the log channel.
    #[must_use]
    pub const fn is_log(self) -> bool {
        self.0 == Self::LOG.0
    }

    /// Returns whether this is the liveness channel.
    #[must_use]
    pub const fn is_liveness(self) -> bool {
        self.0 == Self::LIVENESS.0
    }

    /// Returns whether this channel is application-defined.
    #[must_use]
    pub const fn is_application_defined(self) -> bool {
        self.0 >= Self::FIRST_APPLICATION_DEFINED.0
    }
}

/// Message identifier scoped to a channel.
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

/// Message fragment flags carried in packet headers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MessageFlags(pub u8);

impl MessageFlags {
    /// Empty flag set.
    pub const EMPTY: Self = Self(0);

    /// First fragment of a message.
    pub const FIRST: Self = Self(1 << 0);

    /// Last fragment of a message.
    pub const LAST: Self = Self(1 << 1);

    /// Creates flags from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Returns the raw flag bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Returns whether all bits from `other` are set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns a new flag set with `other` included.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelId, MessageFlags, MessageId};

    #[test]
    fn message_identity_primitives_expose_raw_values() {
        assert_eq!(ChannelId::new(9).get(), 9);
        assert_eq!(MessageId::new(7).get(), 7);
        assert!(MessageFlags::FIRST.contains(MessageFlags::FIRST));
        assert!(!MessageFlags::FIRST.contains(MessageFlags::LAST));
    }
}
