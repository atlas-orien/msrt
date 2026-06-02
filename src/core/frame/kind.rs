//! Protocol frame kind definitions.

/// Protocol frame type carried inside packet payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    /// Carries one message fragment on a channel.
    Message,
    /// Carries acknowledgement information.
    Ack,
}

impl FrameKind {
    /// Encoded MESSAGE frame type.
    pub const MESSAGE_CODE: u8 = 0x00;
    /// Encoded ACK frame type.
    pub const ACK_CODE: u8 = 0x01;

    /// Returns the encoded frame kind.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Message => Self::MESSAGE_CODE,
            Self::Ack => Self::ACK_CODE,
        }
    }

    /// Decodes a frame kind from raw bits.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            Self::MESSAGE_CODE => Some(Self::Message),
            Self::ACK_CODE => Some(Self::Ack),
            _ => None,
        }
    }
}
