//! Receive-side engine boundaries.

use srt_core::{Packet, PacketNumber, Result};
use srt_reliability::DedupDecision;

use crate::link::LinkRead;

/// Raw bytes received from the lower link boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiveInput<'a> {
    /// Borrowed bytes from the lower layer.
    pub bytes: &'a [u8],
}

impl<'a> ReceiveInput<'a> {
    /// Creates receive input from borrowed bytes.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Returns whether the input is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }
}

/// Progress made by the internal ingress pipeline after bytes are fed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FeedProgress {
    /// Number of bytes consumed by the engine.
    pub consumed: usize,
    /// Number of packets accepted by the engine.
    pub packets: usize,
    /// Number of complete messages produced by the engine.
    pub messages: usize,
    /// Number of engine events queued by the engine.
    pub events: usize,
    /// Whether more bytes are needed to complete the current wire boundary.
    pub needs_more: bool,
}

impl FeedProgress {
    /// Empty progress.
    pub const EMPTY: Self = Self {
        consumed: 0,
        packets: 0,
        messages: 0,
        events: 0,
        needs_more: false,
    };

    /// Returns whether any meaningful progress was made.
    #[must_use]
    pub const fn made_progress(self) -> bool {
        self.consumed > 0 || self.packets > 0 || self.messages > 0 || self.events > 0
    }
}

/// Progress made by a non-blocking receive call.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReceiveProgress {
    /// Bytes read from the link.
    pub read: usize,
    /// Progress made by the internal feed pipeline.
    pub feed: FeedProgress,
}

impl ReceiveProgress {
    /// Returns whether the link had no bytes available.
    #[must_use]
    pub const fn no_data(self) -> bool {
        self.read == 0
    }

    /// Returns whether a complete message was produced.
    #[must_use]
    pub const fn message_complete(self) -> bool {
        self.feed.messages > 0
    }
}

/// Decoded packet input passed into receive-side engine logic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketInput<'a> {
    /// Borrowed protocol packet.
    pub packet: Packet<'a>,
}

impl<'a> PacketInput<'a> {
    /// Creates packet input.
    #[must_use]
    pub const fn new(packet: Packet<'a>) -> Self {
        Self { packet }
    }

    /// Returns the packet number.
    #[must_use]
    pub const fn packet_number(self) -> PacketNumber {
        self.packet.header.packet_number
    }
}

/// First receive-side decision for an incoming packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveAction {
    /// Process the packet and its frames.
    Process {
        /// Packet number accepted for processing.
        packet_number: PacketNumber,
    },
    /// Ignore the packet as duplicate.
    Duplicate {
        /// Duplicate packet number.
        packet_number: PacketNumber,
    },
    /// Drop the packet because it violates receive policy.
    Drop {
        /// Dropped packet number.
        packet_number: PacketNumber,
    },
}

impl ReceiveAction {
    /// Creates a receive action from a deduplication decision.
    #[must_use]
    pub const fn from_dedup(packet_number: PacketNumber, decision: DedupDecision) -> Self {
        match decision {
            DedupDecision::Accept => Self::Process { packet_number },
            DedupDecision::Duplicate => Self::Duplicate { packet_number },
        }
    }
}

/// Receive-side protocol boundary.
pub trait Receiver {
    /// Reads available bytes from a non-blocking link and advances receive-side state.
    fn receive<L>(&mut self, link: &mut L, scratch: &mut [u8]) -> Result<ReceiveProgress>
    where
        L: LinkRead,
    {
        let read = link.read(scratch)?;
        let feed = if read == 0 {
            FeedProgress::EMPTY
        } else {
            self.feed(ReceiveInput::new(&scratch[..read]))?
        };

        Ok(ReceiveProgress { read, feed })
    }

    /// Feeds already-read bytes into the internal ingress pipeline.
    fn feed(&mut self, input: ReceiveInput<'_>) -> Result<FeedProgress>;

    /// Accepts decoded packet input and advances receive-side protocol state.
    fn receive_packet(&mut self, input: PacketInput<'_>) -> Result<ReceiveAction>;
}

#[cfg(test)]
mod tests {
    use srt_core::{Error, PacketNumber, Result};

    use super::{FeedProgress, PacketInput, ReceiveAction, ReceiveInput, Receiver};
    use crate::LinkRead;

    struct MockLink<'a> {
        bytes: &'a [u8],
    }

    impl LinkRead for MockLink<'_> {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            let len = self.bytes.len().min(buf.len());
            buf[..len].copy_from_slice(&self.bytes[..len]);
            self.bytes = &self.bytes[len..];
            Ok(len)
        }
    }

    #[derive(Default)]
    struct DummyReceiver {
        fed: usize,
    }

    impl Receiver for DummyReceiver {
        fn feed(&mut self, input: ReceiveInput<'_>) -> Result<FeedProgress> {
            self.fed += input.bytes.len();
            Ok(FeedProgress {
                consumed: input.bytes.len(),
                packets: 0,
                messages: 0,
                events: 0,
                needs_more: true,
            })
        }

        fn receive_packet(&mut self, _input: PacketInput<'_>) -> Result<ReceiveAction> {
            Err(Error::unsupported())
        }
    }

    #[test]
    fn receive_reads_from_link_and_feeds_bytes() {
        let mut receiver = DummyReceiver::default();
        let mut link = MockLink { bytes: b"abc" };
        let mut scratch = [0; 8];

        let progress = receiver.receive(&mut link, &mut scratch).unwrap();

        assert_eq!(progress.read, 3);
        assert_eq!(progress.feed.consumed, 3);
        assert!(progress.feed.needs_more);
        assert_eq!(receiver.fed, 3);
    }

    #[test]
    fn receive_reports_no_data_without_feeding() {
        let mut receiver = DummyReceiver::default();
        let mut link = MockLink { bytes: b"" };
        let mut scratch = [0; 8];

        let progress = receiver.receive(&mut link, &mut scratch).unwrap();

        assert!(progress.no_data());
        assert_eq!(progress.feed, FeedProgress::EMPTY);
        assert_eq!(receiver.fed, 0);
    }

    #[test]
    fn receive_action_maps_dedup_to_process() {
        let packet_number = PacketNumber::new(9);
        let action =
            ReceiveAction::from_dedup(packet_number, srt_reliability::DedupDecision::Accept);

        assert_eq!(action, ReceiveAction::Process { packet_number });
    }
}
