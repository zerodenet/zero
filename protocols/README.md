# Protocols

外部代理协议的独立实现。每个 crate 以协议规范或上游实现为参考，能力状态以当前代码和测试为准。

## 当前实现

| Crate | 协议 | 参照 | TCP | UDP | MUX | 传输 |
|-------|------|------|-----|-----|-----|------|
| `vless` | VLESS | Xray-core | ✅ | ✅ | ✅ | TLS / Reality / WS / gRPC / H2 / QUIC |
| `shadowsocks` | Shadowsocks | shadowsocks-rust | ✅ | ✅ | — | TCP |
| `trojan` | Trojan | trojan-go | ✅ | ✅ | ❌ | TLS |
| `hysteria2` | Hysteria2 | hysteria | ✅ | ✅ | — | QUIC |
| `mieru` | Mieru | mieru | ✅ | ✅ | — | TCP |
| `socks5` | SOCKS5 | RFC 1928 | ✅ | ✅ | — | TCP |
| `http_connect` | HTTP CONNECT | RFC 7231 | ✅ | — | — | TCP |
| `vmess` | VMess | Xray-core | ⚠️ stub | ⚠️ | ⚠️ | — |

## 不是外部协议

`direct` 和 `block` 是内置出站动作，不是独立的外部协议，逻辑位于 `zero-engine` 和 `zero-proxy`。

## 各协议详情

每个协议 crate 下的 `README.md` 包含：
- 协议来源和能力状态
- 功能对齐状态表（✅ 已实现 / ❌ 待实现 / ⚠️ 部分实现）
- 架构文件说明
- 上游参考链接
