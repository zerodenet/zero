# Zero

Zero 是一个用 Rust 写的网络代理项目。

当前仓库以 `v0.1.0` 为目标，先交付一个可运行的最小 TCP 代理工具，并把配置、执行内核、协议实现、路由和平台适配这几层关系跑顺。

## 当前支持

- `SOCKS5` 入站
- `HTTP CONNECT` 入站
- `mixed` 入站，同端口识别 `SOCKS5` 和 `HTTP CONNECT`
- `direct` 出站
- `block` 出站
- `SOCKS5` 链式出站
- 基于域名和 CIDR 的静态路由
- 结构化日志
- 本地只读状态导出

当前版本只支持 `TCP`，不支持 `UDP`、`TUN`、Shadowsocks、Trojan、VLESS、VMess。

## 快速开始

构建：

```powershell
cargo build
```

直接运行：

```powershell
cargo run -- run examples/v0.1.0/basic.json
```

默认示例会在 `127.0.0.1:7890` 开一个 `mixed` 入站，同时接 `SOCKS5` 和 `HTTP CONNECT`。

运行并暴露本地状态端点：

```powershell
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.1.0/basic.json
```

查看状态：

```powershell
cargo run -- status --json examples/v0.1.0/basic.json
```

如果本机装了 `make`，也可以直接用：

```powershell
make run
make status-json
make test
```

## 示例配置

- `examples/v0.1.0/basic.json`
- `examples/v0.1.0/mixed.json`
- `examples/v0.1.0/blocked-route.json`
- `examples/v0.1.0/chained-socks5.json`

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
- `docs/versions/v0.1.0/`：当前版本范围、验收和发布文档

建议先看：

- `docs/versions/v0.1.0/release-notes.md`
- `docs/versions/v0.1.0/known-limitations.md`
- `docs/project/config.md`

## 当前状态

`v0.1.0` 已经完成主链路实现和基础发布验证，适合继续做 bugfix、测试补强和发布收口。
