# Mieru

Zero 的 Mieru 实现使用 Mieru 载体建立隧道，并在隧道内执行 SOCKS5 CONNECT 或 UDP ASSOCIATE 控制流程。隧道协商和数据阶段编解码属于 `protocols/mieru`，通用代理层只负责打开载体、路由和转发。

## 能力摘要

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| TCP 入站 | `supported` | 隧道内 CONNECT |
| TCP 出站 | `supported` | 通过协议所有的 tunnel session 连接目标 |
| UDP 入站 | `supported` | 隧道内 UDP ASSOCIATE |
| UDP 出站 | `supported` | 协议所有的 UDP 会话与数据帧 |

## 文档

- [入站](./inbound.md)
- [出站](./outbound.md)
- [会话流程](./flow.md)
