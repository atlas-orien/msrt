# SRT v1 Protocol Spec

## 状态

本文档是 SRT v1 protocol spec。

它的目的不是继续探索方向，而是把当前已经验证过的 foundation / hardening / reliable transport 行为整理成可以审核的协议标准边界。

当前状态：

```text
v1 MVP
  已完成。

v1 hardening
  当前范围已完成。

v1 protocol spec
  wire / packet / frame layout 已对齐。
  reliable message transport 当前范围已完成。
  freeze 审核已完成。
  当前是 v1 frozen baseline。
```

本文档中的字段和行为是 v1 可靠传输继续推进的基础。代码如果和本文档不一致，应该优先判断是文档需要修正，还是代码出现了协议漂移。

## v1 Freeze 范围

v1 freeze 的目标是冻结一套最小但完整的 no_std message transport：

- 串口 byte stream 上的 envelope 边界。
- Packet Header。
- MESSAGE Frame。
- ACK Frame。
- reliable message send / receive / retransmit / dedup / reassembly。
- BestEffort 的最小 channel policy。
- 非阻塞 engine API。
- 固定容量 buffer 策略。

v1 freeze 不追求所有长期能力一次到位。只要后续能力能通过新增 frame、policy 或 adapter 扩展，而不破坏 v1 wire format 和 API 边界，就应该留到 v1.1 / v2。

## Freeze 审核结论

v1 freeze 前的关键取舍已经按“最小可靠 no_std message transport”原则审核：

1. 接受 fixed-length ACK Frame。
   原因：no_std 解码简单、固定容量、实现可预测。更紧凑的 ACK encoding 可以留到后续版本，但 v1 先冻结当前 fixed-capacity range 语义。

2. 接受 v1 不引入对端 cancel frame。
   原因：`CANCEL_MESSAGE` 会引入新的 frame 类型和双端状态语义。v1 中本端失败后清理 in-flight，对端 incomplete reassembly 依靠 `reassembly_timeout_ms` 释放。

3. 接受 `LatestOnly` / `Deadline` 留到 v1.1 / v2。
   原因：它们属于 partial reliability 的策略扩展，不是 v1 reliable transport 的必要条件。v1 只冻结 `Reliable` 和 `BestEffort` 的实际行为。

4. 接受当前默认参数作为 reference engine defaults。
   尤其是 `DEFAULT_FRAGMENT_BYTES = 32`。这个默认值保守、适合 MCU/串口调试，并且仍允许用户通过 `EngineConfig` 调整。

5. 接受当前 deterministic long-run integration simulation 作为 v1 freeze 前的软件验收。
   更接近真实设备负载的窗口耗尽、长时间 soak、硬件 UART/DMA 测试放到 v1.1 或 hardware validation，不阻塞 v1 protocol freeze。

因此，本文档后续只应做措辞和一致性修正；除非发现 wire format 或可靠性语义缺陷，否则不再扩展 v1 范围。

## 协议定位

SRT v1 是一套面向串口类 byte stream 的 message-oriented transport。

核心目标：

```text
Application Message
  -> MESSAGE Frame fragments
  -> Packet
  -> Wire Envelope
  -> Serial Byte Stream
  -> ACK / dedup / retransmit
  -> reassemble complete Message
  -> deliver to application
```

SRT 借鉴 QUIC 的 packet number、ACK、去重、重传、channel/priority 思想，但不继承 QUIC 的对象模型。

SRT v1 不是：

- HTTP/3。
- QUIC over serial。
- TCP clone。
- 通用 byte-stream transport。
- 操作系统 runtime。
- MCU HAL。

SRT v1 的第一公民是：

```text
Message
```

不是：

```text
Stream
```

## 分层模型

SRT v1 分成三层：

```text
Wire Envelope
  串口 byte stream 外层边界。
  负责 magic、version、length、CRC、resync、半包、粘包。

Packet
  协议传输单元。
  负责 packet type、packet number、packet flags。

Protocol Frame
  Packet payload 内的语义单元。
  v1 spec 只定义 MESSAGE 和 ACK。
```

结构关系：

```text
Wire Envelope
└── Packet
    ├── Packet Header
    └── Packet Payload
        ├── MESSAGE Frame
        └── ACK Frame
```

## 字节序

SRT v1 spec 统一使用 little-endian。

原因：

- MCU 实现简单。
- 当前 Rust MVP 已经使用 little-endian。
- SRT 主要面向 MCU / robot / drone / local host link，不追求互联网协议传统网络字节序兼容。

所有多字节整数在 wire 上都使用 little-endian。

## 基础整数宽度

SRT v1 spec 定义：

