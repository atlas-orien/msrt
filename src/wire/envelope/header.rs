//! Wire envelope header.

use crate::integrity::crc::Crc8;

use super::EnvelopeMagic;

/// Fixed first-stage wire magic length.
pub const WIRE_MAGIC_LEN: usize = 1;

/// Fixed first-stage wire header length.
pub const WIRE_HEADER_LEN: usize = WIRE_MAGIC_LEN + core::mem::size_of::<u8>() + 1;

/// Offset of the packet length field.
pub const WIRE_PACKET_LEN_OFFSET: usize = WIRE_MAGIC_LEN;

/// Offset of the header CRC-8 byte.
pub const WIRE_HEADER_CRC_OFFSET: usize = WIRE_PACKET_LEN_OFFSET + core::mem::size_of::<u8>();

/// Wire envelope header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopeHeader {
    /// Magic used to locate an envelope in a byte stream.
    pub magic: EnvelopeMagic,
    /// Encoded MSRT packet length.
    pub packet_len: u8,
    /// CRC-8 over the packet length field.
    pub header_crc: u8,
}

impl EnvelopeHeader {
    /// Creates a wire envelope header.
    #[must_use]
    pub const fn new(packet_len: u8) -> Self {
        Self {
            magic: EnvelopeMagic::MSRT,
            packet_len,
            header_crc: header_crc(packet_len),
        }
    }

    /// Returns whether this header has a valid CRC-8.
    #[must_use]
    pub const fn has_valid_header_crc(self) -> bool {
        self.header_crc == header_crc(self.packet_len)
    }

    /// Returns the complete envelope length including integrity tag bytes.
    #[must_use]
    pub const fn total_len(self, integrity_tag_len: usize) -> usize {
        WIRE_HEADER_LEN + self.packet_len as usize + integrity_tag_len
    }
}

/// Calculates the CRC-8 over the protected wire length field.
#[must_use]
pub const fn header_crc(packet_len: u8) -> u8 {
    Crc8.calculate(&[packet_len])
}

#[cfg(test)]
mod tests {
    use super::{EnvelopeHeader, WIRE_HEADER_LEN, header_crc};

    #[test]
    fn header_total_len_includes_integrity_tag() {
        let header = EnvelopeHeader::new(9);
        let integrity_tag_len = 2;

        assert!(header.has_valid_header_crc());
        assert_eq!(
            header.total_len(integrity_tag_len),
            WIRE_HEADER_LEN + 9 + integrity_tag_len
        );
    }

    #[test]
    fn header_crc_changes_when_length_changes() {
        assert_ne!(header_crc(9), header_crc(10));
    }
}
