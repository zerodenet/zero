# Zero

Zero 是一个用 Rust 编写的网络代理项目。

当前仓库处于 `v0.0.x` 预发布线，当前版本是 `v0.0.2`。`v0.1.0` 保留给第一次正式发布；在那之前，重点是把本地可用、云端可部署、主代理链路、基础观测和节点组能力收稳。

## 当前支持

- `SOCKS5` 入站
- `HTTP CONNECT` 入站
- `mixed` 同端口多协议入站
- `direct` / `block` / 上游 `SOCKS5` 出站
- `SOCKS5 UDP ASSOCIATE`
- `mode = rule | global | direct`
- `selector` / `fallback` / `urltest` 出站组
- `selector` 运行时切换
- 基于域名、CIDR 和外置 `rule_sets` 的静态路由
- 结构化日志、状态导出、活动会话和最近完成会话

当前支持 `TCP`，也支持通过 `SOCKS5 UDP ASSOCIATE` 进入的 `UDP`。UDP 当前可走 `direct`、`block` 和上游 `SOCKS5`。

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

状态输出：

```powershell
cargo run -- status --json examples/v0.0.1/basic.json
```

带本地状态端点运行：

```powershell
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.0.1/basic.json
```

运行时切换 `selector` 组成员：

```powershell
curl -X POST http://127.0.0.1:9090/selectors/proxy/direct
```

如果本机装了 `make`，也可以直接用：

```powershell
make run
make status-json
make test
```

## 配置示例

- `examples/v0.0.1/basic.json`
- `examples/v0.0.1/mixed.json`
- `examples/v0.0.1/blocked-route.json`
- `examples/v0.0.1/chained-socks5.json`
- `examples/v0.0.1/global-selector.json`
- `examples/v0.0.1/rule-set-files.json`
- `examples/v0.0.1/server-socks5.json`
- `examples/v0.0.1/udp-socks5.json`
- `examples/v0.0.2/fallback.json`
- `examples/v0.0.2/urltest.json`

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
- `crates/config`：配置模型、校验和规则集装载
- `crates/engine`：执行内核
- `crates/router`：路由规则匹配
- `crates/core`：通用类型和领域模型
- `crates/traits`：平台能力抽象
- `crates/platform/tokio`：Tokio 后端
- `protocols/`：外部协议实现
- `examples/`：示例配置
- `docs/`：项目和版本文档

## 文档入口

- [配置说明](/C:/Users/Administrator/develop/rs/zero-new/docs/project/config.md)
- [模式和节点组](/C:/Users/Administrator/develop/rs/zero-new/docs/project/modes-and-groups.md)
- [日志说明](/C:/Users/Administrator/develop/rs/zero-new/docs/project/logging.md)
- [版本索引](/C:/Users/Administrator/develop/rs/zero-new/docs/versions/README.md)
- [v0.0.2](/C:/Users/Administrator/develop/rs/zero-new/docs/versions/v0.0.2/README.md)

## 当前状态

`v0.0.1` 已经封住最小代理基础；`v0.0.2` 继续补节点组能力，当前已经落地：

- `selector` 运行时切换
- `fallback`
- `urltest`

当前还没有进入正式产品发布阶段，`v0.1.0` 仍然保留给第一次正式版。
