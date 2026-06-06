#![doc = "Endpoint session lifecycle helpers."]

pub mod client;
pub mod passive;
pub mod peer;
pub mod server;

pub use crate::core::{MessageId, PacketIndex, PacketType};
pub use crate::engine::{
    EngineConfig, MessageEvent, ReceiveReport, SendFailedEvent, SendFailureReason,
};
pub use crate::integrity::IntegrityConfig;
#[cfg(feature = "dynamic-recovery")]
pub use crate::reliability::DynamicRecoveryConfig;
pub use client::ClientEndpoint;
pub use passive::PassiveEndpoint;
pub use peer::{EndpointPoll, PeerSlot, PeerState};
pub use server::{AcceptError, PeerEntry, ServerEndpoint};
