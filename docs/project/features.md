# 编译参数

Zero 通过 Cargo features 控制编译产物中包含的能力子集，按需裁剪二进制体积和依赖面。

## 预设

| Preset | 包含内容 | 适用场景 |
|--------|---------|---------|
| `default` | `full` + `status-api` | 客户端本地使用 |
| `full` | 全部入站/出站协议 + DNS UDP | 完整代理节点 |

```bash
# 默认构建（客户端场景，不需要 connectors）
cargo build --release

# 等同于
cargo build --release --features full,status-api
```

## 入站协议

每个入站协议独立 feature-gated，可按需裁剪。

| Feature | 协议 | 额外依赖 |
|---------|------|---------|
| `inbound-socks5` | SOCKS5 入站 | — |
| `inbound-http-connect` | HTTP CONNECT 入站 | — |
| `inbound-mixed` | 混合入站（同一端口 SOCKS5 + HTTP） | 隐含 `inbound-socks5` + `inbound-http-connect` |
| `inbound-vless` | VLESS 入站 | TLS / Reality / WebSocket / gRPC / H2 / QUIC 传输 |
| `inbound-hysteria2` | Hysteria2 入站 | QUIC（quinn） |
| `inbound-shadowsocks` | Shadowsocks 入站 | AEAD 加密 + 2022-blake3 |
| `inbound-trojan` | Trojan 入站 | TLS |

```bash
# 裁剪示例：仅 SOCKS5 + HTTP CONNECT
cargo build --release --no-default-features \
  --features inbound-socks5,inbound-http-connect,status-api
```

## 出站协议

| Feature | 协议 | 额外依赖 |
|---------|------|---------|
| `outbound-socks5` | SOCKS5 出站 | — |
| `outbound-vless` | VLESS 出站 | 同入站传输栈 |
| `outbound-hysteria2` | Hysteria2 出站 | QUIC（quinn） |
| `outbound-shadowsocks` | Shadowsocks 出站 | 同入站加密 |
| `outbound-trojan` | Trojan 出站 | TLS |

`direct` 和 `block` 出站始终可用，无需 feature gate——它们不需要协议实现。

## DNS

| Feature | 说明 |
|---------|------|
| `dns-udp` | UDP DNS 服务器后端（用于自建 DNS 解析） |

> 不启用 `dns-udp` 时，DNS 回落系统解析器（`tokio::net::lookup_host`）。DNS 缓存和 Fake IP 始终可用，无需 feature gate。

## 控制面（服务端部署）

以下 feature 用于将 Zero 作为服务端/面板节点部署，**不在默认 `full` 预设中**。

| Feature | 说明 | 隐含 |
|---------|------|------|
| `status-api` | HTTP 状态 API（`/api/v1/*`） | — |
| `event-dispatcher` | 事件分发器：将 zero 事件投递到外部 sink | `dep:zero-connector` |
| `sink-jsonl` | JSON Lines 文件 sink（事件持久化） | `event-dispatcher` |
| `panel-connector` | 面板连接器：心跳 + 远程命令，节点上报 | `status-api` + `event-dispatcher` |

```bash
# 服务端构建（含面板连接器）
cargo build --release --features full,status-api,panel-connector
```

**`panel-connector` 的依赖面：**

- `status-api` — HTTP 控制端点
- `event-dispatcher` — 事件投递基础设施
- `zero-connector` crate — PushConnector（心跳/命令轮询）、EventDispatcher（事件分发）、Webhook sink

## 客户端 vs 服务端

```
客户端场景:  full + status-api  （默认）
              ├─ 入站/出站协议
              ├─ DNS UDP
              └─ HTTP 状态端点（本地调试）

服务端场景:  + panel-connector
              ├─ 事件分发（→ webhook / jsonl）
              └─ 面板心跳上报 + 远程命令
```

## 与协议实现的关系

在 `full` 中没有但实际上存在的协议：

| 协议 | Feature | 说明 |
|------|---------|------|
| VMess | — | 仅有 crate 骨架，未实现。无 feature gate |
| HTTP CONNECT 出站 | — | 未实现出站方向 |

入站和出站 feature 不对称是正常现象——某些协议的出站/入站不需要对方。

## 构建产物大小参考

| Features | 二进制大小（release, stripped） |
|----------|-------------------------------|
| `default`（`full` + `status-api`） | ~15 MB |
| `--no-default-features` + socks5 入站/出站 + direct | ~5 MB |
