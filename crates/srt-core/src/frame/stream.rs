//! STREAM frame primitives.

/// A logical stream identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamId(pub u16);

impl StreamId {
    /// Reserved control stream.
    pub const CONTROL: Self = Self(0);

    /// Creates a stream identifier from its raw value.
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    /// Returns the raw stream identifier value.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// Returns whether this is the reserved control stream.
    #[must_use]
    pub const fn is_control(self) -> bool {
        self.0 == Self::CONTROL.0
    }
}

/// Message identifier scoped to a stream.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId(pub u32);

impl MessageId {
    /// First message identifier on a stream.
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

/// STREAM frame flags.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StreamFlags(pub u8);

impl StreamFlags {
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

/// Borrowed stream fragment bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamData<'a> {
    bytes: &'a [u8],
}

impl<'a> StreamData<'a> {
    /// Empty stream data.
    pub const EMPTY: Self = Self { bytes: &[] };

    /// Creates stream data from borrowed bytes.
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

/// STREAM frame carrying one message fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamFrame<'a> {
    /// Logical stream.
    pub stream_id: StreamId,
    /// Message identifier scoped to the stream.
    pub message_id: MessageId,
    /// Complete message length in bytes.
    pub message_len: u32,
    /// Fragment byte offset inside the complete message.
    pub fragment_offset: u32,
    /// STREAM frame flags.
    pub flags: StreamFlags,
    /// Fragment bytes.
    pub data: StreamData<'a>,
}

impl<'a> StreamFrame<'a> {
    /// Creates a STREAM frame.
    #[must_use]
    pub const fn new(
        stream_id: StreamId,
        message_id: MessageId,
        message_len: u32,
        fragment_offset: u32,
        flags: StreamFlags,
        data: &'a [u8],
    ) -> Self {
        Self {
            stream_id,
            message_id,
            message_len,
            fragment_offset,
            flags,
            data: StreamData::new(data),
        }
    }

    /// Returns whether this is the first fragment.
    #[must_use]
    pub const fn is_first(self) -> bool {
        self.flags.contains(StreamFlags::FIRST)
    }

    /// Returns whether this is the last fragment.
    #[must_use]
    pub const fn is_last(self) -> bool {
        self.flags.contains(StreamFlags::LAST)
    }
}

#[cfg(test)]
mod tests {
    use super::{MessageId, StreamFlags, StreamFrame, StreamId};

    #[test]
    fn stream_frame_carries_message_fragment() {
        let data = [1, 2, 3];
        let frame = StreamFrame::new(
            StreamId::new(9),
            MessageId::new(7),
            10,
            0,
            StreamFlags::FIRST,
            &data,
        );

        assert_eq!(frame.stream_id.get(), 9);
        assert_eq!(frame.message_id.get(), 7);
        assert_eq!(frame.data.len(), 3);
        assert!(frame.is_first());
        assert!(!frame.is_last());
    }
}
