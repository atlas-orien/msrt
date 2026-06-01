//! Minimal protocol engine implementation.

pub(crate) mod ack;
pub(crate) mod inflight;
pub(crate) mod ingress;
pub(crate) mod outgoing;
pub(crate) mod packet;
pub(crate) mod queue;
pub(crate) mod reassembly;
pub(crate) mod retransmit;

use srt_core::{ChannelId, Error, MessageId, PacketNumber, Result};
use srt_reliability::{ChannelReliability, PacketDedup, ReliabilityMode};
use srt_wire::StreamingDecoder;

use crate::{
    ChannelProfile, ChannelSpec, EngineConfig, MAX_CHANNEL_POLICIES, MAX_CHANNEL_SPECS,
    MAX_IN_FLIGHT_PACKETS, MAX_INGRESS_BYTES, MAX_MESSAGE_BYTES, MAX_WIRE_BYTES,
    engine::{
        ack::AckRanges, inflight::InFlightPackets, queue::EventQueue, reassembly::ReassemblyBuffer,
    },
};

/// Minimal non-blocking SRT protocol engine.
///
/// The engine owns protocol state. It splits outgoing messages into packet
/// write events, accepts incoming wire bytes, and emits complete messages once
/// reassembly succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Engine {
    pub(crate) next_packet_number: PacketNumber,
    pub(crate) next_message_id: MessageId,
    pub(crate) fragment_bytes: usize,
    pub(crate) max_retransmit_attempts: u8,
    pub(crate) retransmit_timeout_ms: u64,
    pub(crate) reassembly_timeout_ms: u64,
    pub(crate) channel_specs: [Option<ChannelSpec>; MAX_CHANNEL_SPECS],
    pub(crate) channel_policies: [Option<ChannelReliability>; MAX_CHANNEL_POLICIES],
    pub(crate) now_ms: u64,
    pub(crate) events: EventQueue,
    pub(crate) in_flight: InFlightPackets,
    pub(crate) ack_ranges: AckRanges,
    pub(crate) ingress: StreamingDecoder<MAX_INGRESS_BYTES>,
    pub(crate) dedup: PacketDedup<MAX_IN_FLIGHT_PACKETS>,
    pub(crate) reassembly: ReassemblyBuffer,
}

impl Engine {
    /// Creates an engine.
    #[must_use]
    pub const fn new(config: EngineConfig) -> Self {
        Self {
            next_packet_number: config.initial_packet_number,
            next_message_id: config.initial_message_id,
            fragment_bytes: config.fragment_bytes,
            max_retransmit_attempts: config.max_retransmit_attempts,
            retransmit_timeout_ms: config.retransmit_timeout_ms,
            reassembly_timeout_ms: config.reassembly_timeout_ms,
            channel_specs: config.channel_specs,
            channel_policies: config.channel_policies,
            now_ms: 0,
            events: EventQueue::new(),
            in_flight: InFlightPackets::new(),
            ack_ranges: AckRanges::new(),
            ingress: StreamingDecoder::new(),
            dedup: PacketDedup::new(),
            reassembly: ReassemblyBuffer::new(),
        }
    }

    /// Polls one queued engine output event.
    pub fn poll_event(&mut self) -> Option<EngineOutput> {
        self.events.pop()
    }

    /// Queues a complete message for non-blocking protocol transmission.
    ///
    /// The caller submits the complete message once. The engine splits it into
    /// packet-sized write events internally.
    pub fn send(&mut self, message: &[u8]) -> Result<MessageId> {
        self.send_on(ChannelId::DEFAULT, message)
    }

    /// Queues a complete message on a logical channel.
    ///
    /// This is the channel-aware form of [`Engine::send`].
    pub fn send_on(&mut self, channel_id: ChannelId, message: &[u8]) -> Result<MessageId> {
        outgoing::send_on(self, channel_id, message)
    }

