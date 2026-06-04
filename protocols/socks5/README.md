# SOCKS5

> RFC 1928 — 自有版本 v0.0.1 | Crate: `socks5`

SOCKS5 是标准的互联网代理协议，定义于 RFC 1928。

## 版本追踪

| 项目 | 版本 |
|------|------|
| 协议规范 | [RFC 1928](https://tools.ietf.org/html/rfc1928) |
| 本实现 | `socks5` crate v0.0.1 (自有版本) |

## 功能对齐状态

| 特性 | 状态 |
|------|------|
| TCP CONNECT (无认证) | ✅ |
| TCP CONNECT (Username/Password auth, RFC 1929) | ✅ |
| UDP ASSOCIATE | ✅ |
| 入站: accept + handshake → route → relay | ✅ |
| 出站: TCP connect + SOCKS5 tunnel establish | ✅ |
| 出站: UDP ASSOCIATE relay | ✅ |
| IPv4 / IPv6 / 域名目标地址 | ✅ |
| SOCKS5 UDP 包头解析/构建 | ✅ |

## 架构

```
src/lib.rs       — crate root, re-exports
src/inbound.rs   — Socks5Inbound (auth, CONNECT/UDP ASSOCIATE dispatch)
src/outbound.rs  — Socks5Outbound (TCP tunnel, UDP relay)
src/protocol.rs  — shared: request/response, UDP packet, auth
src/udp.rs       — SOCKS5 UDP relay
```

## 参考

- [RFC 1928 SOCKS5](https://tools.ietf.org/html/rfc1928)
- [RFC 1929 Username/Password Auth](https://tools.ietf.org/html/rfc1929)
