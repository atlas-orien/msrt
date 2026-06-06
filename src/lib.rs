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
pub(crate) mod core;
/// Endpoint session lifecycle helpers.
pub mod endpoint;
/// Protocol engine boundaries.
pub(crate) mod engine;
/// Shared protocol errors.
pub mod error;
/// Packet integrity backends.
pub(crate) mod integrity;
/// Reliability policy boundaries.
pub(crate) mod reliability;
/// Wire envelope boundaries.
pub(crate) mod wire;