```text
PacketNumber  u32
MessageId     u32
ChannelId     u16
MessageLen    u16
FragmentOffset u16
Flags         u8
Checksum      u16
```

说明：

- `PacketNumber(u32)` 足够 v1 长时间运行和测试。
- `MessageId(u32)` 按 channel 作用域递增。
- `ChannelId(u16)` 用于逻辑通道、QoS、可靠性策略和上层路由。
- `MessageLen(u16)` 使单条 message 最大长度在 v1 中天然受限，适合 MCU。
- `FragmentOffset(u16)` 与 `MessageLen(u16)` 对齐。
- `Flags(u8)` 当前足够，后续不够再扩展。

v1 不使用可变长度整数。

## Public Engine API

v1 标准协议层暴露 concrete no_std engine，不暴露 async runtime，也不拥有线程。

核心驱动 API：

```text
send(message)
send_on(channel_id, message)
receive(bytes)
tick(now)
poll_event()
```

语义：

- `send(message)` 等价于 `send_on(ChannelId::CONTROL, message)`。
- `send_on(channel_id, message)` 接收完整 application message，由 engine 内部分片成多个 packet。
- `receive(bytes)` 只处理当前已经到达的 bytes，不阻塞等待未来 bytes。
- `tick(now)` 推进超时、重传、reassembly timeout 等时间驱动状态。
- `poll_event()` 输出 `Write`、`Message`、`SendFailed`。

v1 不提供：

- async task。
- tokio adapter。
- Embassy / RTIC adapter。
- UART read/write driver。
- blocking `send_and_wait()`。

## Wire Envelope

Wire Envelope 是 SRT 在串口 byte stream 上的外层边界。

它不属于 Protocol Frame。

### Wire Envelope Header

SRT v1 spec header：

```text
offset  size  field
0       2     magic
2       1     version
3       1     header_len
4       2     packet_len
6       1     wire_flags
7       1     reserved
```

固定 header 长度：

```text
WIRE_HEADER_LEN = 8
```

### Magic

SRT v1 spec magic：

```text
"SR"
```

十六进制：

```text
0x53 0x52
```

`magic` 的作用是从连续 byte stream 中重新找到可能的 envelope 起点。

`magic` 不属于 Packet Header。

### Version

SRT v1 spec：

```text
version = 1
```

不支持的 version 必须触发 resync 或 rejected envelope。

### Header Length

SRT v1 spec：

```text
header_len = 8
```

保留 `header_len` 是为了未来扩展 wire envelope header。

v1 实现可以只接受 `header_len == 8`。

### Packet Length

`packet_len` 表示 encoded Packet bytes 的长度，不包含：

- wire header
- checksum

完整 envelope 长度：

```text
total_len = header_len + packet_len + checksum_len
```

SRT v1 spec：

```text
checksum_len = 2
```

### Wire Flags

SRT v1 spec：

```text
bit 0: checksum_present
bit 1..7: reserved
```

v1 必须设置：

```text
checksum_present = 1
```

未识别 reserved bits 的处理策略后续冻结。保守策略是 rejected + resync。

### Checksum

SRT v1 spec 使用 `u16 checksum` 字段。

SRT v1 spec 冻结为：

```text
CRC-16/XMODEM
```

参数：

```text
width   = 16
poly    = 0x1021
init    = 0x0000
xorout  = 0x0000
refin   = false
refout  = false
check   = 0x31c3  // "123456789"
```

选择原因：

- MCU 实现简单。
- no_std 实现不依赖查表也可接受。
- 当前 `srt-wire::Crc16` 已按该参数实现。

## Packet

Packet 是 SRT 的传输单元。

Packet 由 Packet Header 和 Packet Payload 组成。

```text
Packet
├── Packet Header
└── Packet Payload
```

### Packet Header

SRT v1 spec packet header：

```text
offset  size  field
0       1     packet_type
1       1     packet_flags
2       4     packet_number
```

说明：

- `srt-engine` 当前已开始按这个 header layout 编码和解码。
- `packet_number` 必须属于 Packet Header，不属于 MESSAGE Frame。

### Packet Type

SRT v1 spec：

```text
0x00 DATA
0x01 ACK
```

说明：

- DATA packet 可以携带 MESSAGE Frame。
- ACK packet 可以携带 ACK Frame。
- 后续是否允许一个 packet 携带多个 frame，留到 serialization freeze 时定稿。

### Packet Flags

SRT v1 spec：

```text
bit 0: ack_eliciting
bit 1..7: reserved
```

DATA packet 默认应该是 ack-eliciting。

ACK-only packet 可以不是 ack-eliciting，避免 ACK 风暴。

