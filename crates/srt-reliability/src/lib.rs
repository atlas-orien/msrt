#![no_std]
#![doc = "Reliability boundaries for Serial Realtime Transport."]

pub mod ack;
pub mod dedup;
pub mod retransmit;
pub mod timeout;
pub mod window;