    /// Feeds already-arrived wire bytes into the engine.
    ///
    /// This method never waits for more bytes. It handles the current input and
    /// queues events if a complete message becomes available.
    pub fn receive(&mut self, bytes: &[u8]) -> ReceiveReport {
        ingress::receive(self, bytes)
    }

    /// Advances time-driven protocol work.
    pub fn tick(&mut self, now_ms: u64) {
        retransmit::tick(self, now_ms);
    }

    /// Returns the next packet number that will be assigned.
    #[must_use]
    pub const fn next_packet_number(&self) -> PacketNumber {
        self.next_packet_number
    }

    pub(crate) fn reliability_mode(&self, channel_id: ChannelId) -> ReliabilityMode {
        let mut spec_index = 0;

        while spec_index < MAX_CHANNEL_SPECS {
            if let Some(spec) = self.channel_specs[spec_index]
                && spec.channel_id.get() == channel_id.get()
            {
                return spec.reliability_mode;
            }
            spec_index += 1;
        }

        if channel_id.is_log() {
            return ReliabilityMode::BestEffort;
        }

        let mut index = 0;

        while index < MAX_CHANNEL_POLICIES {
            if let Some(policy) = self.channel_policies[index]
                && policy.channel_id.get() == channel_id.get()
            {
                return policy.mode;
            }
            index += 1;
        }

        ReliabilityMode::Reliable
    }

    pub(crate) fn channel_profile(&self, channel_id: ChannelId) -> ChannelProfile {
        let mut index = 0;

        while index < MAX_CHANNEL_SPECS {
            if let Some(spec) = self.channel_specs[index]
                && spec.channel_id.get() == channel_id.get()
            {
                return spec.profile;
            }
            index += 1;
        }

        ChannelProfile::default_for(channel_id)
    }
}

/// Events produced by the minimal engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineOutput {
    /// Protocol bytes should be written to the serial link.
    Write(WriteEvent),
    /// A complete application message has been reassembled.
    Message(MessageEvent),
    /// A message could not be sent reliably.
    SendFailed(SendFailedEvent),
}

/// A non-blocking write request produced by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriteEvent {
    /// Packet number assigned to this write.
    pub packet_number: PacketNumber,
    /// Fixed storage containing encoded wire bytes.
    pub bytes: [u8; MAX_WIRE_BYTES],
    /// Number of valid bytes in `bytes`.
    pub len: usize,
}

impl WriteEvent {
    /// Returns the valid encoded wire bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}

/// A complete message delivered by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageEvent {
    /// Logical channel that carried the message.
    pub channel_id: ChannelId,
    /// Protocol-level purpose associated with the channel.
    pub profile: ChannelProfile,
    /// Message identifier scoped to this engine.
    pub message_id: MessageId,
    /// Fixed storage containing complete message bytes.
    pub bytes: [u8; MAX_MESSAGE_BYTES],
    /// Number of valid message bytes in `bytes`.
    pub len: usize,
}

impl MessageEvent {
    /// Returns the valid message bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}

/// A reliable send failure produced by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendFailedEvent {
    /// Logical channel whose message failed.
    pub channel_id: ChannelId,
    /// Message identifier that failed.
    pub message_id: MessageId,
    /// Failure reason.
    pub reason: SendFailureReason,
}

/// Reason a reliable send failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendFailureReason {
    /// At least one packet for the message reached the configured retransmission attempt limit.
    RetryLimitReached,
}

/// Result of `Engine::receive`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveReport {
    /// A packet envelope was accepted.
    Packet {
        /// Packet number decoded from the envelope.
        packet_number: PacketNumber,
    },
    /// A duplicate packet envelope was acknowledged but not processed again.
    Duplicate {
        /// Duplicate packet number.
        packet_number: PacketNumber,
    },
    /// An ACK packet was accepted.
    Ack {
        /// Packet number acknowledged by the peer.
        packet_number: PacketNumber,
    },
    /// The input did not contain a valid magic prefix at offset zero.
    Noise {
        /// Number of bytes treated as noise.
        skipped: usize,
    },
    /// The envelope checksum failed.
    Corrupted,
    /// The envelope is incomplete.
    Incomplete {
        /// Number of bytes required if known.
        needed: Option<usize>,
    },
    /// The packet was valid but could not be applied to engine state.
    Error(Error),
}

