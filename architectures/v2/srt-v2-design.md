# SRT v2 总设计

v2 的核心目标是从 v1 的架构骨架进入第一版 wire format 设计。

v1 已经明确：

```text
core
  定义 Packet / Protocol Frame。

wire
  定义 Packet 与 byte stream 的边界。

reliability
  定义 ACK / 重传 / 去重 / 窗口策略。

engine
  驱动发送、接收、tick、event。
```

v2 需要开始冻结这些对象如何编码成 bytes。

## v2 的中心问题

v2 要回答：

```text
一个完整 SRT message 如何最终变成串口字节？
```

目标路径：

```text
Application Message
  -> STREAM Frame
  -> Packet Payload
  -> Packet Header + Packet Payload
  -> Wire Envelope
  -> Serial Byte Stream
```

接收路径：

```text
Serial Byte Stream
  -> Wire Envelope Decoder
  -> Packet
  -> Protocol Frames
  -> Engine / Reliability
  -> Application Message
```

## v2 设计原则

- 仍然 `no_std`。
- 不要求 heap。
- 不使用 `Vec` 作为协议实现基础。
- 所有长度字段必须有明确上限。
- 字节序必须统一，建议使用 little-endian。
- wire 层和 protocol frame 层继续严格区分。
- magic / crc / resync 只属于 wire 层。
- `stream_id` 只属于 STREAM Frame。
- ACK 确认 PacketNumber，不表示 message 已经完整交付。
- v2 先设计 draft，不承诺最终兼容性。

## v2 范围

v2 应该覆盖：

- Wire Envelope layout
- Packet Header layout
- Protocol Frame layout
- STREAM Frame layout
- ACK Frame layout
- PING Frame layout
- RESET_STREAM Frame layout
- CRC16 参数
- Decoder 状态机
- Encode / decode 错误模型
- Smoke simulation 升级方向

## v2 非范围

v2 不应该直接实现：

- engine 完整状态机
- retransmit 完整算法
- heapless reassembly buffer
- MCU UART driver
- tokio adapter
- CLI monitor
- 多语言 SDK

## v2 关键产物

v2 结束时应该有：

1. 第一版 wire format 文档。
2. `srt-core` frame 编码边界设计。
3. `srt-wire` envelope 编码边界设计。
4. 一组 table-driven encode/decode 测试计划。
5. smoke simulation 对齐真实 wire layout。

## 推荐推进顺序

1. 先冻结 Wire Envelope。
2. 再冻结 Packet Header。
3. 再冻结 Protocol Frame Header。
4. 再定义每种 Frame 的字段编码。
5. 再实现 encoder。
6. 再实现 decoder。
7. 最后升级 smoke simulation。

这个顺序可以避免 engine 提前牵引底层格式。
