#![no_std]
#![doc = "Protocol engine boundaries for Serial Realtime Transport."]

pub mod engine;
pub mod event;
pub mod link;
pub mod message;
pub mod receive;
pub mod scheduler;
pub mod send;
pub mod time;

pub use engine::{EngineDriver, ProtocolEngine};
pub use event::{EngineEvent, EngineEventKind};
pub use link::{LinkIo, LinkRead, LinkWrite, RawLink};
pub use message::{DeliveredMessage, MessageDelivery, Reassembly};
pub use receive::{
    FeedProgress, PacketInput, ReceiveAction, ReceiveInput, ReceiveProgress, Receiver,
};
pub use scheduler::{Schedule, Scheduler};
pub use send::{SendIntent, SendOptions, Sender};
pub use time::{Duration, Instant};
