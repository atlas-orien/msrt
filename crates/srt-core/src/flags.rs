//! Protocol flag primitives.

/// Protocol flags carried by packets or frames.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Flags(pub u16);
