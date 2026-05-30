//! Wire decoder resynchronization state.

/// Decoder resynchronization state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResyncState {
    /// Decoder is aligned and waiting for envelope bytes.
    Aligned,
    /// Decoder needs more bytes to decide whether the current prefix is valid.
    NeedMore,
    /// Decoder is scanning for the next magic value.
    Scanning,
    /// Decoder found a candidate magic prefix.
    Candidate {
        /// Offset where the candidate starts.
        offset: usize,
    },
}

impl ResyncState {
    /// Returns whether the decoder is aligned.
    #[must_use]
    pub const fn is_aligned(self) -> bool {
        matches!(self, Self::Aligned)
    }
}
