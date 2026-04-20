# 发布前检查

发布前至少过一遍这些：

- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- `cargo build --release`

自动化测试至少要覆盖：

- `SOCKS5 -> direct`
- `HTTP CONNECT -> direct`
- `mixed` 同端口入站
- `SOCKS5` 链式出站
- 配置错误输出

手工再看一遍：

- `zero status --json`
- `run --status-listen`
- `curl` 经 `SOCKS5`
- `curl` 经 `HTTP CONNECT`
- 阻断规则是否真的生效
