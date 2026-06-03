# 日志

默认不配 `RUST_LOG` 也会输出 `info` 级别日志。需要更细的调试信息时，再自己调高：

```powershell
$env:RUST_LOG="debug"
cargo run -- run examples/v0.0.1/basic.json
```

## 目标

日志至少要能回答这些问题：

- 配置有没有读进来
- 引擎有没有启动
- 入站监听有没有起来
- 会话走的是哪个入站、哪个出站
- 请求是成功、阻断还是失败
- 引擎退出时累计统计是什么样

## 会话日志

会话日志分两类：

- `session accepted`
  - 表示请求已进入内核并完成路由决策
- `session finished`
  - 表示请求已经完成结算
- `session failed`
  - 表示请求在某个阶段失败，并带上当时已累计的结算字段

关键字段：

- `session_id`
- `protocol`
- `network`
- `mode`
- `inbound_tag`
- `outbound_tag`
- `target`
- `port`
- `duration_ms`
- `bytes_up`
- `bytes_down`
- `inbound_rx_bytes`
- `inbound_tx_bytes`
- `outbound_rx_bytes`
- `outbound_tx_bytes`
- `stage`

这里的口径固定为：

- `bytes_up` / `bytes_down`：会话视角累计字节
- `inbound_*` / `outbound_*`：链路视角累计字节
- 这些字节是应用层链路字节，包含 SOCKS5 / HTTP CONNECT 握手和 SOCKS5 UDP 封包头
- 不包含 TCP/IP 包头、TCP 建连和挥手等内核网络栈开销
- 完成日志只记录结算值，不记录平均速率

## 状态导出

运行时状态导出里，活动会话还会额外提供：

- `throughput_up_bps`
- `throughput_down_bps`

这两个字段是 1 秒采样吞吐，不是平均速率。

## UDP 上游 association 日志

上游 UDP association 会记录这些事件：

- 创建：`info`
- 空闲超时回收：`info`
- 异常丢弃：`warn`
- 复用已有 association：`debug`

相关字段：

- `protocol=socks5-udp`
- `inbound_tag`
- `outbound_tag`
- `upstream_server`
- `upstream_port`
- `idle_timeout_seconds`
- `error`

## File log sinks

Runtime config can define file log sinks under `runtime.log.files`. File sinks use non-blocking tracing appenders and keep their worker guards alive for the process lifetime, so configured log files are written after startup instead of staying empty.

When `max_bytes` and `max_files` are configured, rotation is checked before writing a new oversized entry. This keeps the active file within the configured size boundary except for a single log entry that is itself larger than `max_bytes`.
