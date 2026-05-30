# 架构文档

这个目录用于记录 SRT 在实现协议细节之前的架构决策。

## 版本目录

- [v1](v1/README.md)：no_std protocol scaffold，冻结 crate 边界。
- [v2](v2/README.md)：wire format draft，开始冻结第一版字节格式。

## 当前阶段

当前进入 v2 设计阶段。

v2 的目标不是实现完整 runtime，而是先定义第一版可讨论的 wire format、packet/frame 编码方向、错误边界和测试策略。
