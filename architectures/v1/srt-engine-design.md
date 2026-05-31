# srt-engine 设计

`srt-engine` 是 SRT 的协议引擎边界 crate。

这里的 engine 不是操作系统执行器，也不是 tokio executor，更不是 MCU HAL。它是 SRT 协议本身如何被驱动的抽象层。

`srt-engine` 负责回答一个核心问题：

```text
当上层要发送一条 message，或者底层收到一段 bytes 时，SRT 协议应该如何推进？
```

当前阶段只冻结设计边界，不实现完整协议状态机。

## 位置

SRT 当前分层：

```text
srt-core
  定义 Packet、Packet Header、Packet Number、Protocol Frames。

srt-reliability
  定义 ack、重传、超时、去重、窗口、部分可靠性策略。

srt-engine
  组织发送、接收、响应、tick、事件交付。

Serial Envelope / Wire Boundary
  后续负责 Packet 与串口字节流之间的边界、校验和重同步。
```

`srt-engine` 依赖 `srt-core` 和 `srt-reliability` 的概念，但不应该依赖具体硬件、OS、async executor 或堆分配模型。

## Engine 是什么

`srt-engine` 是协议状态机的边界。

它应该理解：

- 上层 message
- stream 路由
- Packet Number 分配
- STREAM Frame 生成
- ACK Frame 生成
- PING 响应
- RESET_STREAM 处理
- packet 去重
- packet ack
- retransmit tick
- message fragment reassembly
- message delivery event

它不应该直接理解：

- UART 寄存器
- DMA descriptor
- tokio task
- std socket
- GitHub CI
- CLI
- 具体 MCU HAL

## Engine 与其它 crate 的关系

可以这样理解三个核心 crate：

```text
srt-core
  定义协议语言。

srt-reliability
  定义可靠性判断。

srt-engine
  使用协议语言和可靠性判断，驱动通信过程。
```

换句话说：

```text
core 告诉 engine：协议对象长什么样。
reliability 告诉 engine：哪些 packet 应该 ack、重传、丢弃。
engine 决定：什么时候发送、什么时候响应、什么时候交付。
```

## 发送路径

发送路径从一条完整上层 message 开始。

上层看到的是：

```text
send(message bytes)
```

`send` 是非阻塞 API。它不等待 ACK，也不等待链路写完。它只把完整 message 放进 engine，并让 engine 产生后续输出事件。

engine 内部需要逐步变成：

```text
message bytes
  -> stream routing
  -> message_id allocation
  -> STREAM Frame fragments
  -> Packet payload
  -> Packet
  -> wire bytes
  -> queue Write events
```

也就是说，外部用户只调用一次：

```text
endpoint.send(message)
```

而不是：

```text
for fragment in message.chunks(...) {
  endpoint.send(fragment)
}
```

拆分 message、生成多个 packet、维护 packet number、等待 ACK、未来触发重传，都是 engine 内部职责。

注意：当前 `srt-core` 中 `PacketPayload` 暂时还是 borrowed bytes，表示 encoded protocol frames。真正的 frame 编码格式还没有冻结。

因此第一阶段 engine 只需要定义发送意图和边界，不应该提前实现最终 packet/frame 编码。

## 接收路径

接收路径从底层收到的数据开始。

未来完整路径应该是：

```text
raw bytes
  -> Serial Envelope decode
  -> SRT Packet
  -> Protocol Frames
  -> reliability processing
  -> message fragment reassembly
  -> complete message event
```

当前阶段 wire 层还没有定义，所以 `srt-engine` 只需要保留接收入口和事件出口。

后续当 wire 层出现时，engine 不应该自己处理：

- magic
- length
- crc
- resync
- half packet
- sticky packet

这些属于 Serial Envelope / Wire Boundary。

## 非阻塞 receive 与 ingress pipeline

高层 `receive(&mut link)` 必须是非阻塞的。

它不应该等待完整 message 才返回，也不应该在 MCU 主循环里卡住等待更多 UART bytes。

用户不应该关心一次要读多少 bytes。

因此对外 API 更适合是：

```text
receive(&mut link)
```

其中 `link` 是一个 `no_std` 的非阻塞链路抽象，例如 UART、USB CDC、DMA ring 或测试 link。

内部可以保留一个低层入口：

```text
feed(bytes)
```

`feed(bytes)` 只作为 engine / wire decoder 的内部增量输入，不是普通用户主要入口。

`receive(&mut link)` 的语义应该是：

