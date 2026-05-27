//! Packet header encoder.

use srt_core::{Error, PacketHeader, PacketKind, Result};

use crate::frame::HEADER_LEN;

/// Encodes a packet header into its fixed-width wire form.
pub fn encode_packet_header(header: PacketHeader, out: &mut [u8]) -> Result<()> {
    if out.len() < HEADER_LEN {
        return Err(Error::buffer_too_small());
    }

    out[0] = encode_packet_kind(header.kind);
    out[1..3].copy_from_slice(&header.stream_id.get().to_be_bytes());
    out[3..7].copy_from_slice(&header.seq.get().to_be_bytes());
    out[7] = header.flags.bits();

    Ok(())
}

/// Encodes a packet kind into its wire value.
#[must_use]
pub const fn encode_packet_kind(kind: PacketKind) -> u8 {
    match kind {
        PacketKind::Data => 0x00,
        PacketKind::Ack => 0x01,
        PacketKind::Control => 0x02,
    }
}
