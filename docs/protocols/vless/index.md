# VLESS

VLESS 是 `partial` 协议能力。基线 TCP 和 UDP-over-stream 路径存在，UDP MUX、部分 final-hop 传输和互操作覆盖仍有缺口。模块结构与 `protocols/vless/src/` 对应：

| 文档 | 对应源码 | 内容 |
|------|---------|------|
| [inbound.md](inbound.md) | `inbound.rs` | VLESS TCP/UDP 入站、Reality/Vision flow |
| [outbound.md](outbound.md) | `outbound.rs` | VLESS TCP/UDP 出站 |
| [shared.md](shared.md) | `shared.rs` | UUID、flow、命令常量 |

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 入站 | `supported` | 基线 VLESS TCP 入口 |
| TCP 出站 | `supported` | 基线 VLESS TCP 上游 |
| UDP 入站 | `partial` | UDP-over-stream 基线 |
| UDP 出站 | `partial` | 单跳及部分 relay-chain final-hop 路径 |
| MUX | `partial` | TCP MUX 已实现；UDP MUX 未实现 |

## 剩余缺口

- `mux_udp_is_not_implemented`
- `udp_relay_chain_final_transport_limited`
- `non_reality_tls_fingerprint_passthrough_is_incomplete`
- `relay_stream_tls_client_fingerprint_is_not_supported`

## 传输方式

VLESS 支持 9 种传输：`tcp`、`tls`、`reality`、`ws`、`grpc`、`h2`、`http_upgrade`、`quic`、`split_http`。
