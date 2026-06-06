//! PING/PONG packet headers.

/// Header for liveness probes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PingHeader;

impl PingHeader {
    /// Creates a PING header.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Header for liveness probe responses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PongHeader;

impl PongHeader {
    /// Creates a PONG header.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
