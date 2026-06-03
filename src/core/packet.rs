//! Packet-level protocol primitives.

pub mod header;
pub mod number;
pub mod payload;
pub mod ty;

pub use header::{Flags, PacketHeader};
pub use number::PacketNumber;
pub use payload::PacketPayload;
pub use ty::PacketType;

/// Borrowed protocol packet.
///
/// A packet is the protocol transport unit. Its payload contains encoded MSRT
/// protocol frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Packet<'a> {
    /// Packet metadata.
    pub header: PacketHeader,
    /// Borrowed packet payload containing encoded protocol frames.
    pub payload: PacketPayload<'a>,
}

impl<'a> Packet<'a> {
    /// Creates a borrowed packet.
    #[must_use]
    pub const fn new(header: PacketHeader, payload: &'a [u8]) -> Self {
        Self {
            header,
            payload: PacketPayload::new(payload),
        }
    }

    /// Creates a borrowed packet from an existing payload view.
    #[must_use]
    pub const fn from_parts(header: PacketHeader, payload: PacketPayload<'a>) -> Self {
        Self { header, payload }
    }

    /// Returns the packet payload length in bytes.
    #[must_use]
    pub const fn payload_len(self) -> usize {
        self.payload.len()
    }

    /// Returns whether the packet payload is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.payload.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{Flags, Packet, PacketHeader, PacketNumber};
    use crate::core::{ChannelId, MessageFlags, MessageId};

    #[test]
    fn packet_payload_contains_encoded_frames() {
        let payload = [1, 2, 3];
        let header = PacketHeader::data(
            PacketNumber::new(9),
            Flags::ACK_ELICITING,
            ChannelId::DEFAULT,
            MessageId::new(7),
            3,
            0,
            MessageFlags::FIRST,
        );
        let packet = Packet::new(header, &payload);

        assert_eq!(packet.payload_len(), 3);
        assert!(!packet.is_empty());
        assert!(packet.header.is_ack_eliciting());
    }
}
