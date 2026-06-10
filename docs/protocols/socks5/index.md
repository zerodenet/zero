# SOCKS5

SOCKS5 是 Zero 中的稳定核心协议。模块结构与 `protocols/socks5/src/` 对应：

| 文档 | 对应源码 | 内容 |
|------|---------|------|
| [inbound.md](inbound.md) | `inbound.rs` | SOCKS5 CONNECT 入站、UDP ASSOCIATE 入站 |
| [outbound.md](outbound.md) | `outbound.rs` | 上游 SOCKS5 CONNECT / UDP ASSOCIATE 出站 |
| [shared.md](shared.md) | `shared.rs` | 认证协商、地址编码、UDP 包构建/解析 |

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 入站 | `supported` | SOCKS5 CONNECT |
| UDP 入站 | `supported` | SOCKS5 UDP ASSOCIATE |
| TCP 出站 | `supported` | 上游 SOCKS5 CONNECT |
| UDP 出站 | `supported` | 上游 SOCKS5 UDP ASSOCIATE |
| 认证 | `supported` | 无认证 + 用户名/密码认证 |

## 边界说明

SOCKS5 作为本地入口时仅是客户端侧入口。例如 `socks5 UDP ASSOCIATE → vmess → vmess` 应归类为 VMess 中继链路，而非跨协议链路。
