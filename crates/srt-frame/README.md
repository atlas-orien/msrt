# srt-frame

Frame encoder and decoder boundary for SRT.

This crate defines the standard frame encoder and decoder boundary. It will own CRC16, packet resync, partial packet handling, and sticky packet handling. It is `no_std`, uses `heapless`, and does not use `std::vec::Vec`.

Current status: boundary-only scaffold.
