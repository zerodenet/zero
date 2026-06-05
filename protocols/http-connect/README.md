# HTTP CONNECT

> RFC 7231 | Crate: `http_connect`

HTTP CONNECT 方法用于通过代理建立 TCP 隧道。这是公开 RFC 协议，无上游实现可参照。

## 协议来源

| 项目 | 来源 |
|------|------|
| 协议规范 | [RFC 7231 §4.3.6](https://tools.ietf.org/html/rfc7231#section-4.3.6) |
| 本实现 | `http_connect` crate |

## 功能对齐状态

| 特性 | 状态 |
|------|------|
| CONNECT 方法解析 | ✅ |
| Host:Port 目标地址解析 | ✅ |
| 200 Connection Established 响应 | ✅ |
| Proxy-Authorization (Basic auth) | ✅ |
| 入站: accept + parse → route → relay | ✅ |

## 架构

```
src/lib.rs       — crate root, re-exports
src/inbound.rs   — HttpConnectInbound (accept, parse CONNECT, auth)
src/protocol.rs  — shared: request/response format
```

## 参考

- [RFC 7231 §4.3.6 CONNECT](https://tools.ietf.org/html/rfc7231#section-4.3.6)
