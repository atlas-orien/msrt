//! Tick-driven retransmission.

use crate::engine::{
    EngineConfig, SendFailedEvent, SendFailureReason,
    config::MAX_IN_FLIGHT_PACKETS,
    machine::{EngineOutput, Machine, WriteEvent},
};

impl Machine {
    pub(super) fn tick_retransmit(&mut self, config: &EngineConfig, now_ms: u64) {
        self.now_ms = now_ms;
        self.reassembly.expire(now_ms, config.reassembly_timeout_ms);

        let mut retransmits = [None; MAX_IN_FLIGHT_PACKETS];
        let mut retransmit_len = 0;
        let mut failures = [None; MAX_IN_FLIGHT_PACKETS];
        let mut failure_len = 0;
        let mut failed_messages = [None; MAX_IN_FLIGHT_PACKETS];
        let mut failed_message_len = 0;

        for packet in self.in_flight.packets() {
            if now_ms.saturating_sub(packet.last_sent_ms) < config.retransmit_timeout_ms {
                continue;
            }

            if packet.attempts >= config.max_retransmit_attempts {
                let already_failed = failed_messages[..failed_message_len]
                    .iter()
                    .flatten()
                    .any(|key| *key == (packet.channel_id, packet.message_id));

                if !already_failed && failure_len < failures.len() {
                    failures[failure_len] = Some(*packet);
                    failure_len += 1;
                    failed_messages[failed_message_len] =
                        Some((packet.channel_id, packet.message_id));
                    failed_message_len += 1;
                }
            } else if retransmit_len < retransmits.len() {
                retransmits[retransmit_len] = Some(*packet);
                retransmit_len += 1;
            }
        }

        for packet in failures[..failure_len].iter().flatten() {
            self.log_send_failed_snapshot(config, now_ms, packet);
            self.in_flight
                .remove_message(packet.channel_id, packet.message_id);
            let _ = self.events.push(EngineOutput::SendFailed(SendFailedEvent {
                channel_id: packet.channel_id,
                message_id: packet.message_id,
                reason: SendFailureReason::RetryLimitReached,
            }));
        }

        for packet in retransmits[..retransmit_len].iter().flatten() {
            let message_failed = failed_messages[..failed_message_len]
                .iter()
                .flatten()
                .any(|key| *key == (packet.channel_id, packet.message_id));

            if message_failed {
                continue;
            }

            self.in_flight.note_retransmit(packet.packet_number, now_ms);
            let _ = self.events.push(EngineOutput::Write(WriteEvent {
                packet_number: packet.packet_number,
                bytes: packet.bytes,
                len: packet.len,
                attempts: packet.attempts.saturating_add(1),
                priority: crate::engine::machine::WritePriority::Retransmit,
            }));
        }
    }

    #[cfg(feature = "std")]
    fn log_send_failed_snapshot(
        &self,
        config: &EngineConfig,
        now_ms: u64,
        failed: &crate::engine::machine::inflight::InFlightPacket,
    ) {
        eprintln!(
            "msrt in_flight send_failed now={} len={} failed_channel={} failed_message={} failed_packet={} attempts={} age_ms={} retry_limit={} rto_ms={}",
            now_ms,
            self.in_flight.len(),
            failed.channel_id.get(),
            failed.message_id.get(),
            failed.packet_number.get(),
            failed.attempts,
            now_ms.saturating_sub(failed.last_sent_ms),
            config.max_retransmit_attempts,
            config.retransmit_timeout_ms,
        );
    }

    #[cfg(not(feature = "std"))]
    fn log_send_failed_snapshot(
        &self,
        _config: &EngineConfig,
        _now_ms: u64,
        _failed: &crate::engine::machine::inflight::InFlightPacket,
    ) {
    }
}
