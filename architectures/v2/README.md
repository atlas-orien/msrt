# v2 架构文档

v2 进入 engine channel 语义细化阶段。

v1 已冻结的基础保持不变：

- wire envelope / packet / frame 的 v1 兼容边界
- `send` / `send_on` / `receive` / `tick` / `poll_event` 的 no_std 驱动模型
- reliable transport 当前范围与 fixed-capacity 设计

v2 第一阶段目标：

- 在不破坏 v1 基线行为的前提下，细化 channel 的职责语义。
- 把“仅用于 reliability 路由”的 channel，扩展为“可声明用途”的 channel 模型。
- 冻结两个默认 well-known channel：`DEFAULT` 和 `LOG`。
- 允许外部 adapter 自由定义 `16..=255` 的应用 channel。
- 保持 `srt` 核心只负责 channel 契约，不内置具体业务 SDK 接口。

## 文档列表

- [srt-engine channel v2 设计](srt-engine-channel-v2-design.md)：定义 channel profile、well-known channel、日志 channel 行为、core/adapter 边界、兼容策略和迁移步骤。
