//! Protocol identifiers.

/// A logical stream identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamId(pub u16);
