//! Message reassembly status.

/// High-level status for message fragment collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageStatus {
    /// The message has not received all required fragments yet.
    Pending,
    /// The message has received all required fragments and can be delivered.
    Complete,
    /// The message cannot be completed because an invariant failed.
    Invalid,
    /// The message was dropped by reliability policy.
    Dropped,
}