### Packet Number

`PacketNumber(u32)` 是 packet 级可靠性的核心。

用途：

- ACK。
- duplicate packet detection。
- retransmit tracking。
- receive window。

Packet number 在 endpoint 内单调递增，允许 `u32` wrap，但 v1 stable 是否定义 wrap 后窗口行为需要后续单独冻结。

v1 当前不冻结复杂 wrap window 语义。实现必须使用 `PacketNumber(u32)`，但长时间运行到 wrap-around 后的 ACK range / dedup 精确定义留到后续版本。

## Protocol Frames

v1 spec 只定义两个 frame：

```text
MESSAGE
ACK
```

不定义：

```text
PING
PONG
RESET_STREAM
CONNECTION_CLOSE
MAX_DATA
```

这些名字会把 SRT 拉回 QUIC/TCP 对象模型。SRT 后续如需要心跳、取消 message、关闭 channel，应按 SRT 自己的 message runtime 语义命名，例如：

```text
HEARTBEAT
CANCEL_MESSAGE
CLOSE_CHANNEL
```

## MESSAGE Frame

MESSAGE Frame 承载一条 application message 的一个 fragment。

SRT v1 spec MESSAGE frame：

```text
offset  size  field
0       1     frame_type
1       2     channel_id
3       4     message_id
7       2     message_len
9       2     fragment_offset
11      1     message_flags
12      N     fragment_bytes
```

### Frame Type

SRT v1 spec：

```text
0x00 MESSAGE
0x01 ACK
```

### ChannelId

`ChannelId(u16)` 是 message 的逻辑通道。

用途：

- 上层路由。
- QoS。
- reliability policy。
- priority。
- message reassembly namespace。

`ChannelId` 不是 QUIC stream。

它更接近：

```text
topic
lane
mailbox route
message class
```

SRT v1 spec 保留：

```text
channel_id = 0
  control channel
```

其它静态和动态分配规则后续冻结。

v1 当前冻结：

```text
ChannelId::CONTROL = 0
```

应用可以显式使用其它 `ChannelId(u16)`，但动态 channel negotiation 不属于 v1。

### MessageId

`MessageId(u32)` 在 channel 内标识一条 message。

SRT v1 spec：

```text
message key = channel_id + message_id
```

接收端使用 message key 定位 reassembly buffer。

### MessageLen

`MessageLen(u16)` 是完整 application message 的长度。

接收端必须等收到覆盖：

```text
[0, message_len)
```

的所有 fragment 后，才能交付完整 message。

### FragmentOffset

`FragmentOffset(u16)` 是当前 fragment 在完整 message bytes 中的起始位置。

合法性要求：

```text
fragment_offset <= message_len
fragment_offset + fragment_len <= message_len
```

不满足要求的 MESSAGE Frame 必须 rejected。

### Message Flags

SRT v1 spec：

```text
bit 0: first
bit 1: last
bit 2..7: reserved
```

说明：

- `first` 表示该 fragment 从 message 起点开始。
- `last` 表示该 fragment 到达 message 末尾。
- 空 message 可以同时设置 `first` 和 `last`。

接收端不能只依赖 `last` 判断完整 message。必须检查所有 byte range 是否完整覆盖。

接收端也不能要求 `first` fragment 必须最先到达。串口链路上的重传、乱序测试和 sticky decode 都可能让中间 fragment 先被处理。只要 fragment 携带的 `channel_id + message_id + message_len` 一致，接收端可以先创建 reassembly slot，等待缺失 byte range 通过后续 packet 或 retransmit 补齐。

## ACK Frame

SRT v1 spec 定义 fixed-capacity ACK range。

```text
offset  size  field
0       1     frame_type
1       4     largest_acknowledged
5       1     range_count
6       8*N   ack_ranges
```

当前 `N = MAX_ACK_RANGES`，每个 range 固定编码：

```text
offset  size  field
0       4     start_packet_number
4       4     end_packet_number
```

`range_count` 表示前多少个 range 有效。未使用的 range slot 必须编码为零值。

语义：

```text
start <= packet_number <= end
  -> acknowledged
```

当前代码中的 ACK Frame 长度是固定长度，目的是保持 no_std 解码简单。ACK range 生成使用固定容量滑动窗口：

```text
observed packet set full
  -> newer packet number replaces oldest packet number

range count > MAX_ACK_RANGES
  -> encode newest ranges first
```

这样长期运行时 ACK 记忆会向新的 packet number 推进，不会卡在早期 packet 上。

