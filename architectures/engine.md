# Engine

`engine` 是 MSRT 的协议状态机入口。它不是 executor，不是 adapter，也不是 HAL。

## 职责

engine 负责：

- 接收外部输入 bytes
- 接收外部提交的完整 message
- 分配 message id 和 message 内部 packet index
- 分片并编码 outgoing packet
- 处理 incoming packet
- 生成 ACK
- 维护 in-flight packet
- 执行超时重传
- 重组 message fragment
- 通过 `poll` 返回外部需要执行的动作

engine 不负责：

- 读取 UART 寄存器
- 管理 DMA ring
- 运行 tokio task
- 运行 RTIC task
- 选择中断号
- 决定链路 write 是否阻塞
- 管理应用消息格式

## 外部 API

engine 的外部 API 应该保持窄：

```rust
Engine::new(config)
Engine::default()
engine.send(message)
engine.send_log(message)
engine.receive(bytes)
engine.poll(now_ms, tx_buf)
```

`poll_event`、`tick`、内部队列、ACK range、reassembly slot、in-flight table 都不应该暴露给外部 crate。

## EngineConfig

`EngineConfig` 是创建 engine 时的协议配置。它不是外部配置文件，也不要求运行时读取 toml/json。

默认配置使用 CRC-16/XMODEM：

```rust
Engine::new(EngineConfig::default())
```

如果应用需要更强的数据合法性验证，可以在初始化 engine 时选择：

```rust
Engine::new(EngineConfig {
  integrity: IntegrityConfig::aead(),
  ..EngineConfig::default()
})
```

两端必须使用相同的 `integrity` 配置。`IntegrityConfig::aead()` 使用库内默认 key；如果需要应用自己控制 key，可以使用 `IntegrityConfig::aead_with_key(key)`。

`EngineConfig` 同时包含可靠发送的默认固定恢复参数：

```text
max_retransmit_attempts
retransmit_timeout_ms
reassembly_timeout_ms
fragment_bytes
initial_message_id
integrity
```

默认固定恢复适合串口、USB CDC、短线 MCU 链路这类延迟稳定的场景。这里的设计目标是可预测、代码少、不会把动态网络的复杂性强行带进 MCU。

如果启用 `dynamic-recovery` feature，`EngineConfig` 会额外包含 `dynamic_recovery`。这不是默认行为；它用于动态网络或公网 UDP 这类 RTT 和抖动会明显变化的链路。

## Poll 模型

`poll` 是 engine 的统一推进入口。

```text
poll(now_ms, tx_buf)
  -> 内部更新时间
  -> 检查是否需要重传
  -> 弹出一个待处理输出
  -> 如果是发送动作，把 bytes copy 到 tx_buf
  -> 返回 EnginePoll
```

外部可以这样驱动：

```text
loop:
  收到 bytes 时调用 receive(bytes)
  要发送应用消息时调用 send(message)
  周期性或事件触发调用 poll(now_ms, tx_buf)
  如果 poll 返回 Transmit，就把 bytes 写到底层链路
```

`poll` 每次返回一个动作，而不是一次性清空所有状态。这样 adapter 可以自然地控制写出节奏。

## Send 模型

`send(message)` 是 outgoing message 进入协议状态机的入口。调用方提交的是一个完整 application message，不需要自己拆 packet，也不需要关心 wire envelope。

外部发送入口只有两类：

- `send(message)`：发送可靠 Data message。
- `send_log(message)`：发送 best-effort Log message。

ACK、Ping、Pong 都是 engine/endpoint 内部协议行为，不允许外部应用直接发送。

当前 send 链路是：

```text
Engine::send(message)
  -> EngineState::send_data(config, message)
    -> send_data_impl
      -> 分配 message_id
      -> 使用 Data packet kind 和 Reliable 模式
      -> 按 fragment_bytes 拆分 message
      -> 每个 fragment 编码成 packet
      -> 每个 packet 包进 wire envelope
      -> 将 envelope 作为 Write event 放入事件队列
      -> reliable packet 同时保存到 in-flight table
```

`send_log(message)` 使用同样的分片和编码路径，但 packet kind 是 `Log`，reliability mode 是 `BestEffort`，不会进入 in-flight，也不会等待 ACK。

所以 `send` 做的是“生成待发送动作”，不是直接写链路。外部真正拿到 bytes 是通过后续 `poll()`：

```text
send(message)
  -> queue Write(packet 0)
  -> queue Write(packet 1)
  -> queue Write(packet 2)

poll(now_ms, tx_buf)
  -> EnginePoll::Transmit(packet 0 bytes)

poll(now_ms, tx_buf)
  -> EnginePoll::Transmit(packet 1 bytes)

poll(now_ms, tx_buf)
  -> EnginePoll::Transmit(packet 2 bytes)
```

