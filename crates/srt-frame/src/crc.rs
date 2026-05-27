//! CRC16 support for frame validation.

/// CRC16 calculation contract.
pub trait Crc16 {
    /// Computes a CRC16 value for `bytes`.
    fn checksum(bytes: &[u8]) -> u16;
}

/// CRC-16/CCITT-FALSE implementation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Crc16CcittFalse;

impl Crc16 for Crc16CcittFalse {
    fn checksum(bytes: &[u8]) -> u16 {
        let mut crc = 0xFFFF;

        for byte in bytes {
            crc ^= u16::from(*byte) << 8;

            for _ in 0..8 {
                if (crc & 0x8000) != 0 {
                    crc = (crc << 1) ^ 0x1021;
                } else {
                    crc <<= 1;
                }
            }
        }

        crc
    }
}

#[cfg(test)]
mod tests {
    use super::{Crc16, Crc16CcittFalse};

    #[test]
    fn computes_standard_check_value() {
        assert_eq!(Crc16CcittFalse::checksum(b"123456789"), 0x29B1);
    }
}
