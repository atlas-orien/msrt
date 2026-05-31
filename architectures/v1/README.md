# v1 架构文档

v1 是 SRT 的 no_std protocol 阶段。

当前状态：

```text
v1 MVP: 已完成
v1 hardening: 当前范围已完成
v1 stable protocol: draft-locked
```

v1 MVP 已经完成：

- workspace 边界
- crate 边界
- no_std 标准协议核心
- concrete `Engine` MVP API
- 最小 ACK / in-flight / tick 重发闭环
- message fragment reassembly
- 噪声、CRC 错误、丢包、重发、双向通信 smoke simulation
- 架构文档
- CI / git hook

v1 MVP 不是完整可互通协议标准。它证明了 no_std engine 模型可行。当前 v1 stable protocol draft 已冻结第一版 wire format，但还没有实现完整可靠性算法。

v1 hardening 仍然属于 v1 工作。当前范围已经把 MVP 从 demo 级别推进到更接近真实串口可用的状态。

v1 stable protocol draft 已经冻结 wire format、packet layout、MESSAGE / ACK serialization 和基础可靠性行为边界。后续工作应进入可靠性算法、buffer 策略和多 channel 行为的加深阶段。

## 文档列表

- [SRT 总设计](srt-design.md)：整体协议设计方向。
- [srt-core 设计](srt-core-design.md)：`srt-core` crate 的设计边界。
- [srt-reliability 设计](srt-reliability-design.md)：`srt-reliability` crate 的可靠性策略边界。
- [srt-engine 设计](srt-engine-design.md)：`srt-engine` crate 的协议引擎边界。
- [srt-wire 设计](srt-wire-design.md)：`srt-wire` crate 的串口字节流边界。
- [v1 hardening](srt-hardening.md)：v1 加固阶段的目标、工作项和验收标准。
- [v1 stable protocol draft](srt-stable-protocol-draft.md)：v1 稳定协议草案。
- [参考图](image.png)：用于理解 Packet / Frame 分层的参考图。
