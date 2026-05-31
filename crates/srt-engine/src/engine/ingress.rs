//! Incoming wire byte handling.

use srt_core::{Error, ErrorKind};
use srt_reliability::{Dedup, DedupDecision};
use srt_wire::{Crc16, StreamDecodeOutcome};

use crate::{
    Engine, EngineOutput, ReceiveReport,
    engine::packet::{PacketBytes, PacketDecode, decode_packet_bytes},
};

impl Engine {
    /// Feeds already-arrived wire bytes into the engine.
    ///
    /// This method never waits for more bytes. It handles the current input and
    /// queues events if a complete message becomes available.
    pub fn receive(&mut self, bytes: &[u8]) -> ReceiveReport {
        self.receive_bytes(bytes)
    }

    fn receive_bytes(&mut self, bytes: &[u8]) -> ReceiveReport {
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
                    report = self.receive_complete_packet(packet.as_bytes());

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

    fn receive_complete_packet(&mut self, bytes: &[u8]) -> ReceiveReport {
        match decode_packet_bytes(bytes) {
            PacketDecode::Data(fragment) => {
                let packet_number = fragment.packet_number;
                if self.queue_ack(packet_number).is_err() {
                    return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                }

                if self.dedup.observe(packet_number) == DedupDecision::Duplicate {
                    return ReceiveReport::Duplicate { packet_number };
                }

                match self.reassembly.observe(fragment) {
                    Ok(Some(message)) => {
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
                self.in_flight.ack(ack.acknowledged);
                ReceiveReport::Ack {
                    packet_number: ack.acknowledged,
                }
            }
            PacketDecode::Malformed => ReceiveReport::Error(Error::malformed()),
        }
    }
}
