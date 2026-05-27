//! Packet-level protocol primitives.

pub mod header;
pub mod kind;
pub mod payload;

pub use header::{Flags, PacketHeader, Seq, StreamId};
pub use kind::PacketKind;
pub use payload::Payload;

/// Borrowed protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Packet<'a> {
    /// Packet metadata.
    pub header: PacketHeader,
    /// Borrowed packet payload.
    pub payload: Payload<'a>,
}

impl<'a> Packet<'a> {
    /// Creates a borrowed packet.
    #[must_use]
    pub const fn new(header: PacketHeader, payload: &'a [u8]) -> Self {
        Self {
            header,
            payload: Payload::new(payload),
        }
    }

    /// Creates a borrowed packet from an existing payload view.
    #[must_use]
    pub const fn from_parts(header: PacketHeader, payload: Payload<'a>) -> Self {
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
    use super::{Packet, PacketHeader, PacketKind};
    use crate::{Flags, Seq, StreamId};

    #[test]
    fn packet_borrows_payload_without_allocation() {
        let payload = [1, 2, 3];
        let header = PacketHeader::new(
            PacketKind::Data,
            StreamId::new(7),
            Seq::new(9),
            Flags::ACK_ELICITING,
        );
        let packet = Packet::new(header, &payload);

        assert_eq!(packet.payload_len(), 3);
        assert!(!packet.is_empty());
        assert!(packet.header.is_ack_eliciting());
    }
}