#[cfg(test)]
mod tests {
    use super::{Engine, EngineConfig, EngineOutput, ReceiveReport};
    use crate::{ChannelProfile, ChannelSpec};
    use srt_reliability::ChannelReliability;

    #[test]
    fn engine_sends_one_message_as_multiple_write_events() {
        let mut engine = Engine::new(EngineConfig {
            fragment_bytes: 5,
            ..EngineConfig::default()
        });

        let message_id = engine.send(b"hello srt testing").unwrap();
        let mut writes = 0;

        while let Some(event) = engine.poll_event() {
            match event {
                EngineOutput::Write(_) => writes += 1,
                EngineOutput::Message(_) => panic!("sender should not receive its own message"),
                EngineOutput::SendFailed(failed) => {
                    panic!("sender should not fail in this test: {failed:?}");
                }
            }
        }

        assert_eq!(message_id.get(), 0);
        assert_eq!(writes, 4);
    }

    #[test]
    fn engine_receives_fragments_as_one_message_event() {
        let mut a = Engine::new(EngineConfig {
            fragment_bytes: 5,
            ..EngineConfig::default()
        });
        let mut b = Engine::new(EngineConfig::default());

        a.send(b"hello srt testing").unwrap();

        while let Some(event) = a.poll_event() {
            let EngineOutput::Write(write) = event else {
                continue;
            };

            assert!(matches!(
                b.receive(write.as_bytes()),
                ReceiveReport::Packet { .. } | ReceiveReport::Ack { .. }
            ));
        }

        while let Some(event) = b.poll_event() {
            if let EngineOutput::Message(message) = event {
                assert_eq!(message.as_bytes(), b"hello srt testing");
                return;
            }
        }

        panic!("receiver should emit a complete message");
    }

    #[test]
    fn engine_reassembles_interleaved_messages() {
        let mut a = Engine::new(EngineConfig {
            fragment_bytes: 2,
            ..EngineConfig::default()
        });
        let mut b = Engine::new(EngineConfig::default());
        let mut writes = [None; 4];
        let mut write_count = 0;

        a.send(b"abcd").unwrap();
        a.send(b"wxyz").unwrap();

        while let Some(event) = a.poll_event() {
            let EngineOutput::Write(write) = event else {
                continue;
            };
            writes[write_count] = Some(write);
            write_count += 1;
        }

        assert_eq!(write_count, 4);

        for index in [0, 2, 1, 3] {
            let write = writes[index].expect("write should be captured");
            assert!(matches!(
                b.receive(write.as_bytes()),
                ReceiveReport::Packet { .. }
            ));
        }

        assert_message(&mut b, b"abcd");
        assert_message(&mut b, b"wxyz");
    }

    #[test]
    fn engine_rejects_fragment_when_reassembly_budget_is_full() {
        let mut a = Engine::new(EngineConfig {
            fragment_bytes: 2,
            ..EngineConfig::default()
        });
        let mut b = Engine::new(EngineConfig::default());
        let writes = first_fragments_for_five_messages(&mut a);

        for write in &writes[..crate::MAX_REASSEMBLY_MESSAGES] {
            assert!(matches!(
                b.receive(write.expect("fragment").as_bytes()),
                ReceiveReport::Packet { .. }
            ));
        }

        assert!(matches!(
            b.receive(
                writes[crate::MAX_REASSEMBLY_MESSAGES]
                    .expect("extra fragment")
                    .as_bytes()
            ),
            ReceiveReport::Error(_)
        ));
    }

