//! Scheduler polling helpers.

use crate::core::{Error, Result};
use crate::engine::state::{ack::AckState, numbers::NumberState, recovery::RecoveryState};
use crate::engine::{EngineConfig, EnginePoll};

use super::event::{EngineOutput, WriteEvent, WritePriority};

pub(super) fn poll_pending_ack<'a>(
    config: &EngineConfig,
    ack: &mut AckState,
    _numbers: &mut NumberState,
    tx_buf: &'a mut [u8],
) -> Result<EnginePoll<'a>> {
    let Some(key) = ack.pop() else {
        return Ok(EnginePoll::Idle);
    };

    let written =
        crate::engine::codec::outgoing::encode_ack_packet(key, tx_buf, &config.integrity)?;

    Ok(EnginePoll::Transmit {
        bytes: &tx_buf[..written],
        attempts: 0,
    })
}

pub(crate) fn poll_event<'a>(
    event: EngineOutput,
    recovery: &mut RecoveryState,
    now_ms: u64,
    tx_buf: &'a mut [u8],
) -> Result<EnginePoll<'a>> {
    match event {
        EngineOutput::Write(write) => poll_write(write, recovery, now_ms, tx_buf),
        EngineOutput::SendFailed(failed) => Ok(EnginePoll::SendFailed(failed)),
    }
}

fn poll_write<'a>(
    write: WriteEvent,
    recovery: &mut RecoveryState,
    now_ms: u64,
    tx_buf: &'a mut [u8],
) -> Result<EnginePoll<'a>> {
    if tx_buf.len() < write.len {
        return Err(Error::buffer_too_small());
    }

    match write.priority {
        WritePriority::Retransmit => {
            recovery.note_retransmit_sent(write.key, now_ms);
        }
        WritePriority::Control | WritePriority::NewData => {
            recovery.note_sent(write.key, now_ms);
        }
    }

    tx_buf[..write.len].copy_from_slice(write.as_bytes());

    Ok(EnginePoll::Transmit {
        bytes: &tx_buf[..write.len],
        attempts: write.attempts,
    })
}
