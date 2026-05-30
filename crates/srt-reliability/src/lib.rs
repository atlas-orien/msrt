#![no_std]
#![doc = "Reliability boundaries for Serial Realtime Transport."]

pub mod message;
pub mod packet;
pub mod policy;

pub use message::{FragmentRange, MessageFragment, MessageKey, MessageStatus};
pub use packet::{
    AckOutcome, AckTracker, Dedup, DedupDecision, PacketReliabilityEvent, PacketState,
    RetransmitDecision, RetransmitPolicy, SlidingWindow, TimeoutEvent, TimeoutPolicy,
    WindowDecision,
};
pub use policy::{ReliabilityMode, StreamReliability};
