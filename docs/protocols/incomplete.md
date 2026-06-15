# 未完成项

本页集中记录协议层未完成能力。它用于 GUI、面板和开发者判断哪些能力可以直接对接，哪些能力仍需要后续实现或外部互通验收。

## Shadowsocks

SIP022 全部 spec 章节已实现并通过内置测试（3.1.1–3.1.5、3.2 含 3.2.4 滑动窗口 + 按客户端 session id 隔离 UDP 中继流、3.1.3 检测防御、3.1.5 服务端重放 salt 池、AEAD 2022 UDP server response 回填客户端 session id）。常规 AEAD Shadowsocks TCP/UDP 不受下列缺口影响。

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| `shadowsocks_2022_hardening_not_externally_validated` | 新的检测防御/drain 与滑动窗口未对抗真实主动探测/重放攻击完成外部验证 | 用真实 prober/重放工具验证单次读取+drain 与滑动窗口行为；并在未损坏的外部 `ssserver` 上完成 TCP 出站端到端互通 |

AEAD 2022 验证覆盖：TCP 入站已通过 `shadowsocks-rust` 参考客户端 `sslocal` 端到端互操作（HTTP 200）；TCP 出站管线已通过 Zero→Zero；AEAD 2022 UDP server response 已通过手动探针（DNS 往返 + 滑动窗口重放拒绝）；SIP022 3.2.4 按 session id 隔离 UDP 中继流已实现（`UdpFlowKey` 增加 `client_session_id` 维度）。

## VLESS

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| `mux_udp_is_not_implemented` | VLESS MUX UDP outbound API（`MuxConnectionPool::open_udp_stream`）已实现但尚未接入 `VlessUdpOutboundManager`；inbound MUX UDP 已完备 | 将 MUX UDP outbound API 接入 UDP 调度路径 |
| `udp_relay_chain_final_transport_limited` | SplitHTTP 最终跳已通过 `start_relay_flow()` fast path 实现；QUIC 因需要非 TCP 载体仍不支持作为链最终跳 | QUIC 路径明确实现方案或记录为架构限制 |
| `non_reality_tls_fingerprint_passthrough_is_incomplete` | 已审计确认 fingerprint 通过 `ClientTlsConfig.client_fingerprint` 隐式传递到 `connect_tls_upstream()`，SplitHTTP/gRPC/H2/WS + TLS 路径均已覆盖。运行时仍报告此 limitation 以提示调用方未使用显式参数传递 | 待后续清理：在调用链增加显式 `client_fingerprint` 参数后移除 |

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
