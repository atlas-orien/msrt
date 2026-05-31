//! Shared reliability policy descriptors.

use srt_core::ChannelId;

/// Reliability mode requested for a channel or message class.
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

/// Reliability configuration associated with one channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelReliability {
    /// Channel this policy applies to.
    pub channel_id: ChannelId,
    /// Reliability mode for the channel.
    pub mode: ReliabilityMode,
    /// Maximum retransmission attempts allowed by this policy.
    pub max_retransmits: u8,
    /// Engine-defined deadline in ticks.
    pub deadline_ticks: Option<u64>,
}

impl ChannelReliability {
    /// Creates channel reliability configuration.
    #[must_use]
    pub const fn new(
        channel_id: ChannelId,
        mode: ReliabilityMode,
        max_retransmits: u8,
        deadline_ticks: Option<u64>,
    ) -> Self {
        Self {
            channel_id,
            mode,
            max_retransmits,
            deadline_ticks,
        }
    }

    /// Creates a reliable channel policy.
    #[must_use]
    pub const fn reliable(channel_id: ChannelId, max_retransmits: u8) -> Self {
        Self::new(channel_id, ReliabilityMode::Reliable, max_retransmits, None)
    }

    /// Creates a best-effort channel policy.
    #[must_use]
    pub const fn best_effort(channel_id: ChannelId) -> Self {
        Self::new(channel_id, ReliabilityMode::BestEffort, 0, None)
    }
}
