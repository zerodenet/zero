# 配置

`v0.0.2` 使用 JSON。当前顶层结构是：

```json
{
  "inbounds": [],
  "outbounds": [],
  "outbound_groups": [],
  "runtime": {
    "udp_upstream_idle_timeout_seconds": 30
  },
  "api": {
    "event_sinks": [],
    "control": { "enabled": false }
  },
  "mode": { "type": "rule" },
  "route": {
    "rule_sets": [],
    "rules": [],
    "final": { "type": "direct" }
  }
}
```

这里只写当前已经实现的配置。模式和节点组的长期设计见 [modes-and-groups.md](modes-and-groups.md)。

## runtime

`runtime.udp_upstream_idle_timeout_seconds` 控制上游 `SOCKS5` UDP association 的空闲超时。

- 默认：`30`
- 单位：秒
- 约束：必须大于 `0`

## api

`api` 是可选控制面和观测面配置。相关运行能力由 Cargo feature 控制；配置存在不代表默认构建一定支持。

### event_sinks

`api.event_sinks` 描述归一化事件的投递目标。事件类型必须来自 [api.md](api.md) 的事件目录。

本地 JSON Lines：

```json
{
  "tag": "local-events",
  "type": "jsonl",
  "path": "zero-events.jsonl",
  "events": ["flow.completed"],
  "source_id": "edge-local"
}
```

面板 webhook：

```json
{
  "tag": "panel",
  "type": "webhook",
  "url": "https://panel.example.com/api/zero/events",
  "events": ["flow.completed", "engine.warning"],
  "source_id": "edge-shanghai-01",
  "api_key_env": "ZERO_PANEL_API_KEY"
}
```

`webhook` 使用 `Authorization: Bearer <api-key>`。推荐使用 `api_key_env`，也支持 `api_key` 便于测试。`http://` webhook 需要显式设置 `allow_insecure: true`。

### control

`api.control` 用于面板主动调用节点查询和 command。它默认关闭，开启时必须配置 API key：

```json
{
  "enabled": true,
  "listen": { "address": "127.0.0.1", "port": 9090 },
  "api_key_env": "ZERO_NODE_API_KEY"
}
```

当前控制面使用 `Authorization: Bearer <api-key>` 或 `X-Zero-Api-Key: <api-key>`。建议只监听本机、内网或受防火墙保护的地址。

当前 HTTP 控制面支持：

```text
GET  /api/v1/status
GET  /api/v1/config
GET  /api/v1/runtime
GET  /api/v1/events
POST /api/v1/commands
POST /api/v1/selectors/{group}/{target}
```

`POST /api/v1/commands` 使用统一 command JSON，例如：

```json
{
  "method": "policies.select",
  "params": {
    "policy_tag": "proxy",
    "target_tag": "direct"
  }
}
```

## 入站

每个入站都要有 `tag`、`listen`、`protocol`：

```json
{
  "tag": "mixed-in",
  "listen": { "address": "127.0.0.1", "port": 7890 },
  "protocol": { "type": "mixed" }
}
```

当前支持：

- `socks5`
- `http-connect`
- `http`，兼容别名
- `mixed`，同端口识别 `socks5` 和 `http-connect`
- `vless`，当前支持 TCP，可选 TLS 包裹

`mixed` 不是外部协议，而是“同端口多协议入站”的配置入口。

SOCKS5 入站默认是 no-auth。配置 `users` 后会启用 RFC 1929 username/password：

```json
{
  "tag": "socks-in",
  "listen": { "address": "127.0.0.1", "port": 7890 },
  "protocol": {
    "type": "socks5",
    "users": [
      { "username": "alice", "password": "secret" }
    ]
  }
}
```

`mixed` 入站也可以给 SOCKS5 分支配置认证：

```json
{
  "tag": "mixed-in",
  "listen": { "address": "127.0.0.1", "port": 7890 },
  "protocol": {
    "type": "mixed",
    "socks5_users": [
      { "username": "alice", "password": "secret" }
    ]
  }
}
```

VLESS 入站必须配置用户 UUID。`credential_id` 和 `principal_key` 是观测归因字段，会出现在 `flow.completed` 的 `auth` 和事件顶层 `principal_key` 中；UUID 本身不会默认回传给面板：

