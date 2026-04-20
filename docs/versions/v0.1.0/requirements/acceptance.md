# 验收

这版至少要满足下面这些。

## 功能

1. `SOCKS5 -> direct` 可用
2. `HTTP CONNECT -> direct` 可用
3. `mixed` 同端口下能同时处理 `SOCKS5` 和 `HTTP CONNECT`
4. 域名规则能正确走 `direct` 或 `block`
5. CIDR 规则能正确走 `direct` 或 `block`
6. `SOCKS5` 链式出站可用
7. 规则没命中时能走 `final`

## 工程

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- 配置错误时能给出可读信息
- 关闭信号能让监听器停下来
- 日志能看出入站、规则、出站和结果

## 分层

- `zero-core` 和 `zero-traits` 不依赖 Tokio
- 协议实现不塞进根包 `zero`
- 路由逻辑不散落在各协议实现里
