# Protocols

外部代理协议的独立实现。每个 crate 跟踪其上游参照实现的版本，版本号见各自 `Cargo.toml`。

## 当前实现

| Crate | 协议 | 参照 | 版本 | TCP | UDP | MUX | 传输 |
|-------|------|------|------|-----|-----|-----|------|
| `vless` | VLESS | Xray-core | v25.3.1 | ✅ | ✅ | ✅ | 7 种 |
| `shadowsocks` | Shadowsocks | shadowsocks-rust | v1.21.2 | ✅ | ✅ | — | TCP |
| `trojan` | Trojan | trojan-go | v0.10.6 | ✅ | ✅ | ❌ | TLS |
| `hysteria2` | Hysteria2 | hysteria | v2.6.1 | ✅ | ✅ | — | QUIC |
| `mieru` | Mieru | mieru | v3.33.0 | ✅ | ✅ | — | TCP |
| `socks5` | SOCKS5 | RFC 1928 | v0.0.1 | ✅ | ✅ | — | TCP |
| `http-connect` | HTTP CONNECT | RFC 7231 | v0.0.1 | ✅ | — | — | TCP |
| `vmess` | VMess | Xray-core | v25.3.1 | ⚠️ stub | ❌ | ❌ | — |

## 不是外部协议

`direct` 和 `block` 是内置出站动作，不是独立的外部协议，逻辑位于 `zero-engine` 和 `zero-proxy`。

## 各协议详情

每个协议 crate 下的 `README.md` 包含：
- 参照实现和版本追踪
- 功能对齐状态表（✅ 已实现 / ❌ 待实现 / ⚠️ 部分实现）
- 架构文件说明
- 上游参考链接
