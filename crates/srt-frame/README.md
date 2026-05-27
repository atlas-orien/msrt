# srt-frame

Frame encoder and decoder boundary for SRT.

This crate defines the standard frame encoder and decoder boundary. It will own CRC16, packet resync, partial packet handling, and sticky packet handling. It is `no_std`, uses `heapless`, and does not use `std::vec::Vec`.

Current status: first frame boundary implementation.

Main entry points:

- `frame.rs`: frame constants and borrowed frame structure.
- `codec.rs`: codec entry point.
- `codec/traits.rs`: frame encoder and decoder traits.
- `codec/encoder.rs`: encoder entry point.
- `codec/encoder/`: packet and header encoding.
- `codec/decoder.rs`: decoder entry point.
- `codec/decoder/`: decoder state and fixed-capacity buffer.
- `crc.rs`: CRC16 contract and default implementation.
