# 配置

`v0.0.1` 使用 JSON。当前顶层固定这几段：

```json
{
  "inbounds": [],
  "outbounds": [],
  "outbound_groups": [],
  "runtime": {
    "udp_upstream_idle_timeout_seconds": 30
  },
  "mode": { "type": "rule" },
  "route": {
    "rule_sets": [],
    "rules": [],
    "final": { "type": "direct" }
  }
}
```

这份文档只写当前已经实现的配置。模式和节点组的长期设计见 [modes-and-groups.md](/C:/Users/Administrator/develop/rs/zero-new/docs/project/modes-and-groups.md)。

## runtime

`runtime.udp_upstream_idle_timeout_seconds` 控制上游 `SOCKS5` UDP association 的空闲超时。

- 默认值：`30`
- 单位：秒
- 约束：必须大于 `0`

```json
{
  "runtime": {
    "udp_upstream_idle_timeout_seconds": 15
  }
}
```

## 入站

每个入站都要写 `tag`、`listen`、`protocol`：

```json
{
  "tag": "mixed-in",
  "listen": { "address": "127.0.0.1", "port": 7890 },
  "protocol": { "type": "mixed" }
}
```

当前支持的入站类型：

- `socks5`
- `http-connect`
- `http`，兼容别名
- `mixed`，同端口识别 `socks5` 和 `http-connect`

这里的 `mixed` 不是独立外部协议，而是“同端口多协议入站”的配置入口。

`UDP` 当前不需要额外字段。只要入站是 `socks5` 或 `mixed`，客户端走 `SOCKS5 UDP ASSOCIATE` 即可。

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

当前支持的出站：

- `direct`
- `block`
- `socks5`

当前 UDP 只支持：

- `direct`
- `block`
- 上游 `socks5`

## 出站组

`v0.0.1` 当前只实现了一类：

```json
{
  "tag": "proxy",
  "type": "selector",
  "outbounds": ["node-a", "node-b"],
  "selected": "node-a"
}
```

- `selector`

## 模式

当前支持：

- `rule`
- `global`
- `direct`

`global` 需要指定一个出站或出站组：

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

条件：

- `domain`
- `ip`
- `rule-set`
- `and`
- `or`

动作：

- `direct`
- `reject`
- `block`，兼容别名
- `route`

### 外置规则集

`v0.0.1` 支持把匹配数据放到本地文件里，再在主配置里通过 `tag` 引用。

当前只支持：

- `type = file`
- `format = domain-list`
- `format = cidr-list`

配置形态：

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
  - 累计字节
- `throughput_up_bps` / `throughput_down_bps`
  - 1 秒采样吞吐
- `recent_completed_sessions`
  - 最近完成会话的结算记录

完成会话只保留结算值，不保留平均速率字段。

## 约束

- `tag` 不能为空
- 同类对象里的 `tag` 不能重复
- 同一个 `address:port` 只能有一个入站
- 同端口同时接 `socks5` 和 `http-connect` 时，用 `mixed`
- `route` 和 `global mode` 引用的目标必须存在
- `selector` 组里的成员必须是已定义的出站
- `runtime.udp_upstream_idle_timeout_seconds` 必须大于 `0`
- `rule_sets[*].tag` 不能为空且不能重复
- `rule-set` 条件引用的 `tag` 必须存在
- `rule_sets` 当前只支持本地文件
- 规则按顺序匹配，没命中就走 `final`

## 最小场景

- 本地用户侧：[basic.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/basic.json)，默认监听 `127.0.0.1:7890`
- 云端节点侧：[server-socks5.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/server-socks5.json)，默认监听 `0.0.0.0:7890`

## 示例

- [basic.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/basic.json)
- [mixed.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/mixed.json)
- [blocked-route.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/blocked-route.json)
- [chained-socks5.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/chained-socks5.json)
- [global-selector.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/global-selector.json)
- [rule-set-files.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/rule-set-files.json)
- [server-socks5.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/server-socks5.json)
- [udp-socks5.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.0.1/udp-socks5.json)

## 命令

- `cargo run --`
- `cargo run -- run path/to/config.json`
- `cargo run -- run --status-listen 127.0.0.1:9090 path/to/config.json`
- `cargo run -- status path/to/config.json`
- `cargo run -- status --json path/to/config.json`
