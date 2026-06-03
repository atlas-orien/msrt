//! Checksum boundaries.

pub mod crc16;

pub use crc16::Crc16;

/// Encoded checksum length in bytes.
pub(crate) const CHECKSUM_LEN: usize = core::mem::size_of::<u16>();

/// Calculates and verifies checksums for wire bytes.
pub trait Checksum {
    /// Calculates checksum for `bytes`.
    fn calculate(&self, bytes: &[u8]) -> u16;

    /// Returns whether `expected` matches the checksum of `bytes`.
    fn verify(&self, bytes: &[u8], expected: u16) -> bool {
        self.calculate(bytes) == expected
    }
}