    #[test]
    fn engine_reassembly_timeout_releases_slot() {
        let mut a = Engine::new(EngineConfig {
            fragment_bytes: 2,
            ..EngineConfig::default()
        });
        let mut b = Engine::new(EngineConfig {
            reassembly_timeout_ms: 10,
            ..EngineConfig::default()
        });
        let writes = first_fragments_for_five_messages(&mut a);

        for write in &writes[..crate::MAX_REASSEMBLY_MESSAGES] {
            assert!(matches!(
                b.receive(write.expect("fragment").as_bytes()),
                ReceiveReport::Packet { .. }
            ));
        }

        b.tick(10);

        assert!(matches!(
            b.receive(
                writes[crate::MAX_REASSEMBLY_MESSAGES]
                    .expect("extra fragment")
                    .as_bytes()
            ),
            ReceiveReport::Packet { .. }
        ));
    }

    #[test]
    fn engine_send_on_uses_channel_id() {
        let mut a = Engine::new(EngineConfig::default());
        let mut b = Engine::new(EngineConfig::default());
        let channel_id = srt_core::ChannelId::new(7);

        a.send_on(channel_id, b"hello").unwrap();

        let write = next_write(&mut a);
        let bytes = write.as_bytes();

        assert_eq!(bytes[15], channel_id.get());
        assert!(matches!(
            b.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));

        let message = next_message(&mut b);

        assert_eq!(message.channel_id, channel_id);
        assert_eq!(message.as_bytes(), b"hello");
    }

    #[test]
    fn engine_ack_range_clears_multiple_in_flight_packets() {
        let mut a = Engine::new(EngineConfig {
            fragment_bytes: 2,
            ..EngineConfig::default()
        });
        let mut b = Engine::new(EngineConfig::default());
        let mut last_ack = None;

        a.send(b"abcdef").unwrap();

        while let Some(event) = a.poll_event() {
            let EngineOutput::Write(write) = event else {
                continue;
            };

            assert!(matches!(
                b.receive(write.as_bytes()),
                ReceiveReport::Packet { .. }
            ));
        }

        while let Some(event) = b.poll_event() {
            let EngineOutput::Write(write) = event else {
                continue;
            };

            last_ack = Some(write);
        }

        let last_ack = last_ack.expect("receiver should emit ACK range");

        assert!(matches!(
            a.receive(last_ack.as_bytes()),
            ReceiveReport::Ack { .. }
        ));

        a.tick(1);

        assert!(a.poll_event().is_none());
    }

    #[test]
    fn engine_ack_range_gap_retransmits_only_missing_packet() {
        let mut engine = Engine::new(EngineConfig {
            fragment_bytes: 2,
            ..EngineConfig::default()
        });

        engine.send(b"abcdefgh").unwrap();

        for expected in 0..4 {
            assert_eq!(next_write(&mut engine).packet_number.get(), expected);
        }

        let ack = ack_packet_for_ranges(
            &[
                (
                    srt_core::PacketNumber::new(0),
                    srt_core::PacketNumber::new(0),
                ),
                (
                    srt_core::PacketNumber::new(2),
                    srt_core::PacketNumber::new(3),
                ),
            ],
            srt_core::PacketNumber::new(100),
        );

        assert!(matches!(
            engine.receive(ack.as_bytes()),
            ReceiveReport::Ack { .. }
        ));

        engine.tick(1);

        assert_eq!(next_write(&mut engine).packet_number.get(), 1);
        assert!(engine.poll_event().is_none());
    }

    #[test]
    fn engine_tick_waits_for_retransmit_timeout() {
        let mut engine = Engine::new(EngineConfig {
            retransmit_timeout_ms: 10,
            ..EngineConfig::default()
        });

        engine.send(b"hello").unwrap();
        let first = next_write(&mut engine);

        engine.tick(9);
        assert!(engine.poll_event().is_none());

        engine.tick(10);
        assert_eq!(next_write(&mut engine).packet_number, first.packet_number);

        engine.tick(19);
        assert!(engine.poll_event().is_none());

        engine.tick(20);
        assert_eq!(next_write(&mut engine).packet_number, first.packet_number);
    }

