//! Packet-level reliability boundaries.

pub mod ack;
pub mod dedup;
pub mod event;
pub mod retransmit;
pub mod state;
pub mod timeout;
pub mod window;

pub use ack::{AckOutcome, AckTracker};
pub use dedup::{Dedup, DedupDecision};
pub use event::PacketReliabilityEvent;
pub use retransmit::{RetransmitDecision, RetransmitPolicy};
pub use state::PacketState;
pub use timeout::{TimeoutEvent, TimeoutPolicy};
pub use window::{SlidingWindow, WindowDecision};
