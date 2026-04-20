# 日志

默认不用配 `RUST_LOG`，程序会按 `info` 打关键日志。

如果要调高或调低级别，再自己配：

```powershell
$env:RUST_LOG="debug"
cargo run -- run examples/v0.1.0/basic.json
```

日志至少要能看出这些事：

- 配置有没有读进来
- 引擎有没有启动
- 入站监听有没有起来
- 会话走的是哪个入站、哪个出站
- 请求是成功、阻断还是失败
- 引擎退出时统计是什么样

会话日志里的关键字段：

- `session_id`
- `protocol`
- `inbound_tag`
- `outbound_tag`
- `target`
- `port`
- `duration_ms`
- `bytes_from_client`
- `bytes_to_client`
- `stage`
