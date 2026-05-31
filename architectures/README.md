# 架构文档

这个目录用于记录 SRT 在实现协议细节之前的架构决策。

## 版本目录

- [v1](v1/README.md)：no_std protocol 阶段，包含已完成的 foundation / hardening，以及仍在推进的 reliable transport。

## 当前阶段

当前 v1 foundation 和 hardening 当前范围已经完成，但 v1 还没有完成。

v1 已经验证 `srt-engine` 的 no_std 状态机模型：`send(message)`、`receive(bytes)`、`tick(now)`、`poll_event()`，并通过 smoke simulation 跑通半包、粘包、一次 receive 多包、噪声、CRC 错误、丢包、ACK、重发、同时双向发送和 message 交付。

但是 v1 的目标是可靠传输，不只是协议骨架。因此下一步是 [v1 reliable transport plan](v1/srt-reliable-transport-plan.md)：补齐 ACK range、重试失败、多 message / 多 channel reassembly、partial reliability 和对应验收测试。
