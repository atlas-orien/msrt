//! Message fragment identifiers and ranges.

use crate::core::{ChannelId, Error, ErrorKind, MessageId, PacketHeader, Result};

/// Key that identifies one message on one channel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageKey {
    /// Logical channel that owns the message.
    pub channel_id: ChannelId,
    /// Message identifier scoped to the channel.
    pub message_id: MessageId,
}

impl MessageKey {
    /// Creates a message key.
    #[must_use]
    pub const fn new(channel_id: ChannelId, message_id: MessageId) -> Self {
        Self {
            channel_id,
            message_id,
        }
    }
}

/// Byte range covered by one message fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FragmentRange {
    /// Fragment start offset inside the complete message.
    pub offset: u32,
    /// Fragment length in bytes.
    pub len: u32,
}

impl FragmentRange {
    /// Creates a fragment range.
    #[must_use]
    pub const fn new(offset: u32, len: u32) -> Self {
        Self { offset, len }
    }

    /// Returns the exclusive end offset using saturating arithmetic.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.offset.saturating_add(self.len)
    }

    /// Returns whether this range is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Returns whether this range is fully inside a message of `message_len`.
    #[must_use]
    pub const fn fits_in(self, message_len: u32) -> bool {
        self.offset <= message_len && self.end() <= message_len
    }
}

/// Reliability-facing view of a message fragment carried by a DATA packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageFragment {
    /// Message identity.
    pub key: MessageKey,
    /// Complete message length in bytes.
    pub message_len: u32,
    /// Range covered by this fragment.
    pub range: FragmentRange,
}

impl MessageFragment {
    /// Creates a message fragment descriptor.
    #[must_use]
    pub const fn new(key: MessageKey, message_len: u32, range: FragmentRange) -> Self {
        Self {
            key,
            message_len,
            range,
        }
    }

    /// Builds a message fragment descriptor from a packet header and payload length.
    pub fn try_from_packet_header(header: PacketHeader, payload_len: usize) -> Result<Self> {
        let len = u32::try_from(payload_len).map_err(|_| Error::new(ErrorKind::Reliability))?;
        let message_len =
            u32::try_from(header.message_len()).map_err(|_| Error::new(ErrorKind::Reliability))?;
        let fragment_offset = u32::try_from(header.fragment_offset())
            .map_err(|_| Error::new(ErrorKind::Reliability))?;
        let range = FragmentRange::new(fragment_offset, len);

        if !range.fits_in(message_len) {
            return Err(Error::new(ErrorKind::Reliability));
        }

        Ok(Self::new(
            MessageKey::new(header.channel_id(), header.message_id()),
            message_len,
            range,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{ChannelId, Flags, MessageId, PacketHeader, PacketIndex};

    use super::{FragmentRange, MessageFragment, MessageKey};

    #[test]
    fn fragment_range_must_fit_message_len() {
        assert!(FragmentRange::new(2, 3).fits_in(5));
        assert!(!FragmentRange::new(3, 3).fits_in(5));
    }

    #[test]
    fn packet_header_maps_to_message_fragment() {
        let header = PacketHeader::data(
            PacketIndex::new(3),
            Flags::ACK_ELICITING,
            ChannelId::new(7),
            MessageId::new(9),
            8,
            2,
        );

        let fragment = MessageFragment::try_from_packet_header(header, 3).unwrap();

        assert_eq!(
            fragment.key,
            MessageKey::new(ChannelId::new(7), MessageId::new(9))
        );
        assert_eq!(fragment.range, FragmentRange::new(2, 3));
    }
}
