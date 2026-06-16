# 未完成项

本页只记录协议层**尚未完成**的能力缺口。已完成项已移除（实现与验证记录见各协议 `index.md` 与 [protocol-capabilities.md](../project/protocol-capabilities.md)）。能力事实以运行时 `capabilities.protocols`（各协议 metadata）为准。

## Shadowsocks

常规 AEAD Shadowsocks TCP/UDP 不受下列缺口影响；SIP022 全部 spec 章节已实现。

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| `shadowsocks_2022_hardening_not_externally_validated` | 新的检测防御/drain 与滑动窗口未对抗真实主动探测/重放攻击完成外部验证 | 用真实 prober/重放工具验证单次读取+drain 与滑动窗口行为；并在未损坏的外部 `ssserver` 上完成 TCP 出站端到端互通 |

## VLESS

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| `udp_relay_final_hop_not_externally_validated` | XHTTP `stream-one` 单连接最终跳已实现并有内部 e2e 覆盖（`relays_udp_through_socks5_to_vless_xhttp_stream_one_relay_chain`）；尚未与上游 Xray 服务器完成外部互通，不能据此声明生产级最终跳兼容。QUIC 因 XTLS 弃用且需非 TCP 载体，不作最终跳 | 与上游 Xray 服务器完成 stream-one 最终跳 TCP/UDP 端到端互通验证 |

## Trojan

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| 外部互通覆盖不足 | 当前不能声明生产级完整兼容 | 使用基线实现进行 TCP 和 UDP 外部互通测试 |
| MUX 不支持 | 不提供 Trojan MUX 能力 | 明确实现 MUX 或保持 `unsupported` |

## Hysteria2

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| 外部互通覆盖不足 | 不能声明生产级完整兼容 | 使用基线 Hysteria2 实现进行 TCP stream 和 UDP datagram 互通 |

## VMess

| 缺口 | 影响 | 完成标准 |
|------|------|----------|
| `cipher: zero` 非主流互通能力 | Zero 内部路径可用；Xray inbound 不接受 `zero` security，不能作为主流面板默认选项展示 | 只作为 Zero 内部兼容项保留，或在确认主流实现支持后补充外部互通测试 |

## 通用要求

协议从 `partial` 或 `experimental` 提升到 `supported` 需要同时满足：

- 配置解析和校验完整；
- 未编译 feature 时能早期失败；
- TCP/UDP 方向接入统一 runtime pipe；
- 运行时统计、事件、session 生命周期可观测；
- 协议细节留在协议 crate 内；
- 外部基线实现互通测试通过；
- docs 和 `capabilities.protocols` 同步更新。
