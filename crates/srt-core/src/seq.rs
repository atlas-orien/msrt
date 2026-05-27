//! Sequence number primitives.

/// A packet sequence number.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Seq(pub u32);
