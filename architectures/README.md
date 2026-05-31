# 架构文档

这个目录用于记录 SRT 在实现协议细节之前的架构决策。

## 版本目录

- [v1](v1/README.md)：no_std protocol scaffold，冻结 crate 边界和 engine 语义。

## 当前阶段

当前 v1 MVP 已经收口。

v1 MVP 已经验证 `srt-engine` 的 no_std 状态机模型：`send(message)`、`receive(bytes)`、`tick(now)`、`poll_event()`，并通过 smoke simulation 跑通噪声、CRC 错误、丢包、ACK、重发和双向 message 交付。

下一阶段应该进入 v1 hardening：优先完善 streaming wire decode，处理半包、粘包和一次 receive 多包，再冻结正式 wire format draft。
