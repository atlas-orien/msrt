# 架构文档

这个目录用于记录 SRT 在实现协议细节之前的架构决策。

## 版本目录

- [v1](v1/README.md)：no_std protocol scaffold，冻结 crate 边界和 engine 语义。

## 当前阶段

当前仍在 v1 整理阶段。

v1 需要先把 `srt-engine`、`srt-wire`、`srt-reliability`、`srt-core` 和 `srt` facade 的边界整理清楚，再进入 wire format draft。
