#![no_std]
#![doc = "Stream routing and state boundaries for Serial Realtime Transport."]

use srt_core::StreamId;

/// Quality-of-service class reserved for a stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qos {
    /// Best-effort delivery.
    BestEffort,
    /// Realtime delivery preference.
    Realtime,
    /// Reliable delivery preference.
    Reliable,
}

/// Relative stream priority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Priority(pub u8);

/// Coarse stream lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamState {
    /// Stream can accept traffic.
    Open,
    /// Stream is draining pending traffic.
    Draining,
    /// Stream is closed.
    Closed,
}

/// Routes stream traffic to a runtime or transport endpoint.
pub trait StreamRouter {
    /// Route type returned by the router.
    type Route;

    /// Resolves a route for `stream_id`.
    fn route(&self, stream_id: StreamId) -> Option<Self::Route>;
}
