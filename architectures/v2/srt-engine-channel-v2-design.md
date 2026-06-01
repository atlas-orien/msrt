# srt-engine channel v2 设计

## 背景

v1 已冻结并验证了 `ChannelId` + reliability policy 的最小可行模型：

- `ChannelId` 用于路由与隔离。
- channel 可绑定 `Reliable` / `BestEffort`。
- engine 已支持多 channel 并发收发与 reassembly 隔离。

这个模型足够支撑 v1 reliable transport，但对于上层业务语义仍然过粗。

典型问题：

- 上层希望有“日志 channel”，并在 engine 输出侧直接区分日志事件。
- 当前只能靠调用方约定某个 `ChannelId` 是日志，协议层没有显式语义声明。
- 后续要引入控制、遥测、调试等 channel 时，缺少统一 profile 约束。

因此 v2 第一阶段只做一件事：

```text
细化 engine channel 语义，使 channel 可以承载用途声明（先覆盖日志）。
```

## 目标与非目标

### 目标

1. 在现有 `ChannelId` 基础上引入 channel profile 概念。
2. 支持日志 channel 的显式声明与识别。
3. 保持 v1 wire format 兼容（不改 MESSAGE/ACK 编码）。
4. 保持 no_std、fixed-capacity、非阻塞 engine API 模型。
5. 允许渐进迁移：旧代码不配置 profile 也可继续工作。

### 非目标

1. 本阶段不新增 frame type。
2. 本阶段不改 ACK/retransmit 算法。
3. 本阶段不引入 runtime adapter（tokio/embassy/UART driver）。
4. 本阶段不定义完整日志 payload 标准（先只定义 channel 语义）。
5. 本阶段不定义业务 SDK 接口，例如 `send_motor_command`、`send_pid_config` 或 `send_log_line`。

## 设计原则

1. **语义与编码解耦**：channel 语义属于 engine/config 层，不强制写入 v1 frame。
2. **兼容优先**：默认行为与 v1 一致，profile 为增量能力。
3. **低成本判定**：channel 语义查询必须是 O(N) 固定小 N（沿用 fixed array 策略）。
4. **可扩展**：默认应用消息和日志是第一阶段能力，外部 adapter 可定义更多应用 channel。

## v2 Channel 模型

### 0) Core 与 Adapter 边界

v2 必须先明确一个边界：

```text
srt core:
  定义 channel id、profile、reliability、message transport。

external adapter:
  定义业务消息类型、payload schema、handler、便捷发送 API。
```

`srt` 核心不应该知道业务命令是什么，也不应该把某个项目里的业务消息做成内置 API。

例如这些不属于 `srt` 核心：

```text
send_motor_command(...)
send_set_pid(...)
send_log_line(...)
send_imu_sample(...)
on_control(...)
on_telemetry(...)
```

它们应该属于外部 adapter、设备 SDK 或具体应用协议。

`srt` 核心只需要让 adapter 可以稳定地声明和查询：

```text
ChannelId -> ChannelProfile -> ReliabilityMode
```

这样普通业务代码可以不直接接触 `ChannelId`，但这是 adapter 的职责，不是 `srt` 核心的职责。

### 0.1) ChannelId 宽度

v2 将 `ChannelId` 定义为 `u8`。

```text
ChannelId(u8)
```

原因：

- SRT 面向 MCU 和串口链路，每个 MESSAGE frame 少 1 byte 有实际价值。
- 255 个 channel 已经远超过常见 MCU/游戏/机器人应用的真实需求。
- 大量业务分类应该放在 payload 协议中，例如 protobuf oneof，而不是无限拆 channel。
- channel 的定位是轻量分流和可靠性/profile 选择，不是业务类型注册表。

### 1) ChannelProfile

v2 为 channel 引入协议级用途分类：

```text
ChannelProfile
  - Data
  - Log
```

v2 第一阶段只冻结 `Data` 和 `Log`。

`ChannelProfile` 不是业务类型。它只描述通道的大类用途。

例如：

```text
业务类型: GameInput
profile: Data

业务类型: SetMotorSpeed
profile: Data

业务类型: DebugLogLine
profile: Log
```

业务类型到 `ChannelId` 的映射由外部 adapter 决定。

### 2) ChannelSpec

在 engine 配置中，将“单纯 reliability policy”扩展为“channel spec”：

```text
ChannelSpec
  - channel_id
  - reliability_mode
  - profile
```

其中：

- `reliability_mode` 继续复用 v1 的 `Reliable` / `BestEffort`。
- `profile` 用于表达 channel 业务语义。

### 3) 默认行为

未配置 `ChannelSpec` 的 channel：

- `profile` 视为 `Data`。
- `reliability_mode` 维持 v1 默认（Reliable）。

这保证 v1 调用方无感迁移。

### 4) Well-known channel 与应用 channel

v2 保留一小段 well-known channel，用于跨实现共享的协议级约定：

```text
0..15   SRT well-known channels
16..255 application-defined channels
```

第一阶段只冻结最小集合：

```text
ChannelId::DEFAULT = 0
ChannelId::LOG     = 1
```

`ChannelId::DEFAULT` 也可以称为 application/default channel。它承载普通应用消息，默认 `Reliable`。

