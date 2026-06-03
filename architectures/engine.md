# Engine

`engine` 是 MSRT 的协议状态机入口。它不是 executor，不是 adapter，也不是 HAL。

## 职责

engine 负责：

- 接收外部输入 bytes
- 接收外部提交的完整 message
- 分配 packet number 和 message id
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
engine.send_on(channel_id, message)
engine.receive(bytes)
engine.poll(now_ms, tx_buf)
```

`poll_event`、`tick`、内部队列、ACK range、reassembly slot、in-flight table 都不应该暴露给外部 crate。

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

当前 send 链路是：

```text
Engine::send(message)
  -> Engine::send_on(default_channel, message)
    -> Machine::send_on(config, channel_id, message)
      -> 分配 message_id
      -> 根据 channel 选择 reliability mode
      -> 按 fragment_bytes 拆分 message
      -> 每个 fragment 编码成 packet
      -> 每个 packet 包进 wire envelope
      -> 将 envelope 作为 Write event 放入事件队列
      -> reliable packet 同时保存到 in-flight table
```

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

reliable channel 和 best-effort channel 在 `send` 后的保存策略不同。

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
  -> Machine::receive(config, bytes)
    -> ingress.feed(bytes, Crc16)
      -> append 到 ingress buffer
      -> 查找 envelope magic
      -> 校验 envelope header
      -> 根据 length 等待完整 envelope
      -> 校验 envelope checksum
      -> 输出 packet bytes
    -> decode packet bytes
    -> apply packet to machine state
```

wire 层只负责从连续 bytes 中恢复一个完整 packet bytes。packet 进入 engine 后，engine 才处理 packet 语义：

- `Data` packet 会先根据 `ack_eliciting` 决定是否排队 ACK。
- duplicate packet 会被 ACK，但不会再次进入 message reassembly。
- message fragment 会进入 reassembly buffer。
- 如果 fragment 已经拼成完整 message，`receive` 会把 `EngineOutput::Message` 放入内部事件队列。
- `Ack` packet 会更新 in-flight packet 状态。

这意味着 `receive` 不直接把完整 message 返回给应用，但它会推进 message reassembly。完整 message 最终通过后续 `poll()` 返回：

```text
receive(data packet fragment)
  -> reassembly complete
  -> queue EngineOutput::Message

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
- 校验 envelope 边界和 checksum。
- 解出 packet。
- 将 packet 应用到协议状态。
- 把需要外部执行的结果排进事件队列。

## Receive 错误和重同步

incoming bytes 是不可信输入。`receive` 必须能处理噪声、半包、粘包、坏 header、坏 checksum 和重复包。

一个重要原则是：不要过度丢弃。

当 wire 发现当前位置的 magic 后续 header 不合法时，只能证明“当前位置这个 magic 不是合法包头”，不能证明后面缓存的所有 bytes 都无效。因此 decoder 应该丢掉当前位置的 magic，然后继续在剩余缓存中寻找下一个 magic。

```text
A5 bad_len bad_crc A5 good_len good_crc packet crc16
^ invalid candidate
                 ^ possible next valid envelope
```

如果这里直接清空 ingress buffer，后面已经到达的合法 envelope 也会被丢掉。只丢当前位置的 magic，才能在坏数据后尽快 resync。

不同异常的处理边界不同：

- magic 前面的 bytes 是 noise，可以跳过。
- header 不合法时，丢掉当前位置 magic，然后重新扫描。
- length 指向的完整 envelope 还没有到齐时，返回 `Incomplete`，继续保留缓存。
- envelope checksum 错时，丢掉这个完整候选 envelope。
- envelope 合法但 packet 格式 malformed 时，返回 `Error`，这个 packet 不进入状态机语义处理。

这些行为保证 engine 可以同时支持 byte-by-byte 输入、半包输入和一次收到多个 packet 的输入。

## 时间

engine 需要外部传入 `now_ms`，因为协议核心不应该依赖系统时间、硬件 timer 或 runtime clock。

时间只用于协议判断，例如：

- in-flight packet 是否超时
- 是否需要重传
- reassembly slot 是否过期

外部使用毫秒、tick 或其它单调时间源都可以，只要同一个 engine 实例内保持单调语义。

## Machine

`Engine` 是 facade，`Machine` 保存内部协议状态。

这个分层的目的是让 `engine.rs` 保持入口清晰，而把 ACK、ingress、outgoing、reassembly、retransmit 等状态逻辑放入内部模块。

外部不应该知道 `Machine` 的存在。
