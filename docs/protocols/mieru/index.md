# Mieru

Mieru 是 socks5-in-tunnel 模型的加密代理协议：先建立 XChaCha20-Poly1305 加密隧道，再在隧道内用 socks5 协商目标（openSession 不携带目标；隧道内 socks5 不做 greeting/auth，因为 mieru 会话即认证）。模块结构与 `protocols/mieru/src/` 对应；socks5 编排在 `crates/proxy/src/outbound/mieru.rs`（出站 `socks5_connect`）与 `crates/proxy/src/inbound/mieru.rs`（入站 `socks5_serve`）。

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 出站 | `supported` | socks5-in-tunnel：已与外部 mita 端到端互通验证（httpbin.org） |
| UDP 出站 | `supported` | socks5-in-tunnel（UDP ASSOCIATE）：已与外部 mita 互通验证（DNS relay） |
| TCP 入站 | `partial` | socks5-in-tunnel：openSession 握手 + 隧道内 socks5 请求解析（对称于出站，已实现，待外部客户端联通验证） |
| UDP 入站 | `partial` | `socks5_serve` 处理 CMD=3 → `run_mieru_udp_relay`（已实现，待联通验证） |
| MUX | `unsupported` | Mieru MUX 未实现 |

## 剩余缺口

- 入站（TCP + UDP）互操作：已对称实现，待外部 mieru 客户端联通验证（capability 标注 `inbound_interop_unverified`）。
