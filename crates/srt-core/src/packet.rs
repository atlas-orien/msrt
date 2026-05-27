//! Packet-level protocol primitives.

use crate::{Flags, Seq, StreamId};

/// Coarse packet categories reserved by the core protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketKind {
    /// User message packet.
    Data,
    /// Acknowledgement packet.
    Ack,
    /// Transport control packet reserved for protocol runtime use.
    Control,
}

/// Metadata shared by every protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketHeader {
    /// Packet category.
    pub kind: PacketKind,
    /// Logical stream that owns the packet.
    pub stream_id: StreamId,
    /// Packet sequence number.
    pub seq: Seq,
    /// Packet flags.
    pub flags: Flags,
}

impl PacketHeader {
    /// Creates a packet header.
    #[must_use]
    pub const fn new(kind: PacketKind, stream_id: StreamId, seq: Seq, flags: Flags) -> Self {
        Self {
            kind,
            stream_id,
            seq,
            flags,
        }
    }

    /// Returns whether this packet should elicit an acknowledgement.
    #[must_use]
    pub const fn is_ack_eliciting(self) -> bool {
        self.flags.contains(Flags::ACK_ELICITING)
    }
}

/// Borrowed protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Packet<'a> {
    /// Packet metadata.
    pub header: PacketHeader,
    /// Borrowed packet payload.
    pub payload: &'a [u8],
}

impl<'a> Packet<'a> {
    /// Creates a borrowed packet.
    #[must_use]
    pub const fn new(header: PacketHeader, payload: &'a [u8]) -> Self {
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
