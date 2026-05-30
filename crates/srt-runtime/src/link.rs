//! Lower link boundaries.

use srt_core::Result;

/// Read side of a raw byte link.
pub trait LinkRead {
    /// Attempts to read bytes from the raw link into `buf`.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}

/// Write side of a raw byte link.
pub trait LinkWrite {
    /// Attempts to write bytes from `buf` to the raw link.
    fn write(&mut self, buf: &[u8]) -> Result<usize>;
}

/// Raw byte link used by the protocol runtime.
///
/// Implementations may be backed by UART, USB CDC, TCP, tests, or any other byte stream.
pub trait RawLink: LinkRead + LinkWrite {}

impl<T> RawLink for T where T: LinkRead + LinkWrite {}

/// Direction requested by the runtime when interacting with the lower link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkIo {
    /// Runtime wants to read bytes from the lower link.
    Read,
    /// Runtime wants to write bytes to the lower link.
    Write,
    /// Runtime does not currently need link progress.
    Idle,
}
