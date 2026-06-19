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

QUIC 数据包路径载体已支持：`Hysteria2PacketPath` 实现 `UdpPacketPath`，经 QUIC 连接承载内部 datagram（如 Shadowsocks），`[Hysteria2, Shadowsocks]` UDP 中继链可用（见 e2e 测试 `relays_udp_through_hysteria2_to_shadowsocks_packet_path_chain`）。这不是 Shadowsocks 的附属实现，而是 Hysteria2 自己的 relay-chain 载体能力。

## 说明

Hysteria2 使用 QUIC 作为底层传输，不支持传统 TCP/TLS 传输配置。UDP 走 QUIC datagram 通道，不是 UDP-over-stream 模式。
