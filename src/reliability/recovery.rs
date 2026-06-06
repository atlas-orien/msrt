//! Packet recovery timeout policies.

#[cfg(feature = "dynamic-recovery")]
pub mod dynamic;

#[cfg(feature = "dynamic-recovery")]
pub use dynamic::{DynamicRecoveryConfig, DynamicRecoveryState};
