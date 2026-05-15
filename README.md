# Zero

Zero 是一个用 Rust 编写的网络代理内核，当前版本 `v0.0.2`。`v0.1.0` 保留给第一次正式发布。

## 当前支持

### 入站协议
- `SOCKS5`（no-auth，CONNECT + UDP ASSOCIATE）
- `HTTP CONNECT`
- `mixed`（同端口自动检测 SOCKS5 / HTTP CONNECT）
- `VLESS`（TCP / TLS / Reality / WebSocket / gRPC / H2 / HTTPUpgrade / QUIC / SplitHTTP）
- `Hysteria2`（QUIC，密码认证）
- `Shadowsocks`（TCP）

### 出站协议
- `direct` / `block`
- `SOCKS5`（TCP CONNECT + UDP relay）
- `VLESS`（全传输：TLS / Reality / WS / gRPC / H2 / HTTPUpgrade / QUIC / SplitHTTP）
- `Hysteria2`（QUIC，TCP + UDP 出站）
- `Shadowsocks`（TCP）

### 路由与出站组
- `mode = rule | global | direct`
- `selector` / `fallback` / `urltest` 出站组
- `group -> group` 嵌套
- 运行时 selector 切换
- 基于域名、CIDR 和外置 `rule_sets` 的静态路由

### VLESS 协议特性
- 9 种传输层：TCP / TLS / Reality / WebSocket / gRPC (MultiMode) / H2 / HTTPUpgrade / QUIC / SplitHTTP
- Reality 自研 TLS 1.3 实现
- XTLS Vision / Vision-UDP AEAD flow
- MUX 连接池 + 流加密（Xray 兼容帧格式）
- UDP over TCP（v1 + v2 协议）
- Fallback（ALPN + 非 TLS 双路回落）

### 控制面
- HTTP API（`/api/v1/*`，Bearer Token / mTLS 认证）
- IPC Unix socket（JSON 协议，CLI 和外部工具接入）
- CLI：`zero status` / `select` / `flows` / `policies` / `events` / `reload`
- 事件系统（EventSource / EventSink）
- Panel push connector（心跳上报 + 命令拉取）

### 嵌入
- `zero-ffi` crate：`cdylib` + `staticlib`，C 兼容接口，供 Go/Python/移动端嵌入

## 快速开始

```shell
# 构建
cargo build
cargo build --release

# 本地运行
cargo run -- run examples/v0.0.1/basic.json

# 带 HTTP 控制面
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.0.1/basic.json

# CLI 状态查询
cargo run -- status --json examples/v0.0.1/basic.json

# 运行时切换 selector
curl -X POST http://127.0.0.1:9090/selectors/proxy/direct
```

## 配置示例

- `examples/v0.0.1/basic.json` — 默认 mixed 入站
- `examples/v0.0.1/mixed.json` / `chained-socks5.json` / `server-socks5.json`
- `examples/v0.0.2/vless.json` / `vless-tls.json` / `chained-vless-tls.json`
- `examples/v0.0.2/fallback.json` / `nested-groups.json` / `urltest.json`
- `examples/v0.1.0/hysteria2.json`

## 选择性编译

可选模块通过 Cargo feature 控制：

| 分类 | Feature |
|------|---------|
| 入站 | `inbound-socks5`, `inbound-http-connect`, `inbound-mixed`, `inbound-vless`, `inbound-hysteria2`, `inbound-shadowsocks` |
| 出站 | `outbound-socks5`, `outbound-vless`, `outbound-hysteria2`, `outbound-shadowsocks` |
| 控制面 | `status-api`, `event-dispatcher`, `sink-jsonl`, `panel-connector` |

默认构建：`cargo build --features full,status-api`

## 目录

```
src/                          应用程序入口 + CLI + HTTP adapter + IPC
crates/
├── traits/                   平台能力抽象（AsyncSocket, TransportConnector ...）
├── core/                     领域模型（Address, Session, ProtocolType ...）
├── config/                   配置模型、校验、rule_set 装载
├── engine/                   决策、计划、状态、会话、统计、事件
├── proxy/                    代理运行时、监听、出站、UDP 管理
├── router/                   路由规则匹配
├── transport/                传输实现（TLS/WS/gRPC/H2/QUIC/SplitHTTP ...）
├── platform/tokio/           Tokio 后端
├── api/                      控制面 API 类型
├── connector/                事件分发与推送连接器
├── logging/                  结构化日志
├── ffi/                      C 兼容嵌入接口
protocols/
├── socks5/
├── http-connect/
├── vless/                    Reality 自研 TLS 1.3
├── hysteria2/
└── shadowsocks/
examples/                     示例配置
docs/                         文档
```

## 文档入口

- [配置说明](docs/project/config.md)
- [模式和节点组](docs/project/modes-and-groups.md)
- [日志说明](docs/project/logging.md)
- [控制面路线图](docs/control-plane/)
- [版本索引](docs/versions/README.md)
