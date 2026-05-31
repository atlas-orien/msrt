# v2 架构设计

v2 是 SRT 的 wire format draft 阶段。

v1 已经冻结 no_std crate 边界。v2 开始回答更具体的问题：

```text
SRT Packet 到底如何变成串口 byte stream？
Packet Header 如何编码？
Protocol Frame 如何编码？
Decoder 如何从噪声、半包、粘包中恢复？
```

## 文档列表

- [v2 总设计](srt-v2-design.md)
- [v2 wire format 草案](srt-v2-wire-format.md)

## v2 目标

- 定义第一版 wire envelope 字节布局。
- 定义 Packet Header 字节布局。
- 定义 Protocol Frame 字节布局方向。
- 定义 checksum 选择和覆盖范围。
- 定义 decoder 状态机方向。
- 定义兼容性和版本字段。
- 定义 v2 测试策略。

## v2 非目标

- 不实现完整 engine。
- 不实现完整可靠性算法。
- 不实现 MCU HAL。
- 不实现 tokio / OS adapter。
- 不承诺最终 wire compatibility。

v2 的结果应该是一份足够具体的 wire format draft，为后续 `srt-wire` 和 `srt-core` 编解码实现做准备。
