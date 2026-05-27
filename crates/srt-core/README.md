# srt-core

Core protocol primitives for SRT.

This crate owns packet type markers, flags, stream identifiers, and sequence identifiers for the common SRT protocol standard. Shared errors live in `srt-error`. It is `no_std` and avoids allocation by default.

`lib.rs` is intentionally kept as a small module and re-export surface. Concrete primitives live in focused modules:

- `id`: stream and protocol identifiers.
- `seq`: sequence number primitives.
- `flags`: protocol flags.
- `packet`: packet-level primitives.

Current status: boundary-only scaffold.
