//! Tick-driven retransmission.

use crate::{
    Engine, EngineOutput, MAX_IN_FLIGHT_PACKETS, SendFailedEvent, SendFailureReason, WriteEvent,
};

impl Engine {
    /// Advances time-driven protocol work.
    ///
    /// The MVP engine keeps this as a boundary for future ACK timeout and
    /// retransmission logic.
    pub fn tick(&mut self, _now_ms: u64) {
        let mut retransmits = [None; MAX_IN_FLIGHT_PACKETS];
        let mut retransmit_len = 0;
        let mut failures = [None; MAX_IN_FLIGHT_PACKETS];
        let mut failure_len = 0;
        let mut failed_messages = [None; MAX_IN_FLIGHT_PACKETS];
        let mut failed_message_len = 0;

        for packet in self.in_flight.packets() {
            if packet.attempts >= self.max_retransmit_attempts {
                let already_failed = failed_messages[..failed_message_len]
                    .iter()
                    .flatten()
                    .any(|message_id| *message_id == packet.message_id);

                if !already_failed && failure_len < failures.len() {
                    failures[failure_len] = Some(*packet);
                    failure_len += 1;
                    failed_messages[failed_message_len] = Some(packet.message_id);
                    failed_message_len += 1;
                }
            } else if retransmit_len < retransmits.len() {
                retransmits[retransmit_len] = Some(*packet);
                retransmit_len += 1;
            }
        }

        for packet in failures[..failure_len].iter().flatten() {
            self.in_flight.remove_message(packet.message_id);
            let _ = self.events.push(EngineOutput::SendFailed(SendFailedEvent {
                message_id: packet.message_id,
                reason: SendFailureReason::RetryLimitReached,
            }));
        }

        for packet in retransmits[..retransmit_len].iter().flatten() {
            self.in_flight.note_retransmit(packet.packet_number);
            let _ = self.events.push(EngineOutput::Write(WriteEvent {
                packet_number: packet.packet_number,
                bytes: packet.bytes,
                len: packet.len,
            }));
        }
    }
}
