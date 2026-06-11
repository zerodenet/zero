# 未完成项

本页集中记录协议层未完成能力。它用于 GUI、面板和开发者判断哪些能力可以直接对接，哪些能力仍需要后续实现或外部互通验收。

## Shadowsocks

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| `shadowsocks_2022_tcp_header_is_not_implemented` | AEAD 2022 TCP 不能声明完整外部互通 | 实现 SIP022 TCP request/response header，并通过 `shadowsocks-rust` TCP 互通 |
| `shadowsocks_2022_udp_server_response_context_is_not_implemented` | Zero 作为 AEAD 2022 UDP server 时响应包不完整 | 保存 client/session control state，并用于 server response 编码 |

常规 AEAD Shadowsocks TCP/UDP 不受上述缺口影响。

## VLESS

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| `mux_udp_is_not_implemented` | VLESS MUX 不承载 UDP sub-connection | 实现 UDP MUX sub-connection 编解码和运行时派发 |
| `udp_relay_chain_final_transport_limited` | UDP relay-chain 不支持所有 final-hop transport | SplitHTTP、QUIC 等非当前 TCP relay stream 路径有明确实现 |
| `non_reality_tls_fingerprint_passthrough_is_incomplete` | 非 Reality TLS fingerprint 行为不完整 | 将 fingerprint cipher suite / key exchange 偏好传入 TLS 实现 |

## Trojan

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| 外部互通覆盖不足 | 当前不能声明生产级完整兼容 | 使用基线实现进行 TCP 和 UDP 外部互通测试 |
| `relay_stream_tls_client_fingerprint_is_not_supported` | relay-chain final hop 的 TLS fingerprint 能力不完整 | 与 VLESS 同类 TLS relay-stream 边界一起解决 |
| MUX 不支持 | 不提供 Trojan MUX 能力 | 明确实现 MUX 或保持 `unsupported` |

## Hysteria2

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| 外部互通覆盖不足 | 不能声明生产级完整兼容 | 使用基线 Hysteria2 实现进行 TCP stream 和 UDP datagram 互通 |
| `udp_relay_chain_quic_path_not_supported` | QUIC UDP path 不能作为完整 UDP chain carrier | 定义并实现 QUIC packet path carrier |

## Mieru

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| 外部互通覆盖不足 | 不能声明生产级完整兼容 | 使用基线 Mieru 客户端、服务端互通测试 |

## VMess

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| `cipher: zero` 非主流互通能力 | Zero 内部路径可用；Xray inbound 不接受 `zero` security，不能作为主流面板默认选项展示 | 只作为 Zero 内部兼容项保留，或在确认主流实现支持后补充外部互通测试 |

当前 VMess 的 TCP/UDP 基线外部互通已覆盖 Xray（双向 TCP `aes-128-gcm`/`none`、双向 WS+TLS TCP、双向 gRPC+TLS TCP、双向 UDP）、sing-box（Zero outbound TCP+UDP）和 Mihomo/Clash-family（Mihomo outbound TCP `auto`+UDP over `CMD_UDP` raw datagram）。没有验证覆盖的传输组合不能扩大为通用家族兼容声明。

## 通用要求

协议从 `partial` 或 `experimental` 提升到 `supported` 需要同时满足：

- 配置解析和校验完整；
- 未编译 feature 时能早期失败；
- TCP/UDP 方向接入统一 runtime pipe；
- 运行时统计、事件、session 生命周期可观测；
- 协议细节留在协议 crate 内；
- 外部基线实现互通测试通过；
- docs 和 `capabilities.protocols` 同步更新。
