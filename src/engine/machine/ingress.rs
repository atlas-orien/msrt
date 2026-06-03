//! Incoming wire byte handling.

use crate::core::{ChannelId, Error, ErrorKind};
use crate::reliability::{Dedup, DedupDecision};
use crate::wire::{Crc16, StreamDecodeOutcome};

use crate::engine::{
    EngineConfig, ReceiveReport,
    machine::{
        EngineOutput, Machine,
        packet::{PacketBytes, PacketDecode},
    },
};

impl Machine {
    pub(super) fn receive_ingress(&mut self, config: &EngineConfig, bytes: &[u8]) -> ReceiveReport {
        self.receive_bytes(config, bytes)
    }

    fn receive_bytes(&mut self, config: &EngineConfig, bytes: &[u8]) -> ReceiveReport {
        let mut input = bytes;
        let mut report = ReceiveReport::Incomplete { needed: None };

        loop {
            let outcome = match self.ingress.feed(input, &Crc16) {
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
                let packet_number = fragment.header.packet_number;
                let ack_eliciting = fragment.header.is_ack_eliciting();

                if ack_eliciting && self.queue_ack(packet_number).is_err() {
                    return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                }

                if self.dedup.observe(packet_number) == DedupDecision::Duplicate {
                    return ReceiveReport::Duplicate { packet_number };
                }

                match self.reassembly.observe(fragment, self.now_ms) {
                    Ok(Some(mut message)) => {
                        message.profile = config.channel_profile(message.channel_id);
                        if self.events.push(EngineOutput::Message(message)).is_err() {
                            return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                        }
                        ReceiveReport::Packet { packet_number }
                    }
                    Ok(None) => ReceiveReport::Packet { packet_number },
                    Err(error) => ReceiveReport::Error(error),
                }
            }
            PacketDecode::Ack(ack) => {
                self.in_flight.apply_ack(ack.ack);
                ReceiveReport::Ack {
                    packet_number: ack.ack.largest_acknowledged,
                }
            }
            PacketDecode::Ping(ping) => {
                let packet_number = ping.header.packet_number;
                if self.queue_pong(ping.header.message_id).is_err() {
                    return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                }
                ReceiveReport::Ping {
                    packet_number,
                    message_id: ping.header.message_id,
                }
            }
            PacketDecode::Pong(pong) => {
                self.in_flight
                    .remove_message(ChannelId::LIVENESS, pong.header.message_id);
                ReceiveReport::Pong {
                    packet_number: pong.header.packet_number,
                    message_id: pong.header.message_id,
                }
            }
            PacketDecode::Malformed => ReceiveReport::Error(Error::malformed()),
        }
    }
}
