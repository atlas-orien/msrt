#![no_std]
#![doc = "Protocol runtime boundaries for Serial Realtime Transport."]

pub mod event;
pub mod link;
pub mod message;
pub mod receive;
pub mod runtime;
pub mod scheduler;
pub mod send;
pub mod time;

pub use event::{RuntimeEvent, RuntimeEventKind};
pub use link::{LinkIo, LinkRead, LinkWrite, RawLink};
pub use message::{DeliveredMessage, MessageDelivery, Reassembly};
pub use receive::{PacketInput, ReceiveAction, ReceiveInput, Receiver};
pub use runtime::{ProtocolRuntime, RuntimeDriver};
pub use scheduler::{Schedule, Scheduler};
pub use send::{SendIntent, SendOptions, Sender};
pub use time::{Duration, Instant};
