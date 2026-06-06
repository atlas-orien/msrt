#![doc = "Reliability boundaries for MSRT."]

pub mod message;
pub mod packet;
pub mod policy;

pub use message::{FragmentRange, MessageFragment, MessageKey, MessageStatus};
pub use packet::{
    AckOutcome, AckTracker, Dedup, DedupDecision, PacketAckTracker, PacketDedup,
    PacketReliabilityEvent, PacketState, RetransmitDecision, RetransmitPolicy, RetryLimitPolicy,
    SlidingWindow, TimeoutEvent, TimeoutPolicy, WindowDecision,
};
pub use policy::{ChannelReliability, ReliabilityMode};
