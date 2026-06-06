//! Tick-driven retransmission.

use crate::engine::{
    EngineConfig, SendFailedEvent, SendFailureReason,
    config::MAX_IN_FLIGHT_PACKETS,
    state::{EngineOutput, EngineState, WriteEvent},
};

use super::inflight::InFlightPacket;

impl EngineState {
    pub(crate) fn tick_retransmit(&mut self, config: &EngineConfig, now_ms: u64) {
        self.clock.update(now_ms);
        self.reassembly.expire(now_ms, config.reassembly_timeout_ms);

        if !self.recovery.should_tick(now_ms) {
            return;
        }

        let mut retransmits = [None; MAX_IN_FLIGHT_PACKETS];
        let mut retransmit_len = 0;
        let mut failures = [None; MAX_IN_FLIGHT_PACKETS];
        let mut failure_len = 0;
        let mut failed_messages = [None; MAX_IN_FLIGHT_PACKETS];
        let mut failed_message_len = 0;

        for packet in self.recovery.packets() {
            if !packet.sent {
                continue;
            }

            if now_ms.saturating_sub(packet.last_sent_ms) < config.retransmit_timeout_ms {
                continue;
            }

            if packet.attempts >= config.max_retransmit_attempts {
                let already_failed = failed_messages[..failed_message_len]
                    .iter()
                    .flatten()
                    .any(|key| *key == (packet.packet_type, packet.message_id));

                if !already_failed && failure_len < failures.len() {
                    failures[failure_len] = Some(*packet);
                    failure_len += 1;
                    failed_messages[failed_message_len] =
                        Some((packet.packet_type, packet.message_id));
                    failed_message_len += 1;
                }
            } else if retransmit_len < retransmits.len() {
                retransmits[retransmit_len] = Some(*packet);
                retransmit_len += 1;
            }
        }

        for packet in failures[..failure_len].iter().flatten() {
            self.log_send_failed_snapshot(config, now_ms, packet);
            self.recovery
                .remove_message(packet.packet_type, packet.message_id);
            let _ = self
                .scheduler
                .push(EngineOutput::SendFailed(SendFailedEvent {
                    packet_type: packet.packet_type,
                    message_id: packet.message_id,
                    reason: SendFailureReason::RetryLimitReached,
                }));
        }

        for packet in retransmits[..retransmit_len].iter().flatten() {
            let message_failed = failed_messages[..failed_message_len]
                .iter()
                .flatten()
                .any(|key| *key == (packet.packet_type, packet.message_id));

            if message_failed {
                continue;
            }

            let _ = self.scheduler.push(EngineOutput::Write(WriteEvent {
                key: packet.key,
                bytes: packet.bytes,
                len: packet.len,
                attempts: packet.attempts.saturating_add(1),
                priority: crate::engine::state::scheduler::WritePriority::Retransmit,
            }));
        }
    }

    #[cfg(feature = "std")]
    fn log_send_failed_snapshot(
        &self,
        config: &EngineConfig,
        now_ms: u64,
        failed: &InFlightPacket,
    ) {
        eprintln!(
            "msrt in_flight send_failed now={} len={} message_len={} ack_pending={} ack_pending_len={} failed_type={:?} failed_message={} failed_index={} attempts={} age_ms={} retry_limit={} rto_ms={}",
            now_ms,
            self.recovery.in_flight_len(),
            self.message.len(),
            self.ack.is_pending(),
            self.ack.pending_len(),
            failed.packet_type,
            failed.message_id.get(),
            failed.key.packet_index.get(),
            failed.attempts,
            now_ms.saturating_sub(failed.last_sent_ms),
            config.max_retransmit_attempts,
            config.retransmit_timeout_ms,
        );
        self.scheduler.log_snapshot(now_ms, self.ack.is_pending());
        self.log_in_flight_packets(now_ms);
    }

    #[cfg(feature = "std")]
    fn log_in_flight_packets(&self, now_ms: u64) {
        for packet in self.recovery.packets() {
            eprintln!(
                "msrt in_flight packet now={} type={:?} msg={} idx={} attempts={} age_ms={} len={} sent={}",
                now_ms,
                packet.packet_type,
                packet.message_id.get(),
                packet.key.packet_index.get(),
                packet.attempts,
                now_ms.saturating_sub(packet.last_sent_ms),
                packet.len,
                packet.sent,
            );
        }
    }

    #[cfg(not(feature = "std"))]
    #[allow(dead_code)]
    fn log_in_flight_packets(&self, _now_ms: u64) {}

    #[cfg(not(feature = "std"))]
    fn log_send_failed_snapshot(
        &self,
        _config: &EngineConfig,
        _now_ms: u64,
        _failed: &InFlightPacket,
    ) {
    }
}
