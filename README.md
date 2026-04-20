# Zero

Zero 是一个用 Rust 编写的网络代理项目。

当前仓库处于 `v0.0.x` 预发布线，当前版本是 `v0.0.1`。`v0.1.0` 保留给第一次正式发布；在那之前，重点是把本地可用、云端可部署、主代理链路和基础观测能力做稳。

## 当前支持

- `SOCKS5` 入站
- `HTTP CONNECT` 入站
- `mixed` 同端口多协议入站
- `direct` 出站
- `block` 出站
- 上游 `SOCKS5` 链式出站
- `SOCKS5 UDP ASSOCIATE`
- `mode = rule | global | direct`
- `selector` 出站组
- 基于域名和 CIDR 的静态路由
- 结构化日志
- 本地只读状态导出

当前支持 `TCP`，也支持通过 `SOCKS5 UDP ASSOCIATE` 进入的 `UDP`。UDP 当前可走 `direct`、`block` 和上游 `SOCKS5`。

上游 UDP 链路当前还有两项运行时能力：

- 同一条本地 `UDP ASSOCIATE` 会话复用上游 `SOCKS5` UDP association
- 上游 UDP association 默认 `30s` 空闲超时，可通过配置调整

## 运行时观测

`status --json` 当前可直接看到：

- 活动会话的累计上下行字节：`bytes_up` / `bytes_down`
- 活动会话的 1 秒采样吞吐：`throughput_up_bps` / `throughput_down_bps`
- 最近完成会话的结算记录：`recent_completed_sessions`
- 上游 UDP association 的统计和空闲超时

这里的口径固定为：

- `bytes_*` 是事实源
- `throughput_*_bps` 是 1 秒采样吞吐
- 完成日志和完成历史只记录结算值，不记录平均速率

## 快速开始

构建：

```powershell
cargo build
```

本地运行：

```powershell
cargo run -- run examples/v0.0.1/basic.json
```

默认示例会在 `127.0.0.1:7890` 启一个 `mixed` 入站，同时接 `SOCKS5` 和 `HTTP CONNECT`。

云端最小节点：

```powershell
cargo run -- run examples/v0.0.1/server-socks5.json
```

这个示例会在 `0.0.0.0:7890` 提供一个 `SOCKS5` 入站，可作为最小远端节点。

带本地状态端点运行：

```powershell
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.0.1/basic.json
```

查看状态：

```powershell
cargo run -- status --json examples/v0.0.1/basic.json
```

如果本机装了 `make`，也可以直接用：

```powershell
make run
make status-json
make test
```

## 示例配置

- `examples/v0.0.1/basic.json`
- `examples/v0.0.1/mixed.json`
- `examples/v0.0.1/blocked-route.json`
- `examples/v0.0.1/chained-socks5.json`
- `examples/v0.0.1/global-selector.json`
- `examples/v0.0.1/server-socks5.json`
- `examples/v0.0.1/udp-socks5.json`

## 常用命令

```powershell
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo build --release
```

## 目录

- `src/`：根程序入口
- `crates/config`：配置模型和校验
- `crates/engine`：执行内核
- `crates/router`：路由规则
- `crates/core`：通用类型和接口
- `crates/traits`：运行时无关抽象
- `crates/platform/tokio`：Tokio 后端
- `protocols/`：外部协议实现
- `examples/`：示例配置
- `docs/`：项目和版本文档

## 文档入口

- `docs/project/`：长期规则和分层说明
- `docs/project/config.md`：当前配置格式
- `docs/project/logging.md`：日志字段和观测口径
- `docs/versions/v0.0.1/`：当前版本范围、验收和发布说明

建议先看：

- `docs/versions/v0.0.1/release-notes.md`
- `docs/versions/v0.0.1/known-limitations.md`
- `docs/project/config.md`

## 当前状态

当前仍处于 `v0.0.x` 预发布阶段。目标不是宣称功能完整，而是先把本地可用、云端可部署、主代理链路、UDP 基础能力和会话观测做稳，再继续往正式版本推进。
