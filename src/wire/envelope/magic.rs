//! Wire magic value.

/// Magic bytes used to find wire envelope boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopeMagic(pub [u8; 1]);

impl EnvelopeMagic {
    /// Default MSRT wire magic.
    pub const MSRT: Self = Self([0xA5]);

    /// Creates magic from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 1]) -> Self {
        Self(bytes)
    }

    /// Returns raw magic bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 1] {
        self.0
    }

    /// Returns whether `bytes` starts with this magic value.
    #[must_use]
    pub fn matches_prefix(self, bytes: &[u8]) -> bool {
        bytes.starts_with(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::EnvelopeMagic;

    #[test]
    fn magic_matches_prefix() {
        assert!(EnvelopeMagic::MSRT.matches_prefix(&[0xA5, 0x01]));
        assert!(!EnvelopeMagic::MSRT.matches_prefix(&[0x5A]));
    }
}
