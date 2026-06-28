# Mieru

Mieru 是 socks5-in-tunnel 模型的加密代理协议：先建立 XChaCha20-Poly1305 加密隧道，再在隧道内用 socks5 协商目标（openSession 不携带目标；隧道内 socks5 不做 greeting/auth，因为 mieru 会话即认证）。模块结构与 `protocols/mieru/src/` 对应；socks5 编排在 `crates/proxy/src/adapters/mieru/tcp.rs`（出站 `socks5_connect`）与 `crates/proxy/src/inbound/mieru.rs`（入站 `socks5_serve`）。

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 出站 | `supported` | socks5-in-tunnel：已与外部 mita 端到端互通验证（httpbin.org） |
| UDP 出站 | `supported` | socks5-in-tunnel（UDP ASSOCIATE）：已与外部 mita 互通验证（DNS relay） |
| TCP 入站 | `supported` | socks5-in-tunnel：openSession 握手 + 隧道内 socks5 请求解析；经 loopback 测试验证（`protocols/mieru/tests/loopback.rs`，对已验证出站） |
| UDP 入站 | `supported` | `socks5_serve` 处理 CMD=3 → `run_mieru_udp_relay` |
| MUX | `unsupported` | 多会话复用单条 underlay，性能优化项；暂不实现（单会话模式 TCP/UDP 双向已验证可用） |

## 验证依据

- 出站 TCP + UDP：与上游 mita 真实互通（外部节点）。
- 入站：`protocols/mieru/tests/loopback.rs` 在内存管道上把 Zero 出站（mita 验证过的客户端）与入站配对跑握手 loopback。该测试还抓到并修复了入站首读按 padding0 读 136 字节的死锁 bug。
