# SOCKS5 入站

对应 `protocols/socks5/src/inbound.rs` — SOCKS5 CONNECT 和 UDP ASSOCIATE 入站处理。

## TCP CONNECT

1. 读取认证方法 → 选择无认证或用户名/密码认证
2. 读取 CONNECT 请求 (ATYP + ADDR + PORT)
3. 返回 session + 目标地址

## UDP ASSOCIATE

1. 与 CONNECT 相同的认证流程
2. 读取 UDP ASSOCIATE 请求
3. 分配本地 UDP relay 端口
4. 返回 relay 地址 + 端口

## 认证

- 无认证 (no-auth)
- 用户名/密码认证 (RFC 1929)
- 每用户速率限制通过 `Session::apply_auth()` 注入