`poll` 一次只返回一个动作。这样 engine 不需要知道 UART FIFO、DMA、USB CDC、UDP socket 或测试 adapter 一次能写多少 bytes。adapter 如果想尽快清空队列，可以在自己的主循环里反复调用 `poll`，直到返回 `Idle`。

## Send 和可靠性

Data message 和 Log message 在发送后的保存策略不同。

best-effort packet：

```text
send
  -> encode envelope
  -> queue Write event
  -> poll returns Transmit
  -> 不进入 in-flight
  -> 不等待 ACK
  -> 不重传
```

reliable packet：

```text
send
  -> encode envelope
  -> queue Write event
  -> copy envelope into in-flight table
  -> poll returns Transmit
  -> 等待 peer ACK
```

收到 ACK 后：

```text
receive(ack packet)
  -> update in-flight table
  -> remove acknowledged packet
```

如果超过重传时间还没有 ACK：

```text
poll(now_ms, tx_buf)
  -> tick_retransmit
  -> inspect in-flight table
  -> queue retransmit Write event
  -> pop one event
  -> EnginePoll::Transmit(retransmit bytes)
```

如果重传次数达到上限：

```text
poll(now_ms, tx_buf)
  -> tick_retransmit
  -> remove failed message packets from in-flight
  -> queue SendFailed event
  -> EnginePoll::SendFailed
```

因此 reliable send 的核心状态是两份数据：

- event queue 保存“等待外部执行的动作”。
- in-flight table 保存“已经发送或即将发送、仍然需要 ACK 的 packet 副本”。

这两份状态不能合并。event queue 是动作队列，`poll` 会不断弹出；in-flight table 是可靠性状态，ACK 到达或重传失败之前必须保留。

## Receive 模型

`receive(bytes)` 是 incoming byte stream 进入协议状态机的入口。它不等待更多输入，也不假设这次传入的 `bytes` 刚好是一包。

虽然 MCU 底层中断可能一次只收到 1 byte，但 adapter 交给 engine 的可能是任意一段连续 bytes：

```text
1 byte
half envelope
one full envelope
multiple sticky envelopes
noise + envelope
bad header + later valid envelope
```

因此 `receive` 的心智模型不是“处理一个串口中断字节”，而是“把一段已经到达的连续字节流喂给状态机”。

当前 receive 链路是：

```text
Engine::receive(bytes)
  -> EngineState::receive(config, bytes)
    -> ingress.feed(bytes, config.integrity)
      -> append 到 ingress buffer
      -> 查找 envelope magic
      -> 校验 envelope header
      -> 根据 length 等待完整 envelope
      -> 校验 envelope integrity tag
      -> 输出 packet bytes
    -> decode packet bytes
    -> apply packet to engine state
```

wire 层只负责从连续 bytes 中恢复一个完整 packet bytes。packet 进入 engine 后，engine 才处理 packet 语义：

- duplicate `Data` packet 会被重新 ACK，但不会再次进入 message reassembly。
- 新 `Data` packet 只有在 fragment 被 reassembly 接受后才会排队 ACK。
- message fragment 会进入 reassembly buffer。
- 如果 fragment 已经拼成完整 message，`receive` 会把完整 message 放入 message delivery queue。
- `Ack` packet 会更新 in-flight packet 状态。

这意味着 `receive` 不直接把完整 message 返回给应用，但它会推进 message reassembly。完整 message 最终通过后续 `poll()` 返回：

```text
receive(data packet fragment)
  -> reassembly complete
  -> queue complete message

poll(now_ms, tx_buf)
  -> EnginePoll::Message(message)
```

ACK 也是同样的模型：

```text
receive(ack-eliciting data packet)
  -> queue EngineOutput::Write(ack packet)

poll(now_ms, tx_buf)
  -> EnginePoll::Transmit(ack bytes)
```

所以 `receive` 的目标不是“只接收一个 packet 然后结束”，而是：

- 接收并缓存连续 bytes。
- 尽可能恢复完整 envelope。
- 校验 envelope 边界和 integrity tag。
- 解出 packet。
- 将 packet 应用到协议状态。
- 把需要外部执行的结果排进事件队列。

## ACK 时机

ACK 的时机必须和 reassembly 错误边界一致。

当前 Data packet 处理顺序是：

```text
decode Data packet
  -> 如果 PacketKey 已经见过:
       queue ACK
       return Duplicate

  -> 尝试 observe fragment 到 reassembly
       如果 fragment 和已有 message 状态冲突:
         return Corrupted
         不 ACK
         不 dedup

  -> observe PacketKey 到 receive-side dedup
  -> queue ACK
  -> 如果 message 完整，排队 Message event
```

这个顺序是压力测试后定下来的。关键点：

