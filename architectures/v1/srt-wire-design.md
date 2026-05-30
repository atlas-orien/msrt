# srt-wire 设计

`srt-wire` 是 SRT 的串口字节流边界 crate。

它负责把 SRT 协议对象放到原始字节流里传输，也负责从原始字节流中恢复出完整的 SRT Packet。

这里的 wire 不是 protocol frame。

SRT 中的 `Frame` 已经有明确含义：它是 `Packet Payload` 内的协议语义单元，例如 `STREAM`、`ACK`、`PING`。

因此串口外层边界不能再叫 frame，应该叫：

```text
Serial Envelope
Wire Envelope
Wire Packet Boundary
```

当前建议 crate 名称：

```text
srt-wire
```

## 位置

SRT 当前核心分层：

```text
srt-core
  定义 Packet、Packet Header、Packet Number、Protocol Frames。

srt-reliability
  定义 ack、重传、超时、去重、窗口、部分可靠性策略。

srt-runtime
  组织发送、接收、响应、tick、事件交付。

srt-wire
  定义 Packet 在串口类字节流上的 envelope、编码、解码和重同步边界。
```

完整数据流：

```text
Application Message
  -> STREAM Frame fragments
  -> Packet Payload
  -> SRT Packet
  -> Wire Envelope
  -> Serial Byte Stream
```

接收方向：

```text
Serial Byte Stream
  -> Wire Envelope Decoder
  -> SRT Packet
  -> Protocol Frames
  -> Runtime / Reliability
  -> Complete Message
```

## 为什么需要 wire 层

串口、USB CDC、SPI 字节流、TCP mock 等底层链路都可能表现为连续 bytes。

连续 bytes 没有天然 packet 边界。

因此必须解决：

- 从哪里开始读一个 packet？
- packet 有多长？
- bytes 是否损坏？
- 半个 packet 怎么缓存？
- 多个 packet 粘在一起怎么拆？
- 噪声或丢字节后怎么重新同步？

这些问题不属于 `srt-core`。

`srt-core` 只定义协议对象。

这些问题也不属于 `srt-runtime`。

`srt-runtime` 只驱动协议状态机。

所以需要独立的 `srt-wire`。

## 与 Packet / Frame 的关系

必须严格区分：

```text
Packet
  SRT 协议传输单元，属于 srt-core。

Protocol Frame
  Packet Payload 内的语义单元，属于 srt-core。

Wire Envelope
  串口字节流外层边界，属于 srt-wire。
```

结构关系：

```text
Wire Envelope
└── SRT Packet
    ├── Packet Header
    ├── Packet Number
    └── Packet Payload
        ├── STREAM Frame
        ├── ACK Frame
        └── PING Frame
```

注意：不是 Packet 在 Frame 里面，也不是 Wire Envelope 是 Protocol Frame。

## Wire Envelope 初步形态

第一阶段不冻结最终 wire format。

但 wire envelope 未来大概率需要这些字段：

```text
magic
  用于识别 packet 起点和 resync。

version
  用于协议 wire format 兼容。

header_len
  用于扩展 envelope header。

packet_len
  表示完整 encoded SRT Packet 长度。

flags
  wire 层 flags，和 Packet Header Flags 不是同一个东西。

checksum
  用于检测 wire bytes 损坏。

packet_bytes
  encoded SRT Packet。
```

示意：

```text
Wire Envelope
├── magic
├── version
├── header_len
├── packet_len
├── flags
├── packet_bytes
└── checksum
```

这个结构只是设计方向，不是最终定稿。

## Magic

`magic` 用于从连续 byte stream 中寻找 packet 边界。

它的职责是：

```text
bytes lost
noise inserted
decoder state broken
  -> scan magic
  -> recover next possible envelope
```

`magic` 不应该出现在 `PacketHeader` 中。

因为 `PacketHeader` 是协议对象 header，而 `magic` 是 wire envelope 的同步字段。

## Length

wire 层需要 length。

原因是串口是 byte stream，接收端必须知道完整 envelope 需要多少 bytes。

length 可以拆成：

```text
envelope header length
packet length
payload length
```

当前阶段不决定到底使用哪一种组合。

但原则是：wire decoder 必须能在没有堆分配的情况下判断：

```text
还需要多少 bytes 才能组成一个完整 envelope？
```

## Checksum

wire 层需要 checksum。

当前可以先选择 CRC16 作为目标方向，因为 MCU 实现简单、成本低。

但文档阶段不强行冻结 CRC16 多项式。

checksum 只验证 wire bytes 是否损坏。

