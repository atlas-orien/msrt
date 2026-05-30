//! CRC16 checksum marker.

use super::Checksum;

/// CRC16 checksum boundary.
///
/// The concrete polynomial is intentionally not frozen yet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Crc16;

impl Checksum for Crc16 {
    fn calculate(&self, bytes: &[u8]) -> u16 {
        bytes.iter().fold(0_u16, |checksum, byte| {
            checksum.wrapping_add(u16::from(*byte))
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
}
