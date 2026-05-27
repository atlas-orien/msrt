#![no_std]
#![doc = "Frame encoder and decoder boundaries for Serial Realtime Transport."]

use heapless::Vec;
use srt_core::{Error, Result};

/// Maximum placeholder frame buffer used by the scaffold API.
pub const DEFAULT_FRAME_CAPACITY: usize = 256;

/// Encodes protocol frames into a caller-owned buffer.
pub trait FrameEncoder {
    /// Encodes one payload into `out`.
    fn encode<'a>(&mut self, payload: &[u8], out: &'a mut [u8]) -> Result<&'a [u8]>;
}

/// Decodes bytes from a serial stream into complete frame payloads.
pub trait FrameDecoder {
    /// Pushes bytes into the decoder and returns whether a complete frame is available.
    fn push(&mut self, bytes: &[u8]) -> Result<DecodeStatus>;
}

/// Decoder progress after receiving bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeStatus {
    /// More bytes are needed.
    NeedMore,
    /// A full frame is available.
    FrameReady,
    /// Bytes were skipped while resynchronizing.
    Resynced,
}

/// Placeholder CRC16 contract.
pub trait Crc16 {
    /// Computes a CRC16 value for `bytes`.
    fn checksum(bytes: &[u8]) -> u16;
}

/// Small fixed-capacity frame buffer for early integration tests.
pub type FrameBuf = Vec<u8, DEFAULT_FRAME_CAPACITY>;

/// Minimal encoder placeholder.
#[derive(Debug, Default)]
pub struct PassthroughEncoder;

impl FrameEncoder for PassthroughEncoder {
    fn encode<'a>(&mut self, payload: &[u8], out: &'a mut [u8]) -> Result<&'a [u8]> {
        if out.len() < payload.len() {
            return Err(Error::BufferTooSmall);
        }

        let len = payload.len();
        out[..len].copy_from_slice(payload);
        Ok(&out[..len])
    }
}
