# srt

`srt` 是 SRT no_std 标准协议的门面 crate。

它不实现额外逻辑，只统一 re-export 当前协议内核 crate，方便下游以一个入口使用 SRT 标准边界。

## Re-exports

- `srt::core`
- `srt::error`
- `srt::reliability`
- `srt::runtime`
- `srt::wire`

## 非职责

- 不做 OS SDK
- 不做 MCU HAL adapter
- 不做 tokio adapter
- 不做 CLI
- 不实现协议状态机

这个 crate 主要用于统一入口和 integration tests。

## Smoke Test

```sh
cargo run -p srt --bin srt-smoke
```
