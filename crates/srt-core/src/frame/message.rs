//! MESSAGE frame primitives.

/// A logical channel identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChannelId(pub u8);

impl ChannelId {
    /// Default application channel.
    pub const DEFAULT: Self = Self(0);
    /// Log channel reserved for diagnostic output.
    pub const LOG: Self = Self(1);
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

/// MESSAGE frame flags.
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

/// Borrowed message fragment bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageData<'a> {
    bytes: &'a [u8],
}

impl<'a> MessageData<'a> {
    /// Empty message data.
    pub const EMPTY: Self = Self { bytes: &[] };

    /// Creates message data from borrowed bytes.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Returns the borrowed bytes.
    #[must_use]
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Returns data length.
    #[must_use]
    pub const fn len(self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the data is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }
}

/// MESSAGE frame carrying one message fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageFrame<'a> {
    /// Logical channel.
    pub channel_id: ChannelId,
    /// Message identifier scoped to the channel.
    pub message_id: MessageId,
    /// Complete message length in bytes.
    pub message_len: u32,
    /// Fragment byte offset inside the complete message.
    pub fragment_offset: u32,
    /// MESSAGE frame flags.
    pub flags: MessageFlags,
    /// Fragment bytes.
    pub data: MessageData<'a>,
}

impl<'a> MessageFrame<'a> {
    /// Creates a MESSAGE frame.
    #[must_use]
    pub const fn new(
        channel_id: ChannelId,
        message_id: MessageId,
        message_len: u32,
        fragment_offset: u32,
        flags: MessageFlags,
        data: &'a [u8],
    ) -> Self {
        Self {
            channel_id,
            message_id,
            message_len,
            fragment_offset,
            flags,
            data: MessageData::new(data),
        }
    }

    /// Returns whether this is the first fragment.
    #[must_use]
    pub const fn is_first(self) -> bool {
        self.flags.contains(MessageFlags::FIRST)
    }

    /// Returns whether this is the last fragment.
    #[must_use]
    pub const fn is_last(self) -> bool {
        self.flags.contains(MessageFlags::LAST)
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelId, MessageFlags, MessageFrame, MessageId};

    #[test]
    fn message_frame_carries_message_fragment() {
        let data = [1, 2, 3];
        let frame = MessageFrame::new(
            ChannelId::new(9),
            MessageId::new(7),
            10,
            0,
            MessageFlags::FIRST,
            &data,
        );

        assert_eq!(frame.channel_id.get(), 9);
        assert_eq!(frame.message_id.get(), 7);
        assert_eq!(frame.data.len(), 3);
        assert!(frame.is_first());
        assert!(!frame.is_last());
    }
}