它不表达 ACK、不表达可靠性、不表达 message 是否完整。

## Half Packet 与 Sticky Packet

wire decoder 必须处理半包：

```text
read #1: envelope 前半部分
read #2: envelope 后半部分
  -> decode 出一个完整 SRT Packet
```

wire decoder 也必须处理粘包：

```text
read #1: envelope A + envelope B + envelope C 的一部分
  -> decode A
  -> decode B
  -> 保留 C 的部分 bytes
```

这两个问题是 wire 层核心职责。

runtime 不应该自己处理半包和粘包。

## Resync

当 decoder 遇到坏数据时，应该进入 resync。

坏数据可能来自：

- magic 不匹配
- length 非法
- checksum 错误
- envelope header 不完整
- packet bytes 不完整

resync 的目标不是修复坏 packet，而是尽快找到下一个可信 envelope 起点。

第一阶段只定义状态，不实现完整扫描算法。

## no_std 与内存模型

`srt-wire` 必须是 `no_std`。

它不应该使用 `Vec`。

它不应该要求 heap。

未来如果需要内部缓存，应该优先考虑：

```text
heapless
固定容量 buffer
调用方提供的 mutable slice
```

第一阶段可以先只定义 trait、状态和边界，不引入 buffer 实现。

## Encoder / Decoder

`srt-wire` 至少需要两个方向：

```text
Encoder
  Packet -> Wire Envelope bytes

Decoder
  Wire bytes -> Packet
```

但这两个方向不能混淆职责。

Encoder 不负责：

- 生成 ACK
- 判断重传
- 分配 PacketNumber
- 创建 STREAM Frame

Decoder 不负责：

- 处理 ACK
- 做去重
- 做 message reassembly
- 触发 runtime event

这些都属于 runtime 和 reliability。

## 与 runtime 的连接

未来 runtime 可能这样连接 wire：

```text
runtime produces Packet
  -> wire encoder writes bytes
  -> RawLink writes bytes

RawLink reads bytes
  -> wire decoder produces Packet
  -> runtime receives Packet
```

这意味着 `srt-runtime` 最终可能更适合依赖抽象：

```text
PacketReader
PacketWriter
```

而不是直接操作 raw bytes。

当前阶段可以先不修改 runtime，等 wire 边界稳定后再调整。

## 错误边界

wire 层错误应该使用 `srt-error` 的共享错误面。

典型错误：

```text
Malformed
  wire bytes 不符合 envelope 格式。

BufferTooSmall
  输出 buffer 不够。

Frame
  当前 ErrorKind 中已有 Frame，但未来可能需要重命名或新增 Wire。

Unsupported
  遇到不支持的 wire version 或 flags。
```

这里有一个后续设计点：

```text
ErrorKind::Frame
```

现在名字可能不够准确，因为 Frame 已经是 protocol frame。未来可以考虑改成：

```text
ErrorKind::Wire
```

或者同时保留：

```text
ProtocolFrame
Wire
```

当前阶段先记录，不急着修改。

## 不属于本 crate 的内容

`srt-wire` 不负责：

- STREAM Frame 语义
- ACK 语义
- 重传
- 去重
- 滑动窗口
- message reassembly
- runtime event
- UART driver
- DMA driver
- tokio
- CLI
- 上层 protobuf/postcard/CBOR 编解码

它只负责：

```text
SRT Packet <-> Wire Envelope bytes
```

## 当前建议目录结构

未来 crate 可以这样组织：

```text
srt-wire/src/
├── lib.rs
├── envelope.rs
├── envelope/
│   ├── header.rs
│   ├── flags.rs
│   └── magic.rs
├── codec.rs
├── codec/
│   ├── encoder.rs
│   └── decoder.rs
├── checksum.rs
├── checksum/
│   └── crc16.rs
└── resync.rs
```

其中：

```text
envelope
  定义 wire envelope 的结构。

codec
  定义 encode/decode 边界。

checksum
  定义校验边界。

resync
  定义重新同步状态。
```

## 第一阶段结论

第一阶段的 `srt-wire` 应该做到：

1. 明确 wire envelope 和 protocol frame 不是一回事。
2. 明确 magic、length、checksum 属于 wire 层，不属于 Packet Header。
3. 定义 encoder / decoder / checksum / resync 的边界。
4. 保持 `no_std`。
5. 不使用 `Vec`。
6. 不实现完整 wire format。

`srt-wire` 是 SRT 能够真正跑在串口 byte stream 上的关键层，但现在仍然应该先冻结边界，再写代码。
