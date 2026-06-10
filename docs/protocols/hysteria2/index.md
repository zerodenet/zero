# Hysteria2

Hysteria2 是 `partial` 协议能力。基线 QUIC stream 和 QUIC datagram 路径存在。模块结构与 `protocols/hysteria2/src/` 对应。

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 入站 | `supported` | QUIC stream |
| TCP 出站 | `supported` | QUIC stream |
| UDP 入站 | `partial` | QUIC datagram |
| UDP 出站 | `partial` | QUIC datagram |
| MUX | `unsupported` | Hysteria2 不提供独立 MUX 配置 |

## 剩余缺口

- 外部互操作覆盖不足
- `udp_relay_chain_quic_path_not_supported`

## 说明

Hysteria2 使用 QUIC 作为底层传输，不支持传统 TCP/TLS 传输配置。UDP 走 QUIC datagram 通道，不是 UDP-over-stream 模式。
