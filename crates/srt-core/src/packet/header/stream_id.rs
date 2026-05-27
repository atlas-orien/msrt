//! Packet header stream identifier primitive.

/// A logical stream identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamId(pub u16);

impl StreamId {
    /// Broadcast or control stream reserved by the protocol.
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

#[cfg(test)]
mod tests {
    use super::StreamId;

    #[test]
    fn control_stream_is_reserved_zero() {
        assert_eq!(StreamId::CONTROL.get(), 0);
        assert!(StreamId::CONTROL.is_control());
        assert!(!StreamId::new(1).is_control());
    }
}
