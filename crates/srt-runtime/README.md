# srt-runtime

Protocol runtime boundary for SRT.

This crate defines how the standard protocol is driven: send intent, receive input, response generation, progress ticks, and raw link I/O contracts.

It is not an operating-system runtime, not a tokio adapter, and not an MCU HAL adapter. It is part of the `no_std` protocol standard.

Current status: boundary-only scaffold. No protocol state machine is implemented yet.