```text
从 link 中非阻塞读取当前已经到达的 bytes
  -> 把 bytes 喂给内部 feed(bytes)
  -> 尽可能推进内部 decoder / packet / frame / reliability 状态
  -> 让内部状态重新稳定
  -> 立即返回
```

这里的稳定不是指已经得到完整 message，而是指 engine 已经处理完当前输入，后续需要等待新的输入、tick 或外部 poll。

MCU 场景中，输入可能是：

```text
第 1 次 receive(&mut link): link 只读出 wire header 前 3 bytes
第 2 次 receive(&mut link): link 读出剩余 header
第 3 次 receive(&mut link): link 读出 packet body 一半
第 4 次 receive(&mut link): link 读出 packet body 剩余部分 + 下一个 packet 的一部分
```

所以 engine 内部需要一个 ingress pipeline：

```text
receive(&mut link)
  -> link.read(rx_buf)
  -> feed(bytes)

feed(bytes)
  -> wire decoder feed
  -> while packet ready:
       handle_packet(packet)
  -> while frame ready:
       handle_frame(frame)
  -> if message complete:
       queue EngineEvent::MessageReceived
  -> if ACK/write needed:
       queue EngineEvent::LinkWrite / AckRequired
  -> return progress
```

完整 message 的通知不应该由 `receive()` 直接阻塞返回。

完整 message 应该通过事件输出：

```text
receive(&mut link)
poll_event()
  -> MessageReceived
```

ACK、重传、需要写到底层链路的数据也应该通过事件输出。

这样 `send`、`receive`、`tick`、`poll_event` 的角色就很清楚：

```text
send
  应用层输入一条完整 message，内部自动拆 packet，立即返回。

receive
  从链路层非阻塞读取或接收 bytes，并推进 ingress pipeline。

tick
  时间输入，用于 ACK 延迟、超时和重传。

poll_event
  协议输出，交付 message、请求写链路、请求下一次唤醒。
```

这也是 `no_std` MCU 模型的核心：SRT 不启动任务，不拥有线程，不阻塞等待 IO；SRT 只维护一个长期存活的 engine state，由外部 loop 驱动。

底层 link trait 可以类似：

```text
trait LinkRead {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}

trait LinkWrite {
    fn write(&mut self, bytes: &[u8]) -> Result<usize>;
}
```

其中 `read` 必须是非阻塞语义：

```text
没有数据
  -> Ok(0) 或 WouldBlock 类状态。

有数据
  -> 读出当前可用 bytes，立即返回。
```

所以最终模型是：

```text
用户 API
  receive(&mut link)

内部 API
  feed(bytes)
```

这样用户不用知道一次应该接收多少 bytes，同时 engine 仍然保持 `no_std`，不依赖具体 UART 或操作系统。

## Event Driven

SRT 是 message-driven 和 actor/engine friendly 的协议。

engine 不应该强制调用方使用阻塞模型，也不应该强制调用方使用 async 模型。

更合适的边界是事件驱动：

```text
send(message)
receive(&mut link)
tick(now)
poll_event()
```

engine 可以产生事件：

```text
MessageReceived
  完整 message 已经可以交付给上层。

LinkWrite
  有协议数据需要写到底层链路。

MessageAcked
  一条可靠 message 的必要 packet 已经被确认。

MessageFailed
  一条 message 因超过重传次数、deadline 或资源限制而失败。

AckRequired
  收到 ack-eliciting packet，需要生成 ACK。

Retransmit
  某个 packet 需要重传。

WakeAt
  engine 需要在某个时间点再次 tick。
```

这些事件不绑定具体执行方式。

MCU 可以在主循环或中断后半部里 poll。

OS 可以在线程、epoll、tokio 或其它 async executor 里 poll。

## Time 与 Tick

`srt-engine` 不能直接依赖系统时间。

原因是 MCU、RTOS、裸机和 OS 的时间来源完全不同。

engine 应该只接受外部传入的单调时间值：

```text
tick(now)
```

其中 `now` 的单位可以由嵌入环境定义。

`srt-reliability` 中的 timeout policy 只做判断，不拥有真实 timer。

## ACK 响应

ACK 是 engine 的核心职责之一。

当收到一个需要确认的 packet 时：

```text
receive packet
  -> dedup check
  -> process frames
  -> schedule ACK
  -> poll_event produces LinkWrite / AckRequired
```

PING 不需要单独的 PONG Frame。

PING 的响应可以是 ACK。

