//! Shared reliability policy descriptors.

/// Reliability mode requested for a packet kind or message class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReliabilityMode {
    /// Require acknowledgement and allow retransmission.
    Reliable,
    /// Send without retransmission.
    BestEffort,
    /// Prefer the newest message and allow older messages to be dropped.
    LatestOnly,
    /// Retransmit only while the engine-defined deadline remains valid.
    Deadline,
}
