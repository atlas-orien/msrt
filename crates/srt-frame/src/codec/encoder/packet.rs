//! Packet frame encoder.

use srt_core::{Error, Packet, Result};

use crate::codec::FrameEncoder;
use crate::codec::encoder::header::encode_packet_header;
use crate::crc::{Crc16, Crc16CcittFalse};
use crate::frame::{CRC_LEN, HEADER_LEN, LENGTH_LEN, MAGIC, MAGIC_LEN};

/// Stateless packet frame encoder.
#[derive(Debug, Default)]
pub struct PacketFrameEncoder;

impl FrameEncoder for PacketFrameEncoder {
    fn encode_packet<'a>(&mut self, packet: Packet<'_>, out: &'a mut [u8]) -> Result<&'a [u8]> {
        encode_packet(packet, out)
    }
}

/// Encodes one packet into a frame.
pub fn encode_packet<'a>(packet: Packet<'_>, out: &'a mut [u8]) -> Result<&'a [u8]> {
    let payload = packet.payload.as_bytes();
    let body_len = HEADER_LEN
        .checked_add(payload.len())
        .ok_or_else(Error::buffer_too_small)?;
    let body_len_u16 = u16::try_from(body_len).map_err(|_| Error::buffer_too_small())?;
    let frame_len = MAGIC_LEN + LENGTH_LEN + body_len + CRC_LEN;

    if out.len() < frame_len {
        return Err(Error::buffer_too_small());
    }

    out[0] = MAGIC;
    out[1..3].copy_from_slice(&body_len_u16.to_be_bytes());
    encode_packet_header(packet.header, &mut out[3..3 + HEADER_LEN])?;
    out[3 + HEADER_LEN..3 + body_len].copy_from_slice(payload);

    let crc = Crc16CcittFalse::checksum(&out[1..3 + body_len]);
    out[3 + body_len..frame_len].copy_from_slice(&crc.to_be_bytes());

    Ok(&out[..frame_len])
}

#[cfg(test)]
mod tests {
    use super::PacketFrameEncoder;
    use crate::codec::FrameEncoder;
    use crate::frame::{CRC_LEN, HEADER_LEN, LENGTH_LEN, MAGIC, MAGIC_LEN};
    use srt_core::{Flags, Packet, PacketHeader, PacketKind, Seq, StreamId};

    #[test]
    fn encodes_packet_into_frame_bytes() {
        let header = PacketHeader::new(
            PacketKind::Data,
            StreamId::new(0x0102),
            Seq::new(0x0304_0506),
            Flags::ACK_ELICITING.union(Flags::REALTIME),
        );
        let payload = [0xAA, 0xBB];
        let packet = Packet::new(header, &payload);
        let mut out = [0; 32];

        let encoded = PacketFrameEncoder
            .encode_packet(packet, &mut out)
            .expect("packet should encode");

        assert_eq!(encoded[0], MAGIC);
        assert_eq!(encoded[1..3], 10_u16.to_be_bytes());
        assert_eq!(encoded[3], 0x00);
        assert_eq!(encoded[4..6], [0x01, 0x02]);
        assert_eq!(encoded[6..10], [0x03, 0x04, 0x05, 0x06]);
        assert_eq!(
            encoded[10],
            Flags::ACK_ELICITING.union(Flags::REALTIME).bits()
        );
        assert_eq!(encoded[11..13], payload);
        assert_eq!(
            encoded.len(),
            MAGIC_LEN + LENGTH_LEN + HEADER_LEN + payload.len() + CRC_LEN
        );
    }
}
