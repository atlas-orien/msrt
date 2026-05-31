# v1 架构归档

v1 是 SRT 的 no_std protocol MVP 阶段。

这个阶段已经完成：

- workspace 边界
- crate 边界
- no_std 标准协议核心
- concrete `Engine` MVP API
- 最小 ACK / in-flight / tick 重发闭环
- message fragment reassembly
- 噪声、CRC 错误、丢包、重发、双向通信 smoke simulation
- 架构文档
- CI / git hook

v1 MVP 不是完整可互通协议标准。它证明了 no_std engine 模型可行，但还没有冻结最终 wire format，也没有实现完整可靠性算法。

## 文档列表

- [SRT 总设计](srt-design.md)：整体协议设计方向。
- [srt-core 设计](srt-core-design.md)：`srt-core` crate 的设计边界。
- [srt-reliability 设计](srt-reliability-design.md)：`srt-reliability` crate 的可靠性策略边界。
- [srt-engine 设计](srt-engine-design.md)：`srt-engine` crate 的协议引擎边界。
- [srt-wire 设计](srt-wire-design.md)：`srt-wire` crate 的串口字节流边界。
- [参考图](image.png)：用于理解 Packet / Frame 分层的参考图。