这和 QUIC 的方向一致，也更适合保持 frame 类型精简。

发送方不应该在 `send(message)` 里阻塞等待 ACK。更合理的方式是：

```text
send(message)
  -> queue Write(packet 0)
  -> queue Write(packet 1)
  -> return

receive(ack bytes)
  -> update in-flight packet state
  -> if message complete:
       queue MessageAcked

tick(now)
  -> if packet timeout:
       queue retransmit Write(packet)
```

所以从外部看，只有一个简单的 `send`，但内部仍然有持续运行的发送状态机。这个状态机由 `receive` 和 `tick` 推进，而不是由阻塞循环推进。

## 重传

重传由 engine 驱动，但由 reliability 策略判断。

流程大致是：

```text
packet sent
  -> engine 记录 in-flight packet
  -> tick(now)
  -> timeout policy 判断是否超时
  -> retransmit policy 判断是否重传
  -> engine 产生 Retransmit 事件
```

当前阶段不实现 in-flight buffer。

因为这会牵涉：

- 是否允许 heap
- heapless 容量如何配置
- packet bytes 是否复制
- message fragment 是否重新编码
- 旧 message 是否可被新 message 替换
- deadline 过期后如何丢弃

这些需要在 engine 代码边界更清楚后再冻结。

## Message Reassembly

SRT 是 message-oriented，不是无限 byte-stream。

engine 最终需要负责把 STREAM Frame fragments 重组成完整 message。

核心 key 是：

```text
stream_id + message_id
```

完整性判断依赖：

```text
message_len
fragment_offset
data.len()
```

当收到的 fragment 覆盖：

```text
[0, message_len)
```

engine 才可以交付完整 message bytes。

v1 MVP 可以实现一个很小的固定容量 reassembly buffer，用来验证外部 API 是否正确。

这个 buffer 不是最终算法，只证明边界：

```text
receive(packet fragment 0)
  -> no Message event

receive(packet fragment 1)
  -> no Message event

receive(packet fragment N)
  -> queue MessageReceived
```

最终版本还需要处理乱序、重复、丢包、多个并发 message、多个 stream 和资源回收。

## StreamId 与用户 API

协议 wire format 必须携带 `stream_id`。

但用户 API 不一定必须直接传 `stream_id`。

engine 可以支持两种层次：

```text
低层 API
  send(stream_id, message)

高层 API
  send_topic("imu", message)
  send_actor(actor_id, message)
  send_control(message)
```

高层 API 可以由 engine 或上层封装映射到 `StreamId`。

第一阶段 crate 先保留低层 `SendOptions { stream_id }`，避免提前设计 topic/actor 系统。

## RawLink 边界

`RawLink` 是 engine 与外部字节链路之间的最小抽象。

它可以由很多实现承载：

```text
UART
USB CDC
SPI transport
TCP mock
test buffer
```

但 `RawLink` 本身不属于最终协议 wire format。

未来如果设计独立 wire crate，engine 可能不会直接面对 raw bytes，而是面对：

```text
PacketReader
PacketWriter
```

因此当前 `RawLink` 只是临时且保守的边界。

## 不属于本 crate 的内容

`srt-engine` 不负责：

- Packet / Frame 数据结构定义
- CRC
- magic
- serial resync
- UART driver
- DMA driver
- embedded-hal adapter
- tokio adapter
- CLI
- 完整可靠性算法
- 完整 wire format 编解码
- 具体 heapless buffer 容量

这些内容应该由 `srt-core`、`srt-reliability`、后续 wire 层和环境适配层分别处理。

## 当前目录结构

当前 `srt-engine` 已经按协议引擎边界拆分：

```text
srt-engine/src/
├── lib.rs
├── event.rs
├── link.rs
├── message.rs
├── receive.rs
├── engine.rs
├── scheduler.rs
├── send.rs
└── time.rs
```

其中 `scheduler.rs` 只定义未来唤醒边界，不实现真正调度器。第一阶段不应该引入 mailbox。

## 第一阶段结论

第一阶段的 `srt-engine` 应该做到：

1. 明确 engine 是协议状态机边界，不是 OS executor。
2. 定义发送、接收、tick、poll event 的最小接口。
3. 明确 engine 负责 ACK 响应、重传驱动、message reassembly 的组织。
4. 不实现 serial wire codec。
5. 不绑定 std、tokio、embedded-hal 或具体 MCU。

`srt-engine` 是 SRT 协议真正“活起来”的地方，但当前阶段只需要把骨架立稳。
