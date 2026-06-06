//! Packet type definitions.

/// Packet type carried by the v1 packet header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketType {
    /// Packet carrying message fragment bytes.
    Data,
    /// Packet carrying best-effort log fragment bytes.
    Log,
    /// Packet carrying an ACK payload.
    Ack,
    /// Packet asking the peer to prove liveness.
    Ping,
    /// Packet proving liveness in response to a PING.
    Pong,
}

impl PacketType {
    /// Encoded DATA packet type.
    pub const DATA_CODE: u8 = 0x00;
    /// Encoded ACK packet type.
    pub const ACK_CODE: u8 = 0x01;
    /// Encoded PING packet type.
    pub const PING_CODE: u8 = 0x02;
    /// Encoded PONG packet type.
    pub const PONG_CODE: u8 = 0x03;
    /// Encoded LOG packet type.
    pub const LOG_CODE: u8 = 0x04;

    /// Returns the encoded packet type.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Data => Self::DATA_CODE,
            Self::Log => Self::LOG_CODE,
            Self::Ack => Self::ACK_CODE,
            Self::Ping => Self::PING_CODE,
            Self::Pong => Self::PONG_CODE,
        }
    }

    /// Decodes a packet type from raw bits.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            Self::DATA_CODE => Some(Self::Data),
            Self::LOG_CODE => Some(Self::Log),
            Self::ACK_CODE => Some(Self::Ack),
            Self::PING_CODE => Some(Self::Ping),
            Self::PONG_CODE => Some(Self::Pong),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PacketType;

    #[test]
    fn packet_type_codes_roundtrip() {
        assert_eq!(
            PacketType::from_code(PacketType::DATA_CODE),
            Some(PacketType::Data)
        );
        assert_eq!(
            PacketType::from_code(PacketType::LOG_CODE),
            Some(PacketType::Log)
        );
        assert_eq!(
            PacketType::from_code(PacketType::ACK_CODE),
            Some(PacketType::Ack)
        );
        assert_eq!(
            PacketType::from_code(PacketType::PING_CODE),
            Some(PacketType::Ping)
        );
        assert_eq!(
            PacketType::from_code(PacketType::PONG_CODE),
            Some(PacketType::Pong)
        );
        assert_eq!(PacketType::from_code(0xff), None);
    }
}