    #[test]
    fn engine_receives_half_packet() {
        let mut a = Engine::new(EngineConfig::default());
        let mut b = Engine::new(EngineConfig::default());

        a.send(b"hello").unwrap();
        let write = next_write(&mut a);
        let split = 3;

        assert_eq!(
            b.receive(&write.as_bytes()[..split]),
            ReceiveReport::Incomplete {
                needed: Some(srt_wire::WIRE_HEADER_LEN - split)
            }
        );
        assert!(matches!(
            b.receive(&write.as_bytes()[split..]),
            ReceiveReport::Packet { .. }
        ));
        assert_message(&mut b, b"hello");
    }

    #[test]
    fn engine_receives_sticky_packets_and_multiple_packets_per_receive() {
        let mut a = Engine::new(EngineConfig {
            fragment_bytes: 5,
            ..EngineConfig::default()
        });
        let mut b = Engine::new(EngineConfig::default());
        let mut bytes = [0; crate::MAX_WIRE_BYTES * 4];
        let mut len = 0;

        a.send(b"hello srt testing").unwrap();

        while let Some(event) = a.poll_event() {
            let EngineOutput::Write(write) = event else {
                continue;
            };
            let end = len + write.as_bytes().len();
            bytes[len..end].copy_from_slice(write.as_bytes());
            len = end;
        }

        assert!(matches!(
            b.receive(&bytes[..len]),
            ReceiveReport::Packet { .. }
        ));
        assert_message(&mut b, b"hello srt testing");
    }

    #[test]
    fn engine_acknowledges_duplicate_without_delivering_twice() {
        let mut a = Engine::new(EngineConfig::default());
        let mut b = Engine::new(EngineConfig::default());

        a.send(b"hello").unwrap();
        let write = next_write(&mut a);

        assert!(matches!(
            b.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));
        assert_message(&mut b, b"hello");
        assert!(matches!(
            b.receive(write.as_bytes()),
            ReceiveReport::Duplicate { .. }
        ));

        let mut duplicate_messages = 0;
        while let Some(event) = b.poll_event() {
            if matches!(event, EngineOutput::Message(_)) {
                duplicate_messages += 1;
            }
        }

        assert_eq!(duplicate_messages, 0);
    }

    #[test]
    fn engine_uses_greedy_fragmentation() {
        let mut engine = Engine::new(EngineConfig {
            fragment_bytes: 10,
            ..EngineConfig::default()
        });
        let mut fragment_lengths = [0; 2];
        let mut fragment_count = 0;

        engine.send(b"hello world").unwrap();

        while let Some(event) = engine.poll_event() {
            let EngineOutput::Write(write) = event else {
                continue;
            };

            fragment_lengths[fragment_count] = fragment_len_from_wire(write.as_bytes());
            fragment_count += 1;
        }

        assert_eq!(&fragment_lengths[..fragment_count], &[10, 1]);
    }

    #[test]
    fn engine_encodes_v1_draft_packet_and_frame_headers() {
        let mut engine = Engine::new(EngineConfig::default());

        engine.send(b"hello").unwrap();

        let write = next_write(&mut engine);
        let bytes = write.as_bytes();

        assert_eq!(&bytes[..2], &srt_wire::EnvelopeMagic::SRT.bytes());
        assert_eq!(bytes[8], srt_core::PacketType::Data.code());
        assert_eq!(bytes[9], srt_core::Flags::ACK_ELICITING.bits());
        assert_eq!(
            u32::from_le_bytes(bytes[10..14].try_into().unwrap()),
            write.packet_number.get()
        );
        assert_eq!(bytes[14], srt_core::FrameKind::Message.code());
        assert_eq!(bytes[15], srt_core::ChannelId::DEFAULT.get());
        assert_eq!(
            bytes[24],
            srt_core::MessageFlags::FIRST.bits() | srt_core::MessageFlags::LAST.bits()
        );
    }

