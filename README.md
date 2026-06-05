# Zero

用 Rust 编写的网络代理内核。可作为本地代理网关、边缘节点或服务端部署，通过控制面 API 由面板驱动。

## 快速开始

```shell
# 构建（默认含全部协议 + HTTP 控制面）
cargo build --features full,status_api
cargo build --release

# 运行
cargo run -- run config.json

# 带控制面（供面板或 CLI 接入）
cargo run -- run --status-listen 127.0.0.1:9090 config.json

# CLI 状态查询
cargo run -- status --json config.json
```

## 文档

| 文档 | 说明 |
|------|------|
| [快速开始](docs/guides/quickstart.md) | 首次使用的完整指引 |
| [配置说明](docs/project/config.md) | 所有配置项参考 |
| [架构说明](docs/project/architecture.md) | 内核分层与依赖规则 |
| [控制面 API](docs/control-plane-api/README.md) | HTTP / gRPC / IPC 接口参考 |
| [GUI 接入指南](docs/guides/gui-integration.md) | 面板对接方式 |

## 选择性编译

通过 Cargo feature 按需裁剪二进制体积：

```shell
cargo build --features full,status_api           # 全部能力
cargo build --features vless,status_api          # 仅 VLESS + 控制面
```

完整 feature 列表见 [Cargo.toml](Cargo.toml) `[features]` 段。

## 配置示例

`examples/` 目录提供可直接运行的配置样例，覆盖基础入站、链式代理、分组、规则集和具体协议。

## 许可证

MIT — 详见 [LICENSE](LICENSE)
