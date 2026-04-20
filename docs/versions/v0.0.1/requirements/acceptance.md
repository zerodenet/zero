# 验收

这版至少要满足下面这些。

## 可用性

1. [basic.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/basic.json) 启动后，用户能直接通过 `127.0.0.1:7890` 使用 `SOCKS5` 或 `HTTP CONNECT`
2. [server-socks5.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/server-socks5.json) 启动后，能作为远端 `SOCKS5` 节点被另一个 `zero` 的链式出站接入
3. 同端口多协议入站必须可用，当前至少覆盖同一监听口下的 `SOCKS5` 和 `HTTP CONNECT`

## 功能

1. `SOCKS5 -> direct` 可用
2. `HTTP CONNECT -> direct` 可用
3. `mixed` 同端口下能同时处理 `SOCKS5` 和 `HTTP CONNECT`
4. 域名规则能正确走 `direct` 或 `block`
5. CIDR 规则能正确走 `direct` 或 `block`
6. `SOCKS5` 链式出站可用
7. `SOCKS5 UDP ASSOCIATE -> direct` 可用
8. `SOCKS5 UDP ASSOCIATE -> 上游 SOCKS5` 可用
9. 规则没命中时能走 `final`

## 工程

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- 配置错误时能给出可读信息
- 关闭信号能让监听器停下来
- 日志能看出入站、规则、出站和结果
- 至少提供一份本地示例配置和一份云端示例配置

## 分层

- `zero-core` 和 `zero-traits` 不依赖 Tokio
- 协议实现不塞进根包 `zero`
- 路由逻辑不散落在各协议实现里