```json
{
  "tag": "vless-in",
  "listen": { "address": "127.0.0.1", "port": 8443 },
  "protocol": {
    "type": "vless",
    "users": [
      {
        "id": "11111111-2222-3333-4444-555555555555",
        "credential_id": "node-user-1",
        "principal_key": "user:10001"
      }
    ]
  }
}
```

VLESS 入站需要 TLS 时，在协议内增加 `tls`：

```json
{
  "tag": "vless-tls-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "vless",
    "users": [
      { "id": "11111111-2222-3333-4444-555555555555" }
    ],
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    }
  }
}
```

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

当前支持：

- `direct`
- `block`
- `socks5`
- `vless`

SOCKS5 出站默认 no-auth。连接需要认证的上游时配置 `username` 和 `password`：

```json
{
  "tag": "chain",
  "protocol": {
    "type": "socks5",
    "server": "127.0.0.1",
    "port": 2080,
    "username": "upstream",
    "password": "secret"
  }
}
```

VLESS 出站用于连接上游 VLESS TCP 节点：

```json
{
  "tag": "vless-chain",
  "protocol": {
    "type": "vless",
    "server": "203.0.113.10",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555"
  }
}
```

连接 TLS VLESS 上游时配置 `tls`。`server_name` 默认使用 `server`，自签或私有 CA 可通过 `ca_cert_path` 指定：

```json
{
  "tag": "vless-tls-chain",
  "protocol": {
    "type": "vless",
    "server": "edge.example.com",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555",
    "tls": {
      "server_name": "edge.example.com",
      "ca_cert_path": "certs/ca.pem"
    }
  }
}
```

UDP 当前只支持 `direct`、`block` 和上游 `socks5`，暂不支持上游 `vless`。

## 出站组

当前已经实现三类出站组：

- `selector`
- `fallback`
- `urltest`

组成员当前既可以是具体出站，也可以是另一个出站组。配置阶段会拦掉循环引用。

### selector

```json
{
  "tag": "proxy",
  "type": "selector",
  "outbounds": ["node-a", "node-b"],
  "selected": "node-a"
}
```

`selector` 当前支持运行时切换。启动时带上 `--status-listen` 后，可通过本地端点：

```text
POST /selectors/{group_tag}/{target_tag}
```

例如：

```text
POST /selectors/proxy/direct
```

切换成功后，`/config` 和 `/status` 里的 `outbound_groups[*].selected` 会立刻反映最新选择。

### fallback

```json
{
  "tag": "proxy",
  "type": "fallback",
  "outbounds": ["node-a", "direct"]
}
```

语义：

- 按配置顺序尝试成员
- 前一个成员建连失败时，自动切到下一个
- 本次会话一旦建连成功，就固定使用该成员

### urltest

```json
{
  "tag": "proxy",
  "type": "urltest",
  "outbounds": ["node-a", "node-b", "direct"],
  "url": "http://example.com/",
  "interval_seconds": 300
}
```

语义：

- 按 `interval_seconds` 定时探测
- 当前只支持 `http://` 探测地址
- 选取探测成功且延迟最小的成员
- 如果本轮都失败，保留当前选择；首次探测前默认落到第一个成员

## 模式

当前支持：

- `rule`
- `global`
- `direct`

`global` 需要引用一个出站或出站组：

```json
{
  "type": "global",
  "outbound": "proxy"
}
```

## 路由

规则是 `condition + action`：

```json
{
  "condition": { "type": "domain", "values": ["blocked.example"] },
  "action": { "type": "route", "outbound": "block" }
}
```

当前条件：

- `domain`
- `ip`
- `rule-set`
- `and`
- `or`

当前动作：

- `direct`
- `reject`
- `block`，兼容别名
- `route`

## 外置规则集

当前支持把匹配数据放到本地文件里，再在主配置里通过 `tag` 引用。

当前只支持：

- `type = file`
- `format = domain-list`
- `format = cidr-list`

