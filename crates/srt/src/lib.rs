#![no_std]
#![doc = "No-std facade crate for the Serial Realtime Transport protocol."]

/// Core protocol primitives.
pub use srt_core as core;
/// Protocol engine boundaries.
pub use srt_engine as engine;
/// Shared protocol errors.
pub use srt_error as error;
/// Reliability policy boundaries.
pub use srt_reliability as reliability;
/// Wire envelope boundaries.
pub use srt_wire as wire;

pub use srt_core::ChannelId;
pub use srt_engine::{
    ChannelProfile, ChannelSpec, DEFAULT_FRAGMENT_BYTES, DEFAULT_MAX_RETRANSMIT_ATTEMPTS,
    DEFAULT_REASSEMBLY_TIMEOUT_MS, DEFAULT_RETRANSMIT_TIMEOUT_MS, Engine, EngineConfig,
    EngineOutput, MAX_CHANNEL_POLICIES, MAX_CHANNEL_SPECS, MAX_EVENTS, MAX_MESSAGE_BYTES,
    MAX_WIRE_BYTES, MessageEvent, ReceiveReport, SendFailedEvent, SendFailureReason, WriteEvent,
};
pub use srt_reliability::{ChannelReliability, ReliabilityMode};

/// User-facing engine configuration.
pub type Config = EngineConfig;
/// User-facing engine output event.
pub type Event = EngineOutput;
/// User-facing delivered message event.
pub type Message = MessageEvent;
/// User-facing wire write event.
pub type Write = WriteEvent;
/// User-facing receive report.
pub type Receive = ReceiveReport;
/// User-facing failed send event.
pub type SendFailed = SendFailedEvent;