    #[test]
    fn engine_best_effort_channel_does_not_track_in_flight() {
        let channel_id = srt_core::ChannelId::new(9);
        let mut engine = Engine::new(EngineConfig {
            channel_policies: [
                Some(ChannelReliability::best_effort(channel_id)),
                None,
                None,
                None,
            ],
            ..EngineConfig::default()
        });

        engine.send_on(channel_id, b"hello best effort").unwrap();

        while let Some(event) = engine.poll_event() {
            let EngineOutput::Write(_) = event else {
                panic!("best-effort send should only produce writes before tick");
            };
        }

        assert_eq!(engine.in_flight.packets().count(), 0);

        engine.tick(1);

        assert!(engine.poll_event().is_none());
    }

    #[test]
    fn engine_best_effort_packet_is_not_ack_eliciting() {
        let channel_id = srt_core::ChannelId::new(9);
        let mut engine = Engine::new(EngineConfig {
            channel_policies: [
                Some(ChannelReliability::best_effort(channel_id)),
                None,
                None,
                None,
            ],
            ..EngineConfig::default()
        });

        engine.send_on(channel_id, b"hello").unwrap();

        let write = next_write(&mut engine);

        assert_eq!(write.as_bytes()[9], srt_core::Flags::EMPTY.bits());
    }

    #[test]
    fn engine_receives_best_effort_without_ack() {
        let channel_id = srt_core::ChannelId::new(9);
        let mut sender = Engine::new(EngineConfig {
            channel_policies: [
                Some(ChannelReliability::best_effort(channel_id)),
                None,
                None,
                None,
            ],
            ..EngineConfig::default()
        });
        let mut receiver = Engine::new(EngineConfig::default());

        sender.send_on(channel_id, b"hello best effort").unwrap();

        let write = next_write(&mut sender);

        assert!(matches!(
            receiver.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));

        let Some(event) = receiver.poll_event() else {
            panic!("receiver should emit the complete best-effort message");
        };
        let EngineOutput::Message(message) = event else {
            panic!("best-effort packet should not emit ACK before message");
        };

        assert_eq!(message.channel_id, channel_id);
        assert_eq!(message.profile, ChannelProfile::Data);
        assert_eq!(message.as_bytes(), b"hello best effort");
        assert!(receiver.poll_event().is_none());
    }

    #[test]
    fn engine_send_uses_default_application_channel() {
        let mut engine = Engine::new(EngineConfig::default());

        engine.send(b"hello default").unwrap();

        let write = next_write(&mut engine);
        let bytes = write.as_bytes();

        assert_eq!(bytes[15], srt_core::ChannelId::DEFAULT.get());
    }

    #[test]
    fn engine_log_channel_defaults_to_best_effort_and_log_profile() {
        let mut sender = Engine::new(EngineConfig::default());
        let mut receiver = Engine::new(EngineConfig::default());

        sender
            .send_on(srt_core::ChannelId::LOG, b"log line")
            .unwrap();

        let write = next_write(&mut sender);

        assert_eq!(write.as_bytes()[9], srt_core::Flags::EMPTY.bits());
        assert_eq!(sender.in_flight.packets().count(), 0);

        assert!(matches!(
            receiver.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));

        let message = next_message(&mut receiver);

        assert_eq!(message.channel_id, srt_core::ChannelId::LOG);
        assert_eq!(message.profile, ChannelProfile::Log);
        assert_eq!(message.as_bytes(), b"log line");
        assert!(receiver.poll_event().is_none());
    }

