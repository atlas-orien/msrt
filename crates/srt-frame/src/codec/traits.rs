//! Frame codec traits.

use srt_core::{Packet, Result};

use super::decoder::DecodeStatus;

/// Encodes SRT packets into frame bytes.
pub trait FrameEncoder {
    /// Encodes one packet into `out`.
    fn encode_packet<'a>(&mut self, packet: Packet<'_>, out: &'a mut [u8]) -> Result<&'a [u8]>;
}

/// Decodes frame bytes from a serial-like stream.
pub trait FrameDecoder {
    /// Pushes bytes into the decoder and returns decoder progress.
    fn push(&mut self, bytes: &[u8]) -> Result<DecodeStatus>;
}
