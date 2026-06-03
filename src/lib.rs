#![cfg_attr(not(feature = "std"), no_std)]
#![doc = "Portable MSRT protocol implementation."]
#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![warn(
    clippy::alloc_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core
)]

/// Core protocol primitives.
pub mod core;
/// Protocol engine boundaries.
pub mod engine;
/// Shared protocol errors.
pub mod error;
/// Reliability policy boundaries.
pub mod reliability;
/// Wire envelope boundaries.
pub mod wire;

pub use crate::core::ChannelId;
pub use crate::engine::{
    ChannelProfile, ChannelSpec, DEFAULT_FRAGMENT_BYTES, DEFAULT_MAX_RETRANSMIT_ATTEMPTS,
    DEFAULT_REASSEMBLY_TIMEOUT_MS, DEFAULT_RETRANSMIT_TIMEOUT_MS, Engine, EngineConfig,
    EngineOutput, EnginePoll, MAX_CHANNEL_POLICIES, MAX_CHANNEL_SPECS, MAX_EVENTS,
    MAX_MESSAGE_BYTES, MAX_WIRE_BYTES, MessageEvent, ReceiveReport, SendFailedEvent,
    SendFailureReason, WriteEvent,
};
pub use crate::reliability::{ChannelReliability, ReliabilityMode};

/// User-facing engine configuration.
pub type Config = EngineConfig;
/// User-facing engine output event.
pub type Event = EngineOutput;
/// User-facing engine poll action.
pub type Poll<'a> = EnginePoll<'a>;
/// User-facing delivered message event.
pub type Message = MessageEvent;
/// User-facing wire write event.
pub type Write = WriteEvent;
/// User-facing receive report.
pub type Receive = ReceiveReport;
/// User-facing failed send event.
pub type SendFailed = SendFailedEvent;
