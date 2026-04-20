# 配置

`v0.1.0` 用 JSON。顶层只有三段：

```json
{
  "inbounds": [],
  "outbounds": [],
  "route": {
    "rules": [],
    "final": { "type": "direct" }
  }
}
```

## 入站

每个入站都要写 `tag`、`listen`、`protocol`。

```json
{
  "tag": "mixed-in",
  "listen": { "address": "127.0.0.1", "port": 7890 },
  "protocol": { "type": "mixed" }
}
```

支持的入站类型：

- `socks5`
- `http-connect`
- `http`，兼容别名
- `mixed`，同端口识别 `socks5` 和 `http-connect`

## 出站

```json
{
  "tag": "chain",
  "protocol": {
    "type": "socks5",
    "server": "127.0.0.1",
    "port": 2080
  }
}
```

支持的出站：

- `direct`
- `block`
- `socks5`

## 路由

规则是 `condition + action`：

```json
{
  "condition": { "type": "domain", "values": ["blocked.example"] },
  "action": { "type": "route", "outbound": "block" }
}
```

条件：

- `domain`
- `ip`
- `and`
- `or`

动作：

- `direct`
- `reject`
- `block`，兼容别名
- `route`

## 约束

- `tag` 不能为空
- 同类对象里的 `tag` 不能重复
- 同一个 `address:port` 只能有一个入站
- 同端口同时接 `socks5` 和 `http-connect` 时，用 `mixed`
- `route` 引用的 `outbound` 必须存在
- 规则按顺序匹配，没命中就走 `final`

## 示例

- [basic.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.1.0/basic.json)
- [mixed.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.1.0/mixed.json)
- [blocked-route.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.1.0/blocked-route.json)
- [chained-socks5.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.1.0/chained-socks5.json)

## 命令

- `cargo run --`
- `cargo run -- run path/to/config.json`
- `cargo run -- run --status-listen 127.0.0.1:9090 path/to/config.json`
- `cargo run -- status path/to/config.json`
- `cargo run -- status --json path/to/config.json`
