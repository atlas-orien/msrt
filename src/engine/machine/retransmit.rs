//! Tick-driven retransmission.

use crate::engine::{
    Engine, EngineOutput, MAX_IN_FLIGHT_PACKETS, SendFailedEvent, SendFailureReason, WriteEvent,
};

pub(crate) fn tick(engine: &mut Engine, now_ms: u64) {
    engine.machine.now_ms = now_ms;
    engine
        .machine
        .reassembly
        .expire(now_ms, engine.config.reassembly_timeout_ms);

    let mut retransmits = [None; MAX_IN_FLIGHT_PACKETS];
    let mut retransmit_len = 0;
    let mut failures = [None; MAX_IN_FLIGHT_PACKETS];
    let mut failure_len = 0;
    let mut failed_messages = [None; MAX_IN_FLIGHT_PACKETS];
    let mut failed_message_len = 0;

    for packet in engine.machine.in_flight.packets() {
        if now_ms.saturating_sub(packet.last_sent_ms) < engine.config.retransmit_timeout_ms {
            continue;
        }

        if packet.attempts >= engine.config.max_retransmit_attempts {
            let already_failed = failed_messages[..failed_message_len]
                .iter()
                .flatten()
                .any(|key| *key == (packet.channel_id, packet.message_id));

            if !already_failed && failure_len < failures.len() {
                failures[failure_len] = Some(*packet);
                failure_len += 1;
                failed_messages[failed_message_len] = Some((packet.channel_id, packet.message_id));
                failed_message_len += 1;
            }
        } else if retransmit_len < retransmits.len() {
            retransmits[retransmit_len] = Some(*packet);
            retransmit_len += 1;
        }
    }

    for packet in failures[..failure_len].iter().flatten() {
        engine
            .machine
            .in_flight
            .remove_message(packet.channel_id, packet.message_id);
        let _ = engine
            .machine
            .events
            .push(EngineOutput::SendFailed(SendFailedEvent {
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

        engine
            .machine
            .in_flight
            .note_retransmit(packet.packet_number, now_ms);
        let _ = engine.machine.events.push(EngineOutput::Write(WriteEvent {
            packet_number: packet.packet_number,
            bytes: packet.bytes,
            len: packet.len,
        }));
    }
}
