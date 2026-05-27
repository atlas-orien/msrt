# srt-core

Core protocol primitives for SRT.

This crate owns packet type markers, flags, stream identifiers, and sequence identifiers for the common SRT protocol standard. Shared errors live in `srt-error`. It is `no_std` and avoids allocation by default.

`lib.rs` is intentionally kept as a small module and re-export surface. `packet/` is the main entry point for the protocol structure:

- `packet`: packet entry point.
- `packet/header`: packet metadata.
- `packet/header/stream_id`: stream identifier.
- `packet/header/seq`: sequence number.
- `packet/header/flags`: packet flags.
- `packet/payload`: borrowed payload view.
- `packet/kind`: packet categories.

Current status: boundary-only scaffold.