    #[test]
    fn engine_channel_spec_overrides_profile_and_reliability() {
        let channel_id = srt_core::ChannelId::new(16);
        let mut sender = Engine::new(EngineConfig {
            channel_specs: [Some(ChannelSpec::log(channel_id)), None, None, None],
            ..EngineConfig::default()
        });
        let mut receiver = Engine::new(EngineConfig {
            channel_specs: [Some(ChannelSpec::log(channel_id)), None, None, None],
            ..EngineConfig::default()
        });

        sender.send_on(channel_id, b"adapter log").unwrap();

        let write = next_write(&mut sender);

        assert_eq!(write.as_bytes()[9], srt_core::Flags::EMPTY.bits());
        assert_eq!(sender.in_flight.packets().count(), 0);

        assert!(matches!(
            receiver.receive(write.as_bytes()),
            ReceiveReport::Packet { .. }
        ));

        let message = next_message(&mut receiver);

        assert_eq!(message.channel_id, channel_id);
        assert_eq!(message.profile, ChannelProfile::Log);
        assert_eq!(message.as_bytes(), b"adapter log");
    }

    #[test]
    fn engine_reports_send_failed_after_retry_limit() {
        let mut engine = Engine::new(EngineConfig {
            max_retransmit_attempts: 1,
            ..EngineConfig::default()
        });

        let message_id = engine.send(b"hello").unwrap();
        let first = next_write(&mut engine);

        assert_eq!(first.packet_number.get(), 0);

        engine.tick(1);
        let retry = next_write(&mut engine);

        assert_eq!(retry.packet_number, first.packet_number);

        engine.tick(2);

        let Some(EngineOutput::SendFailed(failed)) = engine.poll_event() else {
            panic!("engine should report send failure");
        };

        assert_eq!(failed.message_id, message_id);
        assert_eq!(failed.channel_id, srt_core::ChannelId::DEFAULT);
        assert_eq!(failed.reason, super::SendFailureReason::RetryLimitReached);
    }

    #[test]
    fn engine_send_failed_is_message_scoped() {
        let mut engine = Engine::new(EngineConfig {
            fragment_bytes: 2,
            max_retransmit_attempts: 1,
            ..EngineConfig::default()
        });

        let message_id = engine.send(b"hello").unwrap();
        let first = next_write(&mut engine);
        let second = next_write(&mut engine);
        let third = next_write(&mut engine);

        assert_eq!(first.packet_number.get(), 0);
        assert_eq!(second.packet_number.get(), 1);
        assert_eq!(third.packet_number.get(), 2);

        engine.tick(1);
        assert_eq!(next_write(&mut engine).packet_number, first.packet_number);
        assert_eq!(next_write(&mut engine).packet_number, second.packet_number);
        assert_eq!(next_write(&mut engine).packet_number, third.packet_number);

        engine.tick(2);

        let Some(EngineOutput::SendFailed(failed)) = engine.poll_event() else {
            panic!("engine should report message failure");
        };

        assert_eq!(failed.message_id, message_id);
        assert_eq!(failed.channel_id, srt_core::ChannelId::DEFAULT);
        assert_eq!(failed.reason, super::SendFailureReason::RetryLimitReached);
        assert_eq!(engine.in_flight.packets().count(), 0);
        assert!(engine.poll_event().is_none());
    }

    #[test]
    fn engine_send_failed_suppresses_same_tick_message_retransmits() {
        let mut engine = Engine::new(EngineConfig {
            fragment_bytes: 2,
            max_retransmit_attempts: 1,
            ..EngineConfig::default()
        });

        let message_id = engine.send(b"hello").unwrap();
        let first = next_write(&mut engine);
        let second = next_write(&mut engine);
        let third = next_write(&mut engine);

        assert_eq!(first.packet_number.get(), 0);
        assert_eq!(second.packet_number.get(), 1);
        assert_eq!(third.packet_number.get(), 2);

        engine.tick(1);
        assert_eq!(next_write(&mut engine).packet_number, first.packet_number);
        assert_eq!(next_write(&mut engine).packet_number, second.packet_number);
        assert_eq!(next_write(&mut engine).packet_number, third.packet_number);

        let ack = ack_packet_for_ranges(
            &[(
                srt_core::PacketNumber::new(0),
                srt_core::PacketNumber::new(0),
            )],
            srt_core::PacketNumber::new(100),
        );

        assert!(matches!(
            engine.receive(ack.as_bytes()),
            ReceiveReport::Ack { .. }
        ));

        engine.tick(2);

        let Some(EngineOutput::SendFailed(failed)) = engine.poll_event() else {
            panic!("engine should report message failure");
        };

        assert_eq!(failed.message_id, message_id);
        assert_eq!(failed.channel_id, srt_core::ChannelId::DEFAULT);
        assert_eq!(failed.reason, super::SendFailureReason::RetryLimitReached);
        assert_eq!(engine.in_flight.packets().count(), 0);
        assert!(engine.poll_event().is_none());
    }

