#![no_std]
#![doc = "Shared protocol error types for Serial Realtime Transport."]

/// Broad error category for SRT protocol failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// The input was malformed.
    Malformed,
    /// The provided buffer was too small.
    BufferTooSmall,
    /// A frame boundary or checksum failed.
    Frame,
    /// A reliability invariant failed.
    Reliability,
    /// A stream invariant failed.
    Stream,
    /// A protocol engine invariant failed.
    Engine,
    /// The requested operation is unsupported by this implementation.
    Unsupported,
}

/// Shared SRT protocol error.
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

/// Shared result type for SRT protocol crates.
pub type Result<T> = core::result::Result<T, Error>;
