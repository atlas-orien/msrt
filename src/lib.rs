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

pub use crate::engine::{Engine, EngineConfig};
