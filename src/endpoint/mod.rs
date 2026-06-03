#![doc = "Endpoint session lifecycle helpers."]

pub mod client;
pub mod peer;
pub mod server;

pub use client::ClientEndpoint;
pub use peer::PeerSlot;
pub use server::{AcceptError, ServerEndpoint};
