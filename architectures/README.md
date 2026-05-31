# 架构文档

这个目录用于记录 SRT 在实现协议细节之前的架构决策。

## 版本目录

- [v1](v1/README.md)：no_std protocol 阶段，包含已完成的 MVP、hardening 和 stable protocol draft。

## 当前阶段

当前 v1 MVP 和 hardening 当前范围已经完成，但 v1 stable protocol 还没有完成。

v1 MVP 和 hardening 已经验证 `srt-engine` 的 no_std 状态机模型：`send(message)`、`receive(bytes)`、`tick(now)`、`poll_event()`，并通过 smoke simulation 跑通半包、粘包、一次 receive 多包、噪声、CRC 错误、丢包、ACK、重发和双向 message 交付。

下一阶段是 [v1 stable protocol draft](v1/srt-stable-protocol-draft.md)：冻结正式 wire format、packet layout、MESSAGE / ACK serialization 和可靠性行为边界。
