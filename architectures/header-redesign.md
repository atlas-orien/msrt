# Packet Header 改版记录

这份文档记录 MSRT 从通用 packet header 改成 kind-specific header 的过程和原因。

这次改版的第一目标不是“为了重构”，而是减少包头开销。重构只是为了让包头缩小后，代码边界仍然正确、清晰、可长期维护。

## 背景

早期实现里，Data、Ack、Ping、Pong、Log 共享接近同一套 packet header 思路。这样写代码很方便，因为所有 packet 都可以放进一个通用 `PacketHeader` 里处理。

但这个设计有两个问题：

- 小控制包也会背负 Data message 才需要的字段。
- 代码会误以为所有 packet 都有 message、packet index、payload 或 channel 语义。

对 MSRT 这种面向 MCU、UART、USB CDC、UDP 等链路的协议来说，包头不是小问题。ACK、Ping、Pong 这类包 payload 很小，如果固定 header 太大，传输效率会被持续浪费。

## 原始问题

旧设计里存在几个不干净的边界：

```text
packet identity:
    channel_id + message_id + packet_index

ack model:
    global packet stream / ACK range

public send:
    send_on(channel_id, message)

packet header:
    所有 packet 共享一套字段视图
```

这些边界会把 MSRT 拉向“全局 packet stream”模型。但 MSRT 真正的思想是 message-oriented：

```text
message:
    message_id

packet:
    message 内部的 fragment

packet identity:
    message_id + packet_index
```

一旦边界说错，包头就会自然变大。比如为了 channel 存字段、为了全局 packet stream 存 range、为了 Ping/Pong 兼容通用 header 保留无意义的 message id。

## 新包头设计

当前 wire header 按 packet kind 分开：

```text
Data:
kind             1B
flags            1B
message_id       4B
packet_index     2B
message_len      2B
fragment_offset  2B
--------------------
total           12B

Log:
kind             1B
message_id       4B
packet_index     2B
message_len      2B
fragment_offset  2B
--------------------
total           11B

Ack:
kind             1B
message_id       4B
packet_index     2B
--------------------
total            7B

Ping/Pong:
kind             1B
```

核心结论：

- Data 才需要可靠传输字段。
- Log 是 best-effort message，也需要 message fragment 字段，但不需要 ACK flag。
- Ack 只确认一个 Data packet，所以只需要 `message_id + packet_index`。
- Ping/Pong 只表达保活语义，不需要 message id，也不需要 payload。

## Channel 删除

这次改版删除了协议层 `ChannelId`。

原因不是 channel 概念完全没用，而是它不应该属于 MSRT packet header 的固定成本。应用层如果需要区分消息类型、业务通道、命令类型，可以放进 payload 自己的应用协议里。

MSRT 协议层只关心：

```text
packet kind:
    Data / Log / Ack / Ping / Pong

message identity:
    message_id

packet identity:
    message_id + packet_index
```

这样每个 packet 不再为应用层路由付固定包头成本。

## ACK 改版

早期 ACK 思路受到 QUIC/TCP 一类 packet stream 模型影响，容易自然长出 ACK range、largest ACK、global packet number。

但 MSRT 不是全局 packet stream。Data packet 的稳定身份是：

```text
message_id + packet_index
```

所以 ACK 被简化成：

```text
AckHeader:
    kind
    message_id
    packet_index
```

这带来两个重要结果：

- ACK 语义变成“确认单个 packet”，没有 range 压缩语义。
- 重复 Data 到来时必须重新 ACK，不能因为已经见过就丢弃 ACK。

压力测试中最关键的稳定性修复也是这个方向：ACK 不去重，duplicate Data 必须 re-ACK。

## Ping/Pong 改版

Ping/Pong 是内部连接保活机制，不是用户 message。

因此 Ping/Pong 不应该有：

- `message_id`
- `packet_index`
- `message_len`
- `fragment_offset`
- payload

当前 Ping/Pong wire header 只有 1B `kind`。

这也同步影响 API 设计：外部用户不能调用 `send_ping`，Ping/Pong 只能由 endpoint 内部自动生成。

## API 边界

外部发送入口缩小为：

```rust
engine.send(message)
engine.send_log(message)
```

含义：

- `send` 发送可靠 Data message。
- `send_log` 发送 best-effort Log message。
- ACK/Ping/Pong 是内部控制包，不属于外部发送 API。

内部实现也避免使用“任意 packet type 都能发送 message”的入口。Data/Log 进入 `send_fragmented_message`，ACK/Ping/Pong 走独立编码路径。

这解释了一个容易误解的点：`send_fragmented_message` 的 `message: &[u8]` 不需要是 `Option`，因为它只处理 Data/Log 这种有 message 语义的 packet。空 payload 控制包不走这里。

## Core 边界清理

为了让包头减少不是表面变化，core 也做了边界收紧：

- 删除 `Packet::new/header + payload` 这类万能构造，避免 ACK/Ping/Pong “传入 payload 后被悄悄丢弃”。
- 删除 `PacketHeader::message_id()`、`packet_index()`、`message_len()`、`fragment_offset()` 这类万能访问器，避免 Ping/Pong 返回假零值。
- `reliability::MessageFragment` 改为从 `DataHeader` 构造，不再接受万能 `PacketHeader`。
- `PacketDecode` 拆成 `Data/Log/Ack/Ping/Pong`，Data 和 Log 各自携带自己的 header。

这些改动看起来像重构，本质上是防止旧抽象把无意义字段重新带回包头设计里。

## 最终边界

当前边界应保持：

```text
Data:
    reliable application message fragment

Log:
    best-effort diagnostic/application log fragment

Ack:
    acknowledge exactly one Data packet

Ping/Pong:
    internal liveness only

application routing:
    payload 自己定义
```

这个边界的好处是：

- 每种 packet 只携带自己真正需要的字段。
- ACK 语义简单，不受 range/global stream 污染。
- Ping/Pong 极小。
- 应用层路由不污染协议固定成本。
- core/engine/reliability 的代码不再需要假装所有 packet 都是同一种结构。

## 经验

这次改版说明：包头优化不是简单删字段。真正要做的是先确认协议边界。

如果边界仍然是错的，代码会很自然地把字段重新加回来；如果边界是对的，包头减少只是结果。

MSRT 当前的设计选择是：

```text
不要全局 packet stream
不要协议层 channel
不要 ACK range
不要 Ping/Pong message id
不要万能 PacketHeader 访问器
```

这些约束应该继续保持，除非未来有新的真实需求能证明它们值得付出固定包头成本。
