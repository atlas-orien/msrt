# srt

No-std facade crate for the SRT protocol standard.

This crate does not implement additional protocol logic. It only re-exports the current protocol core crates so downstream users can access the SRT standard boundary from one entry point.

## Re-exports

- `srt::core`
- `srt::error`
- `srt::reliability`
- `srt::engine`
- `srt::wire`

## Non-goals

- does not provide an OS SDK
- does not provide an MCU HAL adapter
- does not provide a tokio adapter
- does not implement a CLI
- does not implement the protocol state machine

This crate is mainly used as a facade and integration-test target.

## Smoke Test

```sh
cargo run -p srt --bin srt-smoke
```

The smoke binary simulates basic two-endpoint transport and injects packet loss, duplicate packets, noise, and checksum corruption. It validates whether the current v1 boundaries can support later protocol implementation work.
