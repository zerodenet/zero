# 工程规则

## 命名

- package 名用 `zero-*`
- 目录名保持短名，比如 `crates/engine`、`protocols/socks5`
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
- `zero` 根包只负责把 feature 转发到 `zero-engine`
- 嵌入式或裁剪场景优先使用 `--no-default-features` 再按需开启模块

当前约定的可选 feature：

- `inbound-socks5`
- `inbound-http-connect`
- `inbound-mixed`
- `outbound-socks5`
- `status-api`

## 代码边界

- `zero-traits` 和 `zero-core` 不绑 Tokio
- 协议实现放 `protocols/*`
- `direct`、`block` 留在 `zero-engine`
- 根包 `zero` 不要塞协议细节

## 文档边界

- 改配置格式，要更新配置文档和示例
- 改协议范围，要更新版本文档
- 改分层，要更新 `docs/project/`
