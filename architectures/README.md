# MSRT 架构总纲

MSRT 是面向 MCU 和上位机之间字节链路的消息传输协议核心。它应该能运行在 `no_std` 环境里，也应该能被 host、RTIC、Embassy、Tokio 或测试程序驱动，但协议核心不能绑定任何一个 runtime、HAL 或 adapter。

当前项目只讨论协议核心，不讨论串口驱动、USB CDC 驱动、DMA、tokio task、RTIC task 或命令行工具。这些属于 adapter 或应用层。

## 核心原则

- 协议核心是状态机，不是 runtime。
- 外部 API 尽量小，默认入口是 `endpoint` 和 `error`。
- 外部负责提供时间、输入 bytes 和发送 buffer。
- engine 负责维护协议状态、分片、ACK、重传、重组和输出动作。
- core、engine、reliability、wire 分层必须清楚，不能互相泄漏职责。
- 文档只记录架构思想，具体实验、实现步骤和临时计划不放在这里。

## 用户心智模型

外部用户只需要持续驱动 endpoint：

```rust
endpoint.receive(now_ms, bytes);
endpoint.send(message)?;

match endpoint.poll(now_ms, tx_buf)? {
    EndpointPoll::Transmit { bytes, .. } => write_link(bytes),
    EndpointPoll::Message(message) => deliver(message),
    EndpointPoll::SendFailed(failed) => report(failed),
    EndpointPoll::Idle => {}
}
```

`receive` 不等待更多输入，`send` 不等待链路发送完成，`poll` 每次只推进并返回一个高层动作。`Engine` 是 endpoint 内部持有的协议状态机，不作为 crate 外部 API 暴露。

## 分层

MSRT 当前分成五个协议部分：

- [core](core.md)：协议对象和协议语言。
- [engine](engine.md)：协议状态机入口和状态推进。
- [endpoint](endpoint.md)：连接生命周期、单 peer 和多 peer 会话管理。
- [reliability](reliability.md)：ACK、去重、重传、可靠性策略。
- [wire](wire.md)：字节流边界、校验和重同步。
- [header-redesign](header-redesign.md)：为了减少包头而做的 packet kind/header 边界改版记录。
- [stress-testing](stress-testing.md)：压力测试过程、ACK 修复和噪音模型结论。

它们的关系可以理解为：

```text
wire
  从 byte stream 恢复完整 encoded packet bytes

core
  定义 packet、packet kind、message id 等协议对象

reliability
  判断 packet 是否重复、是否已 ACK、是否需要重传

engine
  组合以上能力，对外表现为一个可轮询状态机

endpoint
  在 engine 之上管理连接生命周期和 peer -> Engine 映射
```

## Cargo Features

MSRT 的 feature 边界要保持小而明确：

```text
default = ["std"]
dynamic-recovery
tracing
```

- `std`：默认启用，面向普通 host/bin/test 使用。
- `dynamic-recovery`：启用 RTT/PTO 动态恢复策略，适合动态网络、UDP、公网或延迟抖动明显的链路。
- `tracing`：启用库内部结构化诊断日志。默认不开，不跟随 `std`，库也不配置 subscriber。

库代码里不应该使用 `println!` / `eprintln!` 做诊断输出。需要诊断时使用 `tracing` feature；真正把日志打印到终端、文件，或者附带文件名和行号，是 bin、adapter 或应用层的责任。

默认配置服务于 MCU/串口这类短链路：

```text
固定 RTO
固定 retry limit
无 tracing 依赖
```

动态网络或公网 UDP adapter 可以显式启用 `dynamic-recovery`。历史压力测试 binary 已经从主 crate 移除，后续应该放在 adapter、仿真项目或独立测试项目里。

## 临时实现文档

未来持续开发时，可以在本地创建：

```text
architectures/implement/
```

这个目录用于放临时实现计划、实验记录、调试笔记和迁移步骤。它不进入 git，避免架构文档再次变成过程记录。
