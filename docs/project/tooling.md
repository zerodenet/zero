# 工程规则

## 命名

- package 名用 `zero-*`
- 目录名保持短名，比如 `crates/engine`、`crates/proxy`、`protocols/socks5`
- 主程序放根目录 `src/main.rs`

## 基线

- Rust 2021
- Cargo workspace
- `cargo fmt`
- `cargo clippy`
- `cargo test`

## 编译特性

- 核心内核能力默认始终参与编译
- 协议和控制面能力通过 Cargo feature 选择性编译
- `zero` 根包只负责把协议 feature 转发到 `zero-proxy`
- 嵌入式或裁剪场景优先使用 `--no-default-features` 再按需开启模块

根包 `zero` 当前 feature：

| Feature | 说明 |
|------|------|
| `default` | 等同 `full,status_api` |
| `full` | 启用全部协议能力和 `dns` |
| `dns` | DNS 子系统 |
| `socks5` | SOCKS5 inbound/outbound |
| `http_connect` | HTTP CONNECT inbound |
| `mixed` | 同端口自动识别 SOCKS5 / HTTP CONNECT，依赖 `socks5` 和 `http_connect` |
| `vless` | VLESS inbound/outbound 及相关传输 |
| `hysteria2` | Hysteria2 inbound/outbound |
| `shadowsocks` | Shadowsocks inbound/outbound |
| `trojan` | Trojan inbound/outbound |
| `vmess` | VMess inbound/outbound |
| `mieru` | Mieru inbound/outbound |
| `status_api` | 本地 HTTP 控制面 |
| `event_dispatcher` | 事件分发器基础能力 |
| `sink_jsonl` | JSON Lines 事件 sink，依赖 `event_dispatcher` |
| `panel_connector` | 面板/远程连接器，依赖 `status_api` 和 `event_dispatcher` |
| `grpc_api` | gRPC 控制面 adapter |

`zero-proxy` 内部还有面向传输 crate 的细分 feature，例如 VLESS 会转发启用 TLS、WebSocket、gRPC、H2、HTTPUpgrade、SplitHTTP 和 QUIC 相关能力。外部构建入口应优先使用根包 feature，不直接依赖内部 crate feature 形状。

## 代码边界

- `zero-traits` 和 `zero-core` 不绑 Tokio
- 协议实现放 `protocols/*`
- `zero-engine` 只保留决策、计划、状态、会话、统计和事件
- `direct`、`block` 的目标语义留在 `zero-engine`，socket 级执行留在 `zero-proxy`
- 监听、传输、Tokio 接线和协议运行时适配放 `zero-proxy`
- 根包 `zero` 不要塞协议细节

## 文档边界

- 改配置格式，要更新配置文档和示例
- 改协议范围，要更新项目文档和示例
- 改分层，要更新 `docs/project/`