后续可以继续优化更紧凑的 ACK range wire encoding，或增加按时间 / packet distance 的过期策略，但不能破坏 v1 已冻结的基本 ACK range 语义。v1 freeze 接受当前 fixed-length ACK Frame。

## Fragmentation

SRT v1 使用 greedy fragmentation。

策略：

```text
fragment_len = min(max_fragment_bytes, remaining_message_bytes)
```

如果：

```text
max_fragment_bytes = 10
message_len = 11
```

则：

```text
fragment 0: 10 bytes
fragment 1: 1 byte
```

不会平均拆成：

```text
fragment 0: 6 bytes
fragment 1: 5 bytes
```

v1 当前默认：

```text
DEFAULT_FRAGMENT_BYTES = 32
```

这个值是 MCU / 串口友好的默认值，不来自 QUIC MTU。

该默认值已经在 freeze 审核中接受为 reference engine default。用户仍然可以通过 `EngineConfig::fragment_bytes` 调整。

## Receive State Machine

对外 API：

```text
engine.receive(bytes)
```

必须非阻塞。

它可以接收任意串口读取片段：

- 空 bytes。
- 半个 envelope。
- 一个完整 envelope。
- 多个 envelope 粘在一起。
- noise + envelope。
- corrupted envelope + valid envelope。

`receive(bytes)` 的职责是推进当前输入，不等待未来 bytes。

完整 message 不应该通过阻塞 `receive` 返回，而应该通过：

```text
engine.poll_event()
```

产生 Message event。

`receive(bytes)` 一次可以处理：

- 0 个完整 packet。
- 1 个完整 packet。
- 多个 sticky packet。
- noise 后的 packet。
- corrupted packet 后继续 resync。

当一次 `receive(bytes)` 中包含多个 packet 时，函数返回最后一次推进得到的 `ReceiveReport`。完整 message 通过 `poll_event()` 交付，不依赖 `receive` 的返回值。

## Duplicate Packet

当接收端观察到 duplicate DATA packet：

```text
duplicate packet
  -> should ACK again
  -> must not reapply MESSAGE fragment as new data
  -> must not deliver duplicate Message event
```

原因是对端可能没有收到之前的 ACK。

当前实现对 ack-eliciting duplicate DATA 会再次 ACK，但不会重复写入 reassembly，也不会重复产生 Message event。

## Retransmit

SRT v1 spec 定义 tick-driven retransmit：

```text
engine.tick(now)
```

当前 v1 行为：

```text
tick(now)
  -> 只重发达到 retransmit_timeout_ms 的 in-flight packet
  -> 已被 ACK range 覆盖的 packet 不再重发
  -> 达到 max_retransmit_attempts 后产生 SendFailed
  -> message 失败后移除本端同 message 的全部 in-flight packet
  -> 同一个 tick 内不会再重发已经失败的 message
```

v1 当前已经有这些配置：

```text
retransmit_timeout_ms
max_retransmit_attempts
reassembly_timeout_ms
```

这些不是 wire format 字段，但属于 v1 stable behavior。

当前 v1 的 send failure 只冻结一种原因：

```text
RetryLimitReached
```

当 message 失败时：

- 本端移除同 `channel_id + message_id` 的所有 in-flight packet。
- 同一个 tick 内不再重发这条 message 的其它 packet。
- 通过 `EngineOutput::SendFailed` 通知应用。
- 不发送对端 cancel frame。

对端正在 reassembly 的 incomplete message 依靠 `reassembly_timeout_ms` 释放。

## Partial Reliability

SRT 的长期目标不是所有 channel 都强可靠。

SRT v1 spec 保留这些 reliability mode：

```text
Reliable
BestEffort
LatestOnly
Deadline
```

当前 spec 冻结的最小行为：

- Reliable：设置 `ACK_ELICITING`，进入 in-flight，需要 ACK，允许超时重传。
- BestEffort：不设置 `ACK_ELICITING`，不进入 in-flight，不重传，不等待 ACK；如果接收端收到完整 message，仍然正常交付。

当前 spec 只保留概念边界，尚未冻结完整算法：

- LatestOnly：旧 message 可以被新 message 替代。
- Deadline：超过时间窗口后停止重传。

正式算法应在 reliability policy 文档中单独冻结。

因此 v1 freeze 中：

- `Reliable` 属于 v1 正式行为。
- `BestEffort` 属于 v1 正式最小行为。
- `LatestOnly` / `Deadline` 只保留类型和设计方向，不承诺 wire / engine 行为。

## Buffer Budget

v1 必须保持 no_std 固定容量。

当前冻结的主要容量边界：

