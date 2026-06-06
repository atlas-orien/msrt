#![doc = "Endpoint session lifecycle helpers."]

pub mod client;
pub mod passive;
pub mod peer;
pub mod server;

pub use client::ClientEndpoint;
pub use passive::PassiveEndpoint;
pub use peer::{EndpointPoll, PeerSlot, PeerState};
pub use server::{AcceptError, PeerEntry, ServerEndpoint};
