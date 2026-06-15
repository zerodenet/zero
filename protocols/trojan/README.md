# Trojan

> 参照 trojan-go | Crate: `trojan`

Trojan 是一种基于 TLS 的代理协议，通过将代理流量伪装为 HTTPS 来绕过检测。

## 协议来源

| 项目 | 来源 |
|------|------|
| 参照实现 | [trojan-go](https://github.com/p4gefau1t/trojan-go) |
| 本实现 | `trojan` crate |

## 功能对齐状态

| 特性 | 状态 |
|------|------|
| TCP: TLS 伪装 + 密码认证 + connect header | ✅ |
| TCP 链式出站 (TLS → upstream) | ✅ |
| UDP: CMD_UDP → TLS stream → framed relay | ✅ |
| 入站: accept + 密码验证 → route → relay | ✅ |
| 出站: TLS connect + send header → relay | ✅ |
| UDP 链式出站 | ✅ |

## 待实现

| 特性 | 状态 |
|------|------|
| Trojan-Go 多路复用 (smux) | ❌ 未计划 |
| WebSocket 传输 | ❌ 未计划 |

## 架构

```
src/lib.rs       — crate root, re-exports
src/inbound.rs   — TrojanInbound (TLS accept, auth, route)
src/outbound.rs  — TrojanOutbound (TLS connect, send header, relay)
src/shared.rs    — shared: UDP frame, request/response format
```

## 参考

- [trojan-go](https://github.com/p4gefau1t/trojan-go)
- [Trojan 协议规范](https://trojan-gfw.github.io/trojan/protocol)