`ChannelId::LOG` 承载日志消息，默认建议 `BestEffort`。

`2..15` 暂时保留给 SRT 未来协议级扩展，不在 v2 第一阶段定义含义。

应用可以扩展自己的 channel table，但应该通过外部 adapter 或 `EngineConfig` 显式声明，而不是在业务层散落硬编码数字。

例如一个游戏 adapter 可以定义：

```text
16 = GameReliableData
17 = GameRealtimeState
18 = AssetChunk
```

如果应用协议已经使用 protobuf、postcard、bincode 或自定义 binary 在 payload 内分发业务类型，那么大多数业务消息都可以继续走 `ChannelId::DEFAULT`。SRT 不需要区分“控制命令”和“普通数据”，这些属于应用协议。

## 日志 channel 语义

### 1) 识别

当 `channel_id` 命中 `profile = Log` 的配置时，engine 将该 message 视为日志消息。

### 2) 传输

日志消息仍使用 v1 MESSAGE frame。

```text
MESSAGE(channel_id=log_channel, ... payload=log bytes)
```

不新增 wire 字段，不破坏对端 v1 解析。

### 3) 交付

v2 建议 engine 在输出事件层支持日志语义分流（两种可接受策略）：

1. 保持 `Message` 事件不变，只新增 `message.profile` 字段。
2. 新增 `Log` 事件（内部仍来自 MESSAGE frame）。

为了最小破坏，优先策略 1：

```text
EngineOutput::Message { channel_id, profile, bytes }
```

调用方可直接按 `profile == Log` 打印或上报。

### 4) 可靠性建议

日志 channel 默认建议 `BestEffort`，原因：

- 日志更偏向“尽快看到新信息”，而不是严格补发旧日志。
- 可减少 in-flight 压力与重传占用。

但规范不强制：允许调用方把日志 channel 配成 `Reliable`。

## 兼容性

### 向后兼容

- v1 对端仍可接收 v2 发出的日志消息（本质仍是普通 MESSAGE）。
- 未升级 profile 的节点把日志当普通 message 处理，不影响互通。

### 向前扩展

后续若需要跨端共享语义，可在 v2.x/v3 评估：

- 在 MESSAGE flags 中扩展“profile hint”。
- 或新增轻量 metadata frame。

这些不属于 v2 第一阶段。

## API 演进建议

以下是建议方向（非本次实现承诺）：

1. `EngineConfig` 增加 `channel_specs`（替代/兼容 `channel_policies`）。
2. 保留 `set_channel_reliability` 兼容入口，并映射到 `channel_specs`。
3. `MessageEvent` 增加 `profile` 字段。
4. 保留低层发送 API：`send(message)` 和 `send_on(channel_id, message)`。
5. 不在 `srt` 核心新增业务便捷 API，例如 `send_log_line`。

如果需要便捷 API，应由外部 adapter 封装：

```text
adapter.send_log_line(...)
  -> engine.send_on(ChannelId::LOG, encoded_log_line)
```

这能让业务层不直接知道 `ChannelId`，同时保持 `srt` 核心的协议边界干净。

## 测试与验收（文档阶段定义）

v2 第一阶段落地时，至少应新增：

1. `Log` channel 配置后，接收事件可区分日志与普通数据。
2. 未配置 profile 的旧 channel 行为与 v1 一致。
3. `Log + BestEffort` 不进入 in-flight，不触发重传（与 v1 规则一致）。
4. `Log + Reliable` 可正常 ACK/重传（与 v1 规则一致）。
5. 多 channel 混合收发时，日志与业务消息不串台。

## 迁移步骤

1. **文档冻结**：先冻结 v2 channel 语义（本文档）。
2. **配置层改造**：引入 `ChannelSpec`，保留旧配置兼容。
3. **事件层改造**：`MessageEvent` 增加 `profile`。
4. **测试补齐**：新增日志 channel 场景测试。
5. **对外 facade**：在 `srt` crate 暴露 `ChannelProfile` / `ChannelSpec` / well-known `ChannelId`。
6. **adapter 后置**：业务便捷 API 留给外部 adapter 或后续独立 crate。

## 决策记录

- 决策：v2 第一阶段不改 wire。
  - 原因：风险最小，保留 v1 互通。
- 决策：日志语义先在 engine 配置和事件层表达。
  - 原因：满足“支持打印日志”目标，同时不引入协议编码膨胀。
- 决策：日志 channel 默认建议 BestEffort。
  - 原因：符合实时日志的时效优先特性。
- 决策：`srt` 核心不定义业务发送接口。
  - 原因：业务类型、payload schema 和 handler 属于外部 adapter；核心只定义协议 channel 契约。
- 决策：冻结极小 well-known channel 集合。
  - 原因：减少 v2 编号承诺，同时为日志能力提供稳定入口。

## 总结

v2 第一阶段的核心是：

```text
让 channel 不再只是“可靠性路由编号”，而是“带用途语义的传输通道”。
```

先把日志 channel 做成正式能力，再逐步扩展 control/telemetry 等 profile。这条路径能在保持 v1 wire 兼容的同时，为后续功能打开清晰边界。