    fn fragment_len_from_wire(bytes: &[u8]) -> usize {
        let packet_len = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;

        packet_len - crate::layout::PACKET_HEADER_LEN - crate::layout::MESSAGE_FRAME_HEADER_LEN
    }

    fn next_write(engine: &mut Engine) -> super::WriteEvent {
        let Some(EngineOutput::Write(write)) = engine.poll_event() else {
            panic!("engine should produce a write event");
        };

        write
    }

    fn ack_packet_for_ranges(
        ranges: &[(srt_core::PacketNumber, srt_core::PacketNumber)],
        packet_number: srt_core::PacketNumber,
    ) -> super::WriteEvent {
        let mut bytes = [0; crate::MAX_WIRE_BYTES];
        let packet_len = crate::layout::ACK_PACKET_LEN as u16;
        let total_len = srt_wire::WIRE_HEADER_LEN + usize::from(packet_len) + 2;

        bytes[..2].copy_from_slice(&srt_wire::EnvelopeMagic::SRT.bytes());
        bytes[2] = 1;
        bytes[3] = srt_wire::WIRE_HEADER_LEN as u8;
        bytes[4..6].copy_from_slice(&packet_len.to_le_bytes());
        bytes[6] = srt_wire::WireFlags::CHECKSUM_PRESENT.bits();
        bytes[7] = 0;
        bytes[8] = srt_core::PacketType::Ack.code();
        bytes[9] = 0;
        bytes[10..14].copy_from_slice(&packet_number.get().to_le_bytes());
        bytes[14] = srt_core::FrameKind::Ack.code();
        bytes[15..19].copy_from_slice(&ranges[ranges.len() - 1].1.get().to_le_bytes());
        bytes[19] = ranges.len() as u8;

        let mut offset = 20;
        for (start, end) in ranges {
            bytes[offset..offset + 4].copy_from_slice(&start.get().to_le_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&end.get().to_le_bytes());
            offset += 8;
        }

        let checksum = srt_wire::Checksum::calculate(&srt_wire::Crc16, &bytes[..total_len - 2]);
        bytes[total_len - 2..total_len].copy_from_slice(&checksum.to_le_bytes());

        super::WriteEvent {
            packet_number,
            bytes,
            len: total_len,
        }
    }

    fn first_fragments_for_five_messages(engine: &mut Engine) -> [Option<super::WriteEvent>; 5] {
        let mut fragments = [None; 5];
        let mut write_index = 0;

        for message in [b"aa00", b"bb11", b"cc22", b"dd33", b"ee44"] {
            engine.send(message).unwrap();
        }

        while let Some(event) = engine.poll_event() {
            let EngineOutput::Write(write) = event else {
                continue;
            };

            if write_index % 2 == 0 {
                fragments[write_index / 2] = Some(write);
            }
            write_index += 1;
        }

        fragments
    }

    fn assert_message(engine: &mut Engine, expected: &[u8]) {
        let message = next_message(engine);

        assert_eq!(message.as_bytes(), expected);
    }

    fn next_message(engine: &mut Engine) -> super::MessageEvent {
        while let Some(event) = engine.poll_event() {
            if let EngineOutput::Message(message) = event {
                return message;
            }
        }

        panic!("engine should produce a complete message");
    }
}
