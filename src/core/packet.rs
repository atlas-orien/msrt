//! Packet-level protocol primitives.

pub mod body;
pub mod header;
pub mod number;
pub mod payload;
pub mod ty;

pub use body::PacketBody;
pub use header::{
    ACK_PACKET_HEADER_LEN, AckHeader, DATA_PACKET_HEADER_LEN, DataHeader, Flags,
    LIVENESS_PACKET_HEADER_LEN, LOG_PACKET_HEADER_LEN, LogHeader, PacketHeader, PingHeader,
    PongHeader,
};
pub use number::{PacketIndex, PacketKey};
pub use payload::PacketPayload;
pub use ty::PacketType;

/// Borrowed protocol packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Packet<'a> {
    /// Packet kind.
    pub packet_type: PacketType,
    /// Kind-specific packet content.
    pub body: PacketBody<'a>,
}

impl<'a> Packet<'a> {
    /// Creates a borrowed packet from a legacy header.
    #[must_use]
    pub const fn new(header: PacketHeader, payload: &'a [u8]) -> Self {
        Self::from_legacy_header(header, payload)
    }

    /// Creates a borrowed DATA packet.
    #[must_use]
    pub const fn data(header: PacketHeader, payload: &'a [u8]) -> Self {
        Self::from_legacy_header(header, payload)
    }

    /// Creates a borrowed control packet without payload.
    #[must_use]
    pub const fn control(header: PacketHeader) -> Self {
        Self::from_legacy_header(header, &[])
    }

    /// Creates a borrowed packet from explicit kind-specific content.
    #[must_use]
    pub const fn from_parts(packet_type: PacketType, body: PacketBody<'a>) -> Self {
        Self { packet_type, body }
    }

    const fn from_legacy_header(header: PacketHeader, payload: &'a [u8]) -> Self {
        let packet_type = header.packet_type;
        let body = match header.body {
            header::PacketHeaderBody::Data { header, .. } => PacketBody::Data {
                header,
                payload: PacketPayload::new(payload),
            },
            header::PacketHeaderBody::Log { header, .. } => PacketBody::Log {
                header,
                payload: PacketPayload::new(payload),
            },
            header::PacketHeaderBody::Ack { header, .. } => PacketBody::Ack { header },
            header::PacketHeaderBody::Ping { header, .. } => PacketBody::Ping { header },
            header::PacketHeaderBody::Pong { header, .. } => PacketBody::Pong { header },
        };

        Self { packet_type, body }
    }

    /// Returns a legacy header view for compatibility with old facade code.
    #[must_use]
    pub const fn header(self) -> PacketHeader {
        match self.body {
            PacketBody::Data { header, .. } => PacketHeader::from_data_header(header),
            PacketBody::Log { header, .. } => PacketHeader::from_log_header(header),
            PacketBody::Ack { header } => PacketHeader::ack(crate::core::PacketKey::new(
                header.message_id,
                header.packet_index,
            )),
            PacketBody::Ping { .. } => PacketHeader::ping(crate::core::MessageId::ZERO),
            PacketBody::Pong { .. } => PacketHeader::pong(crate::core::MessageId::ZERO),
        }
    }

    /// Returns whether this packet should elicit an acknowledgement.
    #[must_use]
    pub const fn is_ack_eliciting(self) -> bool {
        match self.body {
            PacketBody::Data { header, .. } => header.is_ack_eliciting(),
            PacketBody::Log { .. }
            | PacketBody::Ack { .. }
            | PacketBody::Ping { .. }
            | PacketBody::Pong { .. } => false,
        }
    }

    /// Creates a borrowed DATA packet from exact DATA parts.
    #[must_use]
    pub const fn data_parts(header: DataHeader, payload: &'a [u8]) -> Self {
        Self {
            packet_type: PacketType::Data,
            body: PacketBody::Data {
                header,
                payload: PacketPayload::new(payload),
            },
        }
    }

    /// Creates a borrowed LOG packet from exact LOG parts.
    #[must_use]
    pub const fn log_parts(header: LogHeader, payload: &'a [u8]) -> Self {
        Self {
            packet_type: PacketType::Log,
            body: PacketBody::Log {
                header,
                payload: PacketPayload::new(payload),
            },
        }
    }

    /// Creates a borrowed ACK packet.
    #[must_use]
    pub const fn ack(header: AckHeader) -> Self {
        Self {
            packet_type: PacketType::Ack,
            body: PacketBody::Ack { header },
        }
    }

    /// Creates a borrowed PING packet.
    #[must_use]
    pub const fn ping(header: PingHeader) -> Self {
        Self {
            packet_type: PacketType::Ping,
            body: PacketBody::Ping { header },
        }
    }

    /// Creates a borrowed PONG packet.
    #[must_use]
    pub const fn pong(header: PongHeader) -> Self {
        Self {
            packet_type: PacketType::Pong,
            body: PacketBody::Pong { header },
        }
    }

    /// Returns the packet payload length in bytes.
    #[must_use]
    pub const fn payload_len(self) -> usize {
        match self.body {
            PacketBody::Data { payload, .. } | PacketBody::Log { payload, .. } => payload.len(),
            PacketBody::Ack { .. } | PacketBody::Ping { .. } | PacketBody::Pong { .. } => 0,
        }
    }

    /// Returns whether the packet payload is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        match self.body {
            PacketBody::Data { payload, .. } | PacketBody::Log { payload, .. } => {
                payload.is_empty()
            }
            PacketBody::Ack { .. } | PacketBody::Ping { .. } | PacketBody::Pong { .. } => true,
        }
    }

    /// Returns the packet payload bytes, or an empty slice for control packets.
    #[must_use]
    pub const fn payload_bytes(self) -> &'a [u8] {
        match self.body {
            PacketBody::Data { payload, .. } | PacketBody::Log { payload, .. } => {
                payload.as_bytes()
            }
            PacketBody::Ack { .. } | PacketBody::Ping { .. } | PacketBody::Pong { .. } => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Flags, Packet, PacketBody, PacketHeader, PacketIndex};
    use crate::core::MessageId;

    #[test]
    fn packet_payload_contains_bytes() {
        let payload = [1, 2, 3];
        let header = PacketHeader::data(
            PacketIndex::new(0),
            Flags::ACK_ELICITING,
            MessageId::new(7),
            3,
            0,
        );
        let packet = Packet::new(header, &payload);

        assert_eq!(packet.payload_len(), 3);
        assert!(!packet.is_empty());
        assert_eq!(packet.payload_bytes(), payload);
        assert!(packet.is_ack_eliciting());
    }

    #[test]
    fn ack_packet_has_no_payload() {
        let header = PacketHeader::ack(crate::core::PacketKey::new(
            MessageId::new(7),
            PacketIndex::new(0),
        ));
        let packet = Packet::new(header, &[1, 2, 3]);

        assert_eq!(packet.payload_len(), 0);
        assert!(packet.is_empty());
        assert!(matches!(packet.body, PacketBody::Ack { .. }));
    }
}
