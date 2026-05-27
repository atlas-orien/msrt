# srt-frame 设计

`srt-frame` 负责把 `srt-core` 中的 `Packet` 转换成串口类字节流可以传输的 frame bytes，并且从连续字节流中恢复 packet。

它是 Packet 与 raw bytes 之间的边界。它不负责 runtime 调度，不负责 ack 策略，不负责重传算法，也不绑定任何 UART、DMA、tokio 或操作系统 API。

## 职责

`srt-frame` 应该包含：

- frame wire format 定义
- packet encode
- packet decode
- CRC16
- 半包处理
- 粘包处理
- 错误包丢弃
- 字节流 resync
- caller-provided output buffer
- `heapless` decoder buffer

`srt-frame` 不应该包含：

- UART driver
- DMA driver
- OS serial API
- async runtime
- ack 语义
- retransmit 语义
- stream 调度
- runtime 状态机

## Packet 与 Frame 的关系

`Packet` 是协议语义结构，属于 `srt-core`。

`Frame` 是字节流边界，属于 `srt-frame`。

传输方向：

```text
Packet
  -> srt-frame encode
  -> frame bytes
  -> raw serial-like byte stream
```

接收方向：

```text
raw serial-like byte stream
  -> srt-frame decoder
  -> frame bytes
  -> Packet
```

## 初版 Wire Format 思路

当前先定义一个紧凑、容易 resync 的初版 frame 格式。它不是最终冻结格式，但应该足够指导代码结构。

```text
+--------+---------+--------+-------------+-----------+
| magic  | length  | header | payload     | crc16     |
| u8     | u16     | bytes  | length bytes| u16       |
+--------+---------+--------+-------------+-----------+
```

字段含义：

- `magic`：frame 起始同步字节，用于从乱流中重新定位。
- `length`：header + payload 的长度，不包含 magic、length 自身和 crc16。
- `header`：由 `srt-core::PacketHeader` 编码而来。
- `payload`：packet payload 原始字节。
- `crc16`：覆盖 length、header、payload，用于丢弃损坏 frame。

这个格式暂时选择固定长度 `length: u16`，是因为串口消息可能超过 255 bytes，但第一版仍然保持简单。后续如果需要更接近 QUIC 的紧凑 varint，可以在版本化 header 中演进。

## Packet Header Encoding

`srt-core` 当前 header 结构：

```text
kind: PacketKind
stream_id: StreamId
seq: Seq
flags: Flags
```

初版编码可以先保持固定宽度：

```text
+--------+----------+--------+-------+
| kind   | stream_id| seq    | flags |
| u8     | u16      | u32    | u8    |
+--------+----------+--------+-------+
```

总计 8 bytes。

注意：这只是 frame 层的 wire encoding，不改变 `srt-core` 中的语义结构。

## Decoder 状态

Decoder 面向连续字节流，必须处理：

- 收到半个 frame，继续等待。
- 一次收到多个 frame，逐个产出。
- 收到坏 CRC frame，丢弃并继续 resync。
- 收到无效 magic，跳过直到下一个 magic。
- buffer 空间不足，返回错误。

初版 decoder 可以定义这些状态：

```text
NeedMore
FrameReady
Resynced
Discarded
```

真正的实现应该避免 `Vec`，内部缓存使用 `heapless::Vec` 或 caller-provided storage。

## Encoder 约束

Encoder 不应该分配内存。

推荐接口形态：

```rust
fn encode_packet<'a>(&mut self, packet: Packet<'_>, out: &'a mut [u8]) -> Result<&'a [u8]>;
```

调用者提供输出 buffer。输出 slice 指向写入完成的 frame bytes。

如果 buffer 不够，返回 `Error::buffer_too_small()`。

## CRC16

CRC16 属于 `srt-frame`。

`srt-core` 不应该知道 checksum 的存在。Runtime 和 reliability 也不应该直接依赖 CRC 细节。

CRC 覆盖范围建议为：

```text
length || header || payload
```

不覆盖 `magic`，这样 decoder 可以先用 magic 做同步，再用 CRC 验证 frame 内容。

## 目录结构

`srt-frame` 也应该让目录表达结构：

```text
srt-frame/src/
├── lib.rs
├── frame.rs
├── codec.rs
├── crc.rs
└── codec/
    ├── traits.rs
    ├── encoder.rs
    ├── decoder.rs
    ├── encoder/
    │   ├── header.rs
    │   └── packet.rs
    └── decoder/
        ├── state.rs
        └── buffer.rs
```

建议含义：

- `frame.rs`：frame-level 常量和 borrowed frame 结构。
- `codec.rs`：codec 模块入口。
- `codec/traits.rs`：encoder/decoder trait。
- `codec/encoder.rs`：encoder 模块入口。
- `codec/encoder/header.rs`：PacketHeader 到 bytes 的编码。
- `codec/encoder/packet.rs`：Packet 到 frame bytes 的编码。
- `codec/decoder.rs`：decoder 模块入口。
- `codec/decoder/state.rs`：decoder 状态枚举。
- `codec/decoder/buffer.rs`：heapless buffer 边界。
- `crc.rs`：CRC16 trait 和默认实现。

后续实现时可以根据复杂度继续调整，但不要把所有内容堆在 `lib.rs`。

## 当前阶段

当前阶段先定义边界和最小 wire format，不实现复杂 resync 算法。

第一步实现目标应该是：

- 定义 frame 常量。
- 定义 encode/decode trait。
- 定义 header encoding 函数边界。
- 定义 CRC16 边界。
- 定义 decoder 状态。
- 添加最小测试保护 header length、flags size、buffer size 等基础约束。
