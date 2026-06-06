#![doc = "Reliability boundaries for MSRT."]

pub mod message;
pub mod packet;
pub mod policy;
pub mod recovery;

pub use message::{FragmentRange, MessageFragment, MessageKey, MessageStatus};
pub use packet::{
    AckOutcome, AckTracker, Dedup, DedupDecision, PacketAckTracker, PacketDedup,
    PacketReliabilityEvent, PacketState, RetransmitDecision, RetransmitPolicy, RetryLimitPolicy,
    SlidingWindow, TimeoutEvent, TimeoutPolicy, WindowDecision,
};
pub use policy::ReliabilityMode;
#[cfg(feature = "dynamic-recovery")]
pub use recovery::{DynamicRecoveryConfig, DynamicRecoveryState};
