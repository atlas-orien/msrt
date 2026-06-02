//! Wire magic value.

/// Magic bytes used to find wire envelope boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopeMagic(pub [u8; 2]);

impl EnvelopeMagic {
    /// Default SRT wire magic.
    pub const SRT: Self = Self(*b"SR");

    /// Creates magic from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 2]) -> Self {
        Self(bytes)
    }

    /// Returns raw magic bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 2] {
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
        assert!(EnvelopeMagic::SRT.matches_prefix(b"SRT"));
        assert!(!EnvelopeMagic::SRT.matches_prefix(b"RT"));
    }
}
