//! Fixed-capacity decoder buffer.

use heapless::Vec;
use srt_core::{Error, Result};

use crate::frame::DEFAULT_FRAME_CAPACITY;

/// Fixed-capacity decoder byte buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecoderBuffer<const N: usize = DEFAULT_FRAME_CAPACITY> {
    bytes: Vec<u8, N>,
}

impl<const N: usize> DecoderBuffer<N> {
    /// Creates an empty decoder buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Returns buffered bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    /// Returns buffered length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Appends bytes to the decoder buffer.
    pub fn extend_from_slice(&mut self, bytes: &[u8]) -> Result<()> {
        self.bytes
            .extend_from_slice(bytes)
            .map_err(|_| Error::buffer_too_small())
    }

    /// Clears buffered bytes.
    pub fn clear(&mut self) {
        self.bytes.clear();
    }
}

impl<const N: usize> Default for DecoderBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::DecoderBuffer;

    #[test]
    fn reports_buffer_overflow() {
        let mut buffer = DecoderBuffer::<2>::new();

        assert!(buffer.extend_from_slice(&[1, 2]).is_ok());
        assert!(buffer.extend_from_slice(&[3]).is_err());
    }
}
