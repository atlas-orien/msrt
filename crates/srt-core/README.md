# srt-core

Core protocol primitives for SRT.

This crate owns frame and packet type markers, flags, stream identifiers, and sequence identifiers for the common SRT protocol standard. Shared errors live in `srt-error`. It is `no_std` and avoids allocation by default.

Current status: boundary-only scaffold.
