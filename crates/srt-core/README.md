# srt-core

Core protocol primitives for SRT.

This crate owns packet and protocol frame primitives for the common SRT protocol standard. Shared errors live in `srt-error`. It is `no_std` and avoids allocation by default.

`lib.rs` is intentionally kept as a small module and re-export surface.

- `packet`: packet transport unit.
- `packet/header`: packet metadata.
- `packet/number`: packet number.
- `packet/payload`: borrowed payload view containing encoded protocol frames.
- `frame`: protocol frame entry point.
- `frame/message`: message-oriented MESSAGE frame.
- `frame/ack`: ACK frame.

Current status: boundary-only scaffold.
