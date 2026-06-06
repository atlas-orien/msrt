//! Scheduler debug logging.

use super::event::EngineOutput;

pub(super) fn log_event(now_ms: u64, queue: &str, offset: usize, event: &EngineOutput) {
    match event {
        EngineOutput::Write(write) => {
            let packet_type = packet_type(write.as_bytes())
                .map(|packet_type| packet_type.code())
                .unwrap_or_default();
            eprintln!(
                "msrt scheduler event now={} queue={} offset={} kind=write packet_type={} ch={} msg={} idx={} attempts={} len={} priority={:?}",
                now_ms,
                queue,
                offset,
                packet_type,
                write.key.channel_id.get(),
                write.key.message_id.get(),
                write.key.packet_index.get(),
                write.attempts,
                write.len,
                write.priority,
            );
        }
        EngineOutput::SendFailed(failed) => {
            eprintln!(
                "msrt scheduler event now={} queue={} offset={} kind=send_failed ch={} msg={}",
                now_ms,
                queue,
                offset,
                failed.channel_id.get(),
                failed.message_id.get(),
            );
        }
    }
}

fn packet_type(bytes: &[u8]) -> Option<crate::core::PacketType> {
    crate::core::PacketType::from_code(*bytes.get(crate::wire::WIRE_HEADER_LEN)?)
}
