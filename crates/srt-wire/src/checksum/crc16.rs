//! CRC-16/XMODEM checksum.

use super::Checksum;

/// CRC-16/XMODEM checksum.
///
/// Parameters:
///
/// - polynomial: `0x1021`
/// - initial value: `0x0000`
/// - xorout: `0x0000`
/// - refin: `false`
/// - refout: `false`
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Crc16;

impl Checksum for Crc16 {
    fn calculate(&self, bytes: &[u8]) -> u16 {
        bytes.iter().fold(0_u16, |mut checksum, byte| {
            checksum ^= u16::from(*byte) << 8;

            let mut bit = 0;
            while bit < 8 {
                if checksum & 0x8000 != 0 {
                    checksum = (checksum << 1) ^ 0x1021;
                } else {
                    checksum <<= 1;
                }
                bit += 1;
            }

            checksum
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Checksum, Crc16};

    #[test]
    fn checksum_verifies_calculated_value() {
        let checksum = Crc16;
        let bytes = [1, 2, 3];
        let expected = checksum.calculate(&bytes);

        assert!(checksum.verify(&bytes, expected));
        assert!(!checksum.verify(&bytes, expected.wrapping_add(1)));
    }

    #[test]
    fn checksum_matches_crc16_xmodem_check_value() {
        let checksum = Crc16;

        assert_eq!(checksum.calculate(b"123456789"), 0x31c3);
    }
}
