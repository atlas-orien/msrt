//! Incoming wire byte handling.

use crate::core::{Error, ErrorKind};
use crate::integrity::Integrity;
use crate::reliability::{Dedup, DedupDecision};
use crate::wire::{StreamDecodeOutcome, StreamingDecoder};

use crate::engine::{
    EngineConfig, ReceiveReport,
    codec::packet::{PacketBytes, PacketDecode},
    config::MAX_INGRESS_BYTES,
    state::EngineState,
};

/// Incoming byte stream decode state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IngressState {
    decoder: StreamingDecoder<MAX_INGRESS_BYTES>,
}

impl IngressState {
    pub(crate) const fn new() -> Self {
        Self {
            decoder: StreamingDecoder::new(),
        }
    }

    pub(crate) fn feed(
        &mut self,
        bytes: &[u8],
        integrity: &impl Integrity,
    ) -> crate::core::Result<StreamDecodeOutcome<'_>> {
        self.decoder.feed(bytes, integrity)
    }
}

impl EngineState {
    pub(super) fn receive_ingress(&mut self, config: &EngineConfig, bytes: &[u8]) -> ReceiveReport {
        self.receive_bytes(config, bytes)
    }

    fn receive_bytes(&mut self, config: &EngineConfig, bytes: &[u8]) -> ReceiveReport {
        let mut input = bytes;
        let mut report = ReceiveReport::Incomplete { needed: None };

        loop {
            let outcome = match self.ingress.feed(input, &config.integrity) {
                Ok(outcome) => outcome,
                Err(error) => return ReceiveReport::Error(error),
            };
            input = &[];

            match outcome {
                StreamDecodeOutcome::Packet {
                    packet_bytes,
                    consumed: _,
                } => {
                    let packet = match PacketBytes::try_from_slice(packet_bytes) {
                        Ok(packet) => packet,
                        Err(error) => return ReceiveReport::Error(error),
                    };
                    report = self.receive_complete_packet(config, &packet);

                    if matches!(report, ReceiveReport::Error(_)) {
                        return report;
                    }
                }
                StreamDecodeOutcome::NeedMore { additional } => {
                    return match report {
                        ReceiveReport::Incomplete { .. } => {
                            ReceiveReport::Incomplete { needed: additional }
                        }
                        other => other,
                    };
                }
                StreamDecodeOutcome::Noise { skipped } => {
                    report = ReceiveReport::Noise { skipped };
                }
                StreamDecodeOutcome::Corrupted { consumed: _ } => {
                    report = ReceiveReport::Corrupted;
                }
                StreamDecodeOutcome::Resync { skipped } => {
                    report = ReceiveReport::Noise { skipped };
                }
            }
        }
    }

    fn receive_complete_packet(
        &mut self,
        config: &EngineConfig,
        packet: &PacketBytes,
    ) -> ReceiveReport {
        match packet.decode() {
            PacketDecode::Data(fragment) => {
                let packet_index = fragment.header.packet_index();
                let key = fragment.header.key();
                let ack_eliciting = fragment.header.is_ack_eliciting();

                if self.receive.dedup().is_duplicate(key) {
                    if ack_eliciting && self.queue_ack(key).is_err() {
                        return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                    }
                    return ReceiveReport::Duplicate { packet_index };
                }

                let report = match self.reassembly.observe(fragment, self.clock.now_ms()) {
                    Ok(Some(mut message)) => {
                        message.profile = config.channel_profile(message.channel_id);
                        self.message.push(message);
                        ReceiveReport::Packet { packet_index }
                    }
                    Ok(None) => ReceiveReport::Packet { packet_index },
                    Err(error) => ReceiveReport::Error(error),
                };

                if matches!(report, ReceiveReport::Error(_)) {
                    return report;
                }

                match self.receive.dedup().observe_packet(key) {
                    Ok(DedupDecision::Accept) => {}
                    Ok(DedupDecision::Duplicate) => {
                        return ReceiveReport::Duplicate { packet_index };
                    }
                    Err(error) => return ReceiveReport::Error(error),
                }

                if ack_eliciting && self.queue_ack(key).is_err() {
                    return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                }

                report
            }
            PacketDecode::Log(fragment) => {
                let packet_index = fragment.header.packet_index();

                match self.reassembly.observe(fragment, self.clock.now_ms()) {
                    Ok(Some(mut message)) => {
                        message.profile = config.channel_profile(message.channel_id);
                        self.message.push(message);
                        ReceiveReport::Packet { packet_index }
                    }
                    Ok(None) => ReceiveReport::Packet { packet_index },
                    Err(error) => ReceiveReport::Error(error),
                }
            }
            PacketDecode::Ack(ack) => {
                let packet_index = ack.key.packet_index;
                self.recovery.apply_ack(ack.key);
                ReceiveReport::Ack { packet_index }
            }
            PacketDecode::Ping(ping) => {
                let _ = ping;
                if self.queue_pong(config).is_err() {
                    return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                }
                ReceiveReport::Ping
            }
            PacketDecode::Pong(_) => ReceiveReport::Pong,
            PacketDecode::Malformed => ReceiveReport::Corrupted,
        }
    }
}
