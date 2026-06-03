#![doc = "Shared protocol error types for MSRT."]

/// Broad error category for MSRT protocol failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// The input was malformed.
    Malformed,
    /// The provided buffer was too small.
    BufferTooSmall,
    /// A packet boundary or checksum failed.
    Packet,
    /// A reliability invariant failed.
    Reliability,
    /// A channel invariant failed.
    Channel,
    /// A protocol engine invariant failed.
    Engine,
    /// The requested operation is unsupported by this implementation.
    Unsupported,
}

/// Shared MSRT protocol error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    kind: ErrorKind,
}

impl Error {
    /// Creates a new protocol error from an error kind.
    #[must_use]
    pub const fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the broad error category.
    #[must_use]
    pub const fn kind(self) -> ErrorKind {
        self.kind
    }

    /// Returns a malformed-input error.
    #[must_use]
    pub const fn malformed() -> Self {
        Self::new(ErrorKind::Malformed)
    }

    /// Returns a buffer-too-small error.
    #[must_use]
    pub const fn buffer_too_small() -> Self {
        Self::new(ErrorKind::BufferTooSmall)
    }

    /// Returns an unsupported-operation error.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self::new(ErrorKind::Unsupported)
    }
}

/// Shared result type for MSRT protocol crates.
pub type Result<T> = core::result::Result<T, Error>;
