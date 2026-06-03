//! Incoming wire byte handling.

use crate::core::{Error, ErrorKind};
use crate::reliability::{Dedup, DedupDecision};
use crate::wire::{Crc16, StreamDecodeOutcome};

use crate::engine::{
    EngineConfig, ReceiveReport,
    machine::{
        EngineOutput, Machine,
        packet::{PacketBytes, PacketDecode, decode_packet_bytes},
    },
};

impl Machine {
    pub(crate) fn receive(&mut self, config: &EngineConfig, bytes: &[u8]) -> ReceiveReport {
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
                    report = self.receive_complete_packet(config, packet.as_bytes());

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

    fn receive_complete_packet(&mut self, config: &EngineConfig, bytes: &[u8]) -> ReceiveReport {
        match decode_packet_bytes(bytes) {
            PacketDecode::Data(fragment) => {
                let packet_number = fragment.packet_number;
                let ack_eliciting = fragment.ack_eliciting;

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
                self.in_flight.ack_frame(ack.frame);
                ReceiveReport::Ack {
                    packet_number: ack.frame.largest_acknowledged,
                }
            }
            PacketDecode::Malformed => ReceiveReport::Error(Error::malformed()),
        }
    }
}
