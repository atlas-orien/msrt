//! Frame-level wire structures and constants.

use heapless::Vec;
use srt_core::Packet;

/// Frame synchronization byte.
pub const MAGIC: u8 = 0xA7;

/// Encoded magic field length.
pub const MAGIC_LEN: usize = 1;

/// Encoded length field length.
pub const LENGTH_LEN: usize = 2;

/// Encoded packet header length.
pub const HEADER_LEN: usize = 8;

/// Encoded CRC16 field length.
pub const CRC_LEN: usize = 2;

/// Minimum encoded frame length.
pub const MIN_FRAME_LEN: usize = MAGIC_LEN + LENGTH_LEN + HEADER_LEN + CRC_LEN;

/// Default fixed frame buffer capacity.
pub const DEFAULT_FRAME_CAPACITY: usize = 256;

/// Small fixed-capacity frame buffer for early integration tests.
pub type FrameBuf = Vec<u8, DEFAULT_FRAME_CAPACITY>;

/// Frame header used by the wire format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    /// Length of encoded packet header plus payload.
    pub length: u16,
}

impl FrameHeader {
    /// Creates a frame header.
    #[must_use]
    pub const fn new(length: u16) -> Self {
        Self { length }
    }
}

/// Borrowed frame view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame<'a> {
    /// Frame header.
    pub header: FrameHeader,
    /// Packet carried by this frame.
    pub packet: Packet<'a>,
}

impl<'a> Frame<'a> {
    /// Creates a borrowed frame.
    #[must_use]
    pub const fn new(header: FrameHeader, packet: Packet<'a>) -> Self {
        Self { header, packet }
    }
}
