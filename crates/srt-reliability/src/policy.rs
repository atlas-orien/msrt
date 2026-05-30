//! Shared reliability policy descriptors.

use srt_core::StreamId;

/// Reliability mode requested for a stream or message class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReliabilityMode {
    /// Require acknowledgement and allow retransmission.
    Reliable,
    /// Send without retransmission.
    BestEffort,
    /// Prefer the newest message and allow older messages to be dropped.
    LatestOnly,
    /// Retransmit only while the runtime-defined deadline remains valid.
    Deadline,
}

/// Reliability configuration associated with one stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamReliability {
    /// Stream this policy applies to.
    pub stream_id: StreamId,
    /// Reliability mode for the stream.
    pub mode: ReliabilityMode,
    /// Maximum retransmission attempts allowed by this policy.
    pub max_retransmits: u8,
    /// Runtime-defined deadline in ticks.
    pub deadline_ticks: Option<u64>,
}

impl StreamReliability {
    /// Creates stream reliability configuration.
    #[must_use]
    pub const fn new(
        stream_id: StreamId,
        mode: ReliabilityMode,
        max_retransmits: u8,
        deadline_ticks: Option<u64>,
    ) -> Self {
        Self {
            stream_id,
            mode,
            max_retransmits,
            deadline_ticks,
        }
    }

    /// Creates a reliable stream policy.
    #[must_use]
    pub const fn reliable(stream_id: StreamId, max_retransmits: u8) -> Self {
        Self::new(stream_id, ReliabilityMode::Reliable, max_retransmits, None)
    }

    /// Creates a best-effort stream policy.
    #[must_use]
    pub const fn best_effort(stream_id: StreamId) -> Self {
        Self::new(stream_id, ReliabilityMode::BestEffort, 0, None)
    }
}
