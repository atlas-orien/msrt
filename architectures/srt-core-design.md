# srt-core 设计

`srt-core` 是最小、最核心的协议 crate。

它的目标是定义所有 SRT 协议 crate 共同认可的基础结构。它应该足够简单、稳定、`no_std`、无堆分配，并且容易推理。

## 职责

`srt-core` 定义 SRT packet 是什么，以及描述 packet 所需的基础类型。

它应该包含：

- packet 结构
- packet header 结构
- packet kind 定义
- stream identifier
- sequence number
- flags
- 协议常量和限制

它不应该包含：

- 字节编码或解码
- CRC 实现
- packet 重新同步
- 重传算法
- ack 跟踪
- stream 调度
- runtime 状态机
- UART 或操作系统适配

## Packet 是中心

`srt-core` 的中心结构应该是 `Packet`。

SRT 在原始字节流上传输消息，但协议本身应该以 packet 为单位进行思考。Packet 是 runtime 发送、接收、ack、重传、路由和调度的语义传输单元。

预期结构如下：

```rust
pub struct PacketHeader {
    pub kind: PacketKind,
    pub stream_id: StreamId,
    pub seq: Seq,
    pub flags: Flags,
}

pub struct Packet<'a> {
    pub header: PacketHeader,
    pub payload: &'a [u8],
}
```

这样可以让核心 packet 模型保持：

- `no_std`
- 无堆分配
- 不绑定 payload 容器
- MCU 和上位机环境都可用
- 不绑定 frame 编码

## Flags

`Flags` 当前使用 `u8`。

原因是 SRT 面向串口和 MCU，每个 byte 都应该谨慎使用。当前阶段只有少量 packet flags，不应该提前使用 `u16` 扩大所有 packet 的 header。

这也更接近 QUIC 的思路：header bits 应该紧凑，并且根据 packet/header 类型解释，而不是预先放一个很宽的通用 flags 字段。

如果未来 8 bit 不够，可以通过版本化 header、扩展 header、control packet 或 packet kind 扩展，而不是在第一版协议里提前增加固定开销。

## Packet 与 Frame 的边界

`Packet` 属于 `srt-core`。

Frame 编码属于 `srt-frame`。

这个边界很重要：

```text
Packet
  协议含义。
  被 runtime、stream、reliability 逻辑使用。

Frame
  字节流边界。
  用于在串口类链路上编码和解码 packet。
```

`srt-core` 不应该知道字节如何转义、校验、拆分或重新同步。它只定义协议各层共同认可的数据结构。

## Error 依赖

`srt-core` 可以为了使用方便 re-export `srt-error` 的类型：

```rust
pub use srt_error::{Error, ErrorKind, Result};
```

规范的错误定义位于 `srt-error`，不是 `srt-core`。

## 模块布局

`lib.rs` 应该保持很小：

```rust
pub mod flags;
pub mod id;
pub mod packet;
pub mod seq;

pub use flags::Flags;
pub use id::StreamId;
pub use packet::{Packet, PacketHeader, PacketKind};
pub use seq::Seq;
pub use srt_error::{Error, ErrorKind, Result};
```

具体类型定义由各个模块文件负责。`lib.rs` 只作为公开出口。

## 当前阶段

当前阶段，`srt-core` 只应该定义结构。

未来可以添加简单 constructor 或 accessor 来维护不变量，但协议行为不应该进入这个 crate。
