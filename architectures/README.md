# 架构文档

这个目录用于记录 SRT 在实现协议细节之前的架构决策。

## 版本目录

- [v1](v1/README.md)：no_std protocol 阶段，包含已完成的 MVP、hardening 和 stable protocol draft。

## 当前阶段

当前 v1 MVP、hardening 当前范围和 stable protocol draft-lock 已经完成。

v1 已经验证 `srt-engine` 的 no_std 状态机模型：`send(message)`、`receive(bytes)`、`tick(now)`、`poll_event()`，并通过 smoke simulation 跑通半包、粘包、一次 receive 多包、噪声、CRC 错误、丢包、ACK、重发和双向 message 交付。

当前 [v1 stable protocol draft](v1/srt-stable-protocol-draft.md) 已冻结第一版 wire format、packet layout、MESSAGE / ACK serialization 和基础可靠性行为边界。后续重点是可靠性算法加深，而不是重新拆 crate 或改运行环境模型。
