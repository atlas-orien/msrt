//! Wire envelope header.

use super::EnvelopeMagic;

/// Fixed first-stage wire magic length.
pub const WIRE_MAGIC_LEN: usize = 1;

/// Fixed first-stage wire header length.
pub const WIRE_HEADER_LEN: usize = WIRE_MAGIC_LEN + core::mem::size_of::<u8>() + 1;

/// Offset of the packet length field.
pub const WIRE_PACKET_LEN_OFFSET: usize = WIRE_MAGIC_LEN;

/// Offset of the header CRC-8 byte.
pub const WIRE_HEADER_CRC_OFFSET: usize = WIRE_PACKET_LEN_OFFSET + core::mem::size_of::<u8>();

/// Fixed first-stage wire checksum length.
pub const CHECKSUM_LEN: usize = core::mem::size_of::<u16>();

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

    /// Returns the complete envelope length including checksum bytes.
    #[must_use]
    pub const fn total_len(self) -> usize {
        WIRE_HEADER_LEN + self.packet_len as usize + CHECKSUM_LEN
    }
}

/// Calculates the CRC-8 over the protected wire length field.
#[must_use]
pub const fn header_crc(packet_len: u8) -> u8 {
    let bytes = [packet_len];
    let mut checksum = 0_u8;
    let mut index = 0;

    while index < bytes.len() {
        checksum ^= bytes[index];
        let mut bit = 0;
        while bit < 8 {
            if checksum & 0x80 != 0 {
                checksum = (checksum << 1) ^ 0x07;
            } else {
                checksum <<= 1;
            }
            bit += 1;
        }
        index += 1;
    }

    checksum
}

#[cfg(test)]
mod tests {
    use super::{EnvelopeHeader, WIRE_HEADER_LEN, header_crc};

    #[test]
    fn header_total_len_includes_checksum() {
        let header = EnvelopeHeader::new(9);

        assert!(header.has_valid_header_crc());
        assert_eq!(header.total_len(), WIRE_HEADER_LEN + 9 + 2);
    }

    #[test]
    fn header_crc_changes_when_length_changes() {
        assert_ne!(header_crc(9), header_crc(10));
    }
}