- duplicate Data 必须重新 ACK，因为之前的 ACK 可能丢了。
- reassembly 发现语义冲突时不能 ACK，否则发送方会以为该 packet 成功送达。
- ACK 只确认 packet 被协议层接受，不代表业务 payload 内容正确。

协议层只交付 bytes；业务层如果有自己的命令格式、校验、版本或 schema，应该在业务层判断。

## Receive 错误和重同步

incoming bytes 是不可信输入。`receive` 必须能处理噪声、半包、粘包、坏 header、坏 integrity tag 和重复包。

一个重要原则是：不要过度丢弃。

当 wire 发现当前位置的 magic 后续 header 不合法时，只能证明“当前位置这个 magic 不是合法包头”，不能证明后面缓存的所有 bytes 都无效。因此 decoder 应该丢掉当前位置的 magic，然后继续在剩余缓存中寻找下一个 magic。

```text
A5 bad_len bad_crc A5 good_len good_crc packet integrity_tag
^ invalid candidate
                 ^ possible next valid envelope
```

如果这里直接清空 ingress buffer，后面已经到达的合法 envelope 也会被丢掉。只丢当前位置的 magic，才能在坏数据后尽快 resync。

不同异常的处理边界不同：

- magic 前面的 bytes 是 noise，可以跳过。
- header 不合法时，丢掉当前位置 magic，然后重新扫描。
- length 指向的完整 envelope 还没有到齐时，返回 `Incomplete`，继续保留缓存。
- envelope integrity tag 错时，丢掉这个完整候选 envelope。
- envelope 合法但 packet 格式 malformed 时，返回 `Corrupted` 或 `Error`，这个 packet 不进入可靠性状态。

如果 packet/reassembly 仍能判断为“当前 packet 不可信”，engine 应该尽量返回 `Corrupted` 并继续保留 session；如果内部状态已经不可继续信任，才返回 `Error`。endpoint 收到真正的 engine error 或 `SendFailed` 后会丢弃旧 engine，下一次连接创建新 session。

这些行为保证 engine 可以同时支持 byte-by-byte 输入、半包输入和一次收到多个 packet 的输入。

## 时间

engine 需要外部传入 `now_ms`，因为协议核心不应该依赖系统时间、硬件 timer 或 runtime clock。

时间只用于协议判断，例如：

- in-flight packet 是否超时
- 是否需要重传
- reassembly slot 是否过期

外部使用毫秒、tick 或其它单调时间源都可以，只要同一个 engine 实例内保持单调语义。

## 内部状态

`Engine` 是 facade，`EngineState` 保存内部协议状态。

这个分层的目的是让 [src/engine.rs](/Users/ancient/src/rust/msrt/src/engine.rs) 保持外部入口清晰。保存协议状态的代码放在 [src/engine/state.rs](/Users/ancient/src/rust/msrt/src/engine/state.rs) 和 `engine/state/` 内部模块；packet 编码、解码和 envelope 组装这类无状态 glue 放在 `engine/codec/`。

`EngineState` 不是一个“大 machine”的抽象名字，而是多个内部状态机的聚合。它不直接保存裸的 message id、dedup table、decoder buffer 或 event queue，而是把这些状态收进对应的 state 对象：

- `ClockState` 负责保存 engine 当前看到的单调时间。
- `NumberState` 负责分配 message id。
- `SchedulerState` 负责输出动作优先级、去重和排队。
- `AckState` 负责 ACK 观察和 ACK pending 状态。
- `RecoveryState` 负责可靠发送、in-flight packet、ACK 应用、RTO 和 retry limit。
- `IngressState` 负责 byte stream 到 packet bytes 的恢复。
- `ReceiveState` 负责 receive-side duplicate suppression。
- `ReassemblyState` 负责 message fragment 到完整 message 的重组。

外部 crate 不应该知道这些内部状态的存在。外部只应该使用 `Engine::send`、`Engine::receive` 和 `Engine::poll`。

## Scheduler 优先级

engine 内部输出不再是一个普通 FIFO 队列。当前调度优先级是：

```text
1. pending ACK
2. control / Pong
3. retransmit
4. local event，例如 SendFailed
5. complete Message
6. new Data
```

ACK 不能被新数据压住。重传也应该优先于新业务数据。这个顺序来自压力测试结论，不是单纯性能优化。

`Ack`、`Pong`、`Retransmit` 不应该被错误去重。尤其 ACK 是不可靠控制包，重复 Data 到来时必须再次 ACK。

## Tracing

库内部诊断日志只通过 `tracing` feature 编译：

```bash
cargo run --features tracing --bin ...
```

库只发出 `tracing::debug!` event，不安装 subscriber，也不决定日志输出格式。文件名、行号、终端输出、文件输出都属于 bin 或应用层配置。

默认 feature 不包含 `tracing`，避免 MCU/普通用户无意中引入日志依赖。
