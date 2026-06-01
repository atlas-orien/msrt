#![no_std]
#![doc = "Protocol engine boundaries for Serial Realtime Transport."]

pub mod config;
pub mod engine;
pub mod event;
pub(crate) mod layout;
pub mod link;
pub mod message;
pub mod scheduler;
pub mod time;

pub use config::{
    ChannelProfile, ChannelSpec, DEFAULT_FRAGMENT_BYTES, DEFAULT_MAX_RETRANSMIT_ATTEMPTS,
    DEFAULT_REASSEMBLY_TIMEOUT_MS, DEFAULT_RETRANSMIT_TIMEOUT_MS, EngineConfig,
    MAX_ACK_TRACKED_PACKETS, MAX_CHANNEL_POLICIES, MAX_CHANNEL_SPECS, MAX_EVENTS,
    MAX_IN_FLIGHT_PACKETS, MAX_INGRESS_BYTES, MAX_MESSAGE_BYTES, MAX_REASSEMBLY_MESSAGES,
    MAX_WIRE_BYTES,
};
pub use engine::{
    Engine, EngineOutput, MessageEvent, ReceiveReport, SendFailedEvent, SendFailureReason,
    WriteEvent,
};
pub use event::{EngineEvent, EngineEventKind};
pub use link::{LinkIo, LinkRead, LinkWrite, RawLink};
pub use message::{DeliveredMessage, MessageDelivery, Reassembly};
pub use scheduler::{Schedule, Scheduler};
pub use time::{Duration, Instant};
