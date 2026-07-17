# Hysteria2

Zero 的 Hysteria2 实现使用 QUIC stream 承载 TCP 请求，使用 QUIC datagram 承载 UDP 数据。鉴权和协议帧属于 Hysteria2 协议模块，QUIC 连接生命周期通过中立能力交给通用运行时。

## 能力摘要

| 能力 | 状态 | 承载方式 |
| --- | --- | --- |
| TCP 入站 | `supported` | QUIC stream |
| TCP 出站 | `supported` | QUIC stream |
| UDP 入站 | `partial` | QUIC datagram |
| UDP 出站 | `partial` | QUIC datagram 与 packet-path |
| MUX | `unsupported` | 不另行定义协议级 MUX |

## 文档

- [入站](./inbound.md)
- [出站](./outbound.md)
- [公共约定](./shared.md)
