# Hysteria2

> 参照 hysteria | Crate: `hysteria2`

Hysteria2 是基于 QUIC 的代理协议，针对不稳定和高延迟网络进行优化，使用 HMAC 进行认证。

## 协议来源

| 项目 | 来源 |
|------|------|
| 参照实现 | [hysteria](https://github.com/apernet/hysteria) |
| 本实现 | `hysteria2` crate |

## 功能对齐状态

| 特性 | 状态 |
|------|------|
| QUIC 传输 (quinn) | ✅ |
| HMAC-SHA256 认证 (TLS key export + salt) | ✅ |
| TCP: accept_bi → parse connect header → route → relay | ✅ |
| UDP: datagram loop + resolver-based forwarding | ✅ |
| 入站: auth stream → auth OK/ERR → stream accept loop | ✅ |
| 出站: QUIC connect → auth handshake → open_bi → relay | ✅ |
| UDP chain 出站 | ✅ |

## 待实现

| 特性 | 状态 |
|------|------|
| UDP 入站路由 (当前直接转发，不经过 engine route decision) | ❌ |
| 速率控制 (token bucket, hysteria 风格拥塞控制) | ❌ |

## 架构

```
src/lib.rs       — crate root, re-exports
src/inbound.rs   — Hysteria2Inbound (auth, accept_bi, UDP datagram)
src/outbound.rs  — Hysteria2Outbound (connect, auth, relay)
src/protocol.rs  — shared: auth frame, connect header, UDP datagram
src/udp.rs       — UDP datagram parse/build
```

## 参考

- [hysteria](https://github.com/apernet/hysteria)
- [Hysteria2 协议规范](https://v2.hysteria.network/docs/advanced/Protocol)