```text
MAX_WIRE_BYTES
MAX_INGRESS_BYTES
MAX_MESSAGE_BYTES
MAX_EVENTS
MAX_IN_FLIGHT_PACKETS
MAX_ACK_TRACKED_PACKETS
MAX_ACK_RANGES
MAX_REASSEMBLY_MESSAGES
MAX_CHANNEL_POLICIES
```

容量不足时不能 silently overwrite。

当前行为：

- in-flight 满：send path 返回 engine error。
- event queue 满：对应操作返回或报告 engine error。
- reassembly slot 满：incoming fragment 返回 engine error。
- message 超过 `MAX_MESSAGE_BYTES`：返回 engine error。
- ACK observed packet set 满：淘汰最旧 packet number，保留更新 packet number。
- ACK range 超过 `MAX_ACK_RANGES`：优先编码最新 ranges。

## Error / Reject 行为

SRT v1 spec 定义：

```text
bad magic
  -> noise / resync

unsupported version
  -> reject envelope / resync

unsupported header_len
  -> reject envelope / resync

checksum failed
  -> corrupted / resync

packet_len too large
  -> reject envelope / resync

malformed packet
  -> reject packet

malformed MESSAGE frame
  -> reject frame

duplicate DATA packet
  -> ACK but do not reapply
```

具体 `ReceiveReport` / `EngineOutput` 类型需要在代码对齐阶段和这个行为表一致。

## v1 不支持

v1 spec 不支持：

- QUIC stream。
- HTTP/3。
- TCP-compatible byte-stream API。
- PING / PONG Frame。
- RESET_STREAM。
- CANCEL_MESSAGE Frame。
- CLOSE_CHANNEL Frame。
- dynamic channel negotiation。
- connection migration。
- congestion control。
- TLS。
- crypto handshake。
- dynamic MTU discovery。
- packet number wrap-around window semantics。
- ACK delay。
- OS runtime adapter。
- MCU HAL adapter。
- multi-language SDK。
- async runtime integration。

这些不是当前协议核心目标。

## 后续版本候选

v1.1 / v2 可以考虑：

- `CANCEL_MESSAGE`：发送方失败后通知对端释放 incomplete reassembly。
- `HEARTBEAT`：显式心跳。
- `CLOSE_CHANNEL`：关闭某个 channel。
- `LatestOnly`：同 channel 只保留最新 message。
- `Deadline`：超过 deadline 后停止重传。
- 更紧凑的 ACK range encoding。
- ACK delay。
- packet number wrap-around window 语义。
- 更细的 QoS / priority 调度。
- OS / MCU runtime adapter crate。

## 代码对齐清单

当前代码已按本文档检查：

1. `srt-core` 只暴露 MESSAGE / ACK frame。
2. `srt-core` 使用 `MessageFrame` / `ChannelId` 命名。
3. `srt-wire` 实现 envelope header、magic、version、length、CRC-16/XMODEM、resync。
4. `srt-engine` 使用正式 Packet Header + MESSAGE / ACK Frame layout。
5. Packet Header 明确编码 `packet_type`、`packet_flags`、`packet_number`。
6. MESSAGE Frame 明确编码 `frame_type`、`channel_id`、`message_id`、`message_len`、`fragment_offset`、`message_flags`。
7. ACK Frame 明确编码 `frame_type`、`largest_acknowledged`、`range_count`、fixed-capacity ACK ranges。
8. smoke 覆盖 half packet、sticky packet、CRC error、drop、ACK、retransmit、duplicate packet、simultaneous duplex、multi-channel、bidirectional message。
9. integration simulation 覆盖双向多 message、多 channel、drop、corrupt、reorder、tick retransmit 和最终可靠交付。

v1 freeze 审核结论：

1. fixed-length ACK Frame：已接受为 v1 行为。
2. 无对端 cancel frame，依靠 reassembly timeout 清理：已接受为 v1 行为。
3. `LatestOnly` / `Deadline` 后移到 v1.1 / v2：已接受。
4. 当前默认参数，尤其是 `DEFAULT_FRAGMENT_BYTES = 32`：已接受为 reference engine defaults。
5. 更接近真实设备负载的窗口耗尽测试：后移到 v1.1 / hardware validation，不阻塞 v1 freeze。

详见 [SRT v1 可靠传输补齐计划](srt-reliable-transport-plan.md)。

## 结论

SRT v1 protocol spec 的核心不是“把 QUIC 搬到串口上”。

SRT v1 protocol spec 的核心目标是：

```text
Message-Oriented Transport over serial byte streams
```

用 packet number、ACK、dedup、retransmit、channel reliability 等思想，让完整 message 可以在 MCU / robot / drone 场景中可靠、实时、低成本地传输。