```json
{
  "route": {
    "rule_sets": [
      {
        "tag": "ads",
        "type": "file",
        "path": "rules/ads.txt",
        "format": "domain-list"
      },
      {
        "tag": "lan",
        "type": "file",
        "path": "rules/lan.txt",
        "format": "cidr-list"
      }
    ],
    "rules": [
      {
        "condition": { "type": "rule-set", "tag": "ads" },
        "action": { "type": "reject" }
      },
      {
        "condition": { "type": "rule-set", "tag": "lan" },
        "action": { "type": "route", "outbound": "direct" }
      }
    ],
    "final": { "type": "route", "outbound": "proxy" }
  }
}
```

说明：

- `path` 支持相对路径，默认相对配置文件所在目录解析
- `domain-list` 按域名列表加载
- `cidr-list` 按 CIDR 列表加载
- 空行会忽略
- 以 `#` 或 `//` 开头的行会忽略
- 规则文件只负责匹配数据，不负责动作

## 状态字段口径

`status --json` 当前和会话观测相关的字段口径是：

- `bytes_up` / `bytes_down`
  - flow 视角累计应用层链路字节
  - 包含 SOCKS5 / HTTP CONNECT 握手、SOCKS5 UDP 封包头和转发 payload
  - 不包含 TCP/IP 包头、TCP 三次握手等内核网络栈开销
  - TCP 按连接统计，SOCKS5 UDP 按目标 flow 统计
- `inbound_rx_bytes` / `inbound_tx_bytes`
  - 入站侧实际读写的应用层字节
- `outbound_rx_bytes` / `outbound_tx_bytes`
  - 出站侧实际读写的应用层字节
- `throughput_up_bps` / `throughput_down_bps`
  - 1 秒采样吞吐
- `recent_completed_sessions`
  - 最近完成会话的结算记录
  - TCP 连接和 SOCKS5 UDP flow 使用同一套字段
- `outbound_groups[*].selected`
  - 当前组选择的成员
- `outbound_groups[*].latency_ms`
  - `urltest` 最近一次成功探测的延迟
- `outbound_groups[*].last_checked_unix_ms`
  - `urltest` 最近一次完成探测的时间

## 约束

- `tag` 不能为空
- SOCKS5 username/password 不能为空，且单项最多 255 字节
- SOCKS5 出站认证必须同时配置 `username` 和 `password`，不能只配其中一个
- VLESS 入站必须至少配置一个用户，`id` 必须是 UUID；启用 TLS 时 `cert_path` 和 `key_path` 不能为空
- VLESS 出站的 `server` 不能为空，`port` 必须大于 `0`，`id` 必须是 UUID；`tls.server_name` 和 `tls.ca_cert_path` 如果配置则不能为空
- 同类对象里的 `tag` 不能重复
- 同一个 `address:port` 只能有一个入站
- 同端口同时接 `socks5` 和 `http-connect` 时，用 `mixed`
- `route` 和 `global mode` 引用的目标必须存在
- 出站组里的成员必须是已定义出站或已定义组
- 出站组不允许循环引用
- `runtime.udp_upstream_idle_timeout_seconds` 必须大于 `0`
- `rule_sets[*].tag` 不能为空且不能重复
- `rule-set` 条件引用的 `tag` 必须存在
- `urltest.url` 当前必须是 `http://`
- `urltest.interval_seconds` 必须大于 `0`

## 示例

- [basic.json](../../examples/v0.0.1/basic.json)
- [mixed.json](../../examples/v0.0.1/mixed.json)
- [blocked-route.json](../../examples/v0.0.1/blocked-route.json)
- [chained-socks5.json](../../examples/v0.0.1/chained-socks5.json)
- [global-selector.json](../../examples/v0.0.1/global-selector.json)
- [rule-set-files.json](../../examples/v0.0.1/rule-set-files.json)
- [server-socks5.json](../../examples/v0.0.1/server-socks5.json)
- [udp-socks5.json](../../examples/v0.0.1/udp-socks5.json)
- [fallback.json](../../examples/v0.0.2/fallback.json)
- [nested-groups.json](../../examples/v0.0.2/nested-groups.json)
- [urltest.json](../../examples/v0.0.2/urltest.json)
- [vless.json](../../examples/v0.0.2/vless.json)
- [vless-tls.json](../../examples/v0.0.2/vless-tls.json)
- [chained-vless-tls.json](../../examples/v0.0.2/chained-vless-tls.json)
