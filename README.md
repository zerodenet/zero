# Zero

用 Rust 编写的网络代理内核。可作为本地代理网关、边缘节点或服务端部署，通过控制面 API 由面板驱动。

当前版本 `v0.0.4`，处于 `v0.0.x` 预发布线。

## 快速开始

```shell
# 构建（默认含全部协议 + HTTP 控制面）
cargo build --features full,status-api
cargo build --release

# 运行
cargo run -- run examples/v0.0.1/basic.json

# 带控制面（供面板或 CLI 接入）
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.0.1/basic.json

# CLI 状态查询
cargo run -- status --json examples/v0.0.1/basic.json
```

## 文档

| 文档 | 说明 |
|------|------|
| [快速开始](docs/guides/quickstart.md) | 首次使用的完整指引 |
| [配置说明](docs/project/config.md) | 所有配置项参考 |
| [架构说明](docs/project/architecture.md) | 内核分层与依赖规则 |
| [控制面 API](docs/control-plane-api/README.md) | HTTP / gRPC / IPC 接口参考 |
| [GUI 接入指南](docs/guides/gui-integration.md) | 面板对接方式 |
| [版本索引](docs/versions/README.md) | 各版本交付范围 |

## 选择性编译

通过 Cargo feature 按需裁剪二进制体积：

```shell
cargo build --features full,status-api           # 全部能力
cargo build --features inbound-vless,outbound-vless,status-api  # 仅 VLESS
```

完整 feature 列表见 [Cargo.toml](Cargo.toml) `[features]` 段。

## 配置示例

`examples/` 目录下按版本组织：

- `v0.0.1/` — 基础 mixed / socks5 / http 入站
- `v0.0.2/` — VLESS TLS + chained + fallback / urltest / nested groups
- `v0.1.0/` — Hysteria2

## 许可证

MIT — 详见 [LICENSE](LICENSE)
