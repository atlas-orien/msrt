//! Incoming wire byte handling.

use srt_core::{Error, ErrorKind};
use srt_reliability::{Dedup, DedupDecision};
use srt_wire::{Crc16, StreamDecodeOutcome};

use crate::{
    Engine, EngineOutput, ReceiveReport,
    engine::{
        outgoing::queue_ack,
        packet::{PacketBytes, PacketDecode, decode_packet_bytes},
    },
};

pub(crate) fn receive(engine: &mut Engine, bytes: &[u8]) -> ReceiveReport {
    receive_bytes(engine, bytes)
}

fn receive_bytes(engine: &mut Engine, bytes: &[u8]) -> ReceiveReport {
    let mut input = bytes;
    let mut report = ReceiveReport::Incomplete { needed: None };

    loop {
        let outcome = match engine.ingress.feed(input, &Crc16) {
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
                report = receive_complete_packet(engine, packet.as_bytes());

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

fn receive_complete_packet(engine: &mut Engine, bytes: &[u8]) -> ReceiveReport {
    match decode_packet_bytes(bytes) {
        PacketDecode::Data(fragment) => {
            let packet_number = fragment.packet_number;
            let ack_eliciting = fragment.ack_eliciting;

            if ack_eliciting && queue_ack(engine, packet_number).is_err() {
                return ReceiveReport::Error(Error::new(ErrorKind::Engine));
            }

            if engine.dedup.observe(packet_number) == DedupDecision::Duplicate {
                return ReceiveReport::Duplicate { packet_number };
            }

            match engine.reassembly.observe(fragment, engine.now_ms) {
                Ok(Some(message)) => {
                    if engine.events.push(EngineOutput::Message(message)).is_err() {
                        return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                    }
                    ReceiveReport::Packet { packet_number }
                }
                Ok(None) => ReceiveReport::Packet { packet_number },
                Err(error) => ReceiveReport::Error(error),
            }
        }
        PacketDecode::Ack(ack) => {
            engine.in_flight.ack_frame(ack.frame);
            ReceiveReport::Ack {
                packet_number: ack.frame.largest_acknowledged,
            }
        }
        PacketDecode::Malformed => ReceiveReport::Error(Error::malformed()),
    }
}
