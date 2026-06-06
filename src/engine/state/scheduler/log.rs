//! Scheduler debug logging.

use super::event::EngineOutput;

pub(super) fn log_event(now_ms: u64, queue: &str, offset: usize, event: &EngineOutput) {
    match event {
        EngineOutput::Write(write) => {
            let packet_type = packet_type(write.as_bytes())
                .map(|packet_type| packet_type.code())
                .unwrap_or_default();
            tracing::debug!(
                target: "msrt::scheduler",
                now_ms,
                queue,
                offset,
                kind = "write",
                packet_type,
                message_id = write.key.message_id.get(),
                packet_index = write.key.packet_index.get(),
                attempts = write.attempts,
                len = write.len,
                priority = ?write.priority,
                "msrt scheduler event",
            );
        }
        EngineOutput::SendFailed(failed) => {
            tracing::debug!(
                target: "msrt::scheduler",
                now_ms,
                queue,
                offset,
                kind = "send_failed",
                packet_type = ?failed.packet_type,
                message_id = failed.message_id.get(),
                "msrt scheduler event",
            );
        }
    }
}

fn packet_type(bytes: &[u8]) -> Option<crate::core::PacketType> {
    crate::core::PacketType::from_code(*bytes.get(crate::wire::WIRE_HEADER_LEN)?)
}
