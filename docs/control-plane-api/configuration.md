# 配置模型参考

控制面所有配置位于 `api` 键下。

## 完整示例

```json
{
  "api": {
    "control": {
      "enabled": true,
      "listen": { "address": "127.0.0.1", "port": 9090 },
      "api_key": "sk-secret"
    },
    "hooks": [
      { "type": "ipc", "socket": "/run/billing/hook.sock", "timeout_ms": 100 }
    ],
  "push": {
    "url": "https://receiver.example.com",
    "node_id": "node-001",
    "api_key": "sk-xxx",
    "heartbeat_interval_seconds": 30,
    "pull_commands": true,
    "command_poll_interval_seconds": 10
  },
    "event_sinks": [
      {
        "type": "jsonl",
        "tag": "audit",
        "path": "/var/log/zero/events.jsonl",
        "events": ["flow.completed", "engine.warning"]
      },
      {
        "type": "webhook",
        "tag": "billing",
        "url": "https://billing.example.com/events",
        "events": ["flow.completed"]
      }
    ]
  }
}
```

## `api.control`

本地 HTTP 控制接口。

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `enabled` | bool | `false` | 是否启动 HTTP 控制服务器 |
| `listen` | object | — | 监听地址，`enabled=true` 时必填 |
| `listen.address` | string | — | 绑定 IP，`127.0.0.1` 仅本地，`0.0.0.0` 公网 |
| `listen.port` | u16 | — | 监听端口 |
| `api_key` | string | — | Bearer token，不设则无认证（仅建议本地使用） |
| `api_key_env` | string | — | 从环境变量读取 api_key，优先级低于 `api_key` |

**CLI 覆盖**：`--status-listen 127.0.0.1:9090` 优先于配置文件。二者不能同时使用。

### 限流

内建限流，无需配置：

| 类别 | 限制 | 响应 |
|------|------|------|
| Query (GET) | 100 req/s | 429 Too Many Requests |
| Command (POST) | 10 req/s | 429 Too Many Requests |
| SSE 并发 | 5 连接 | 429 Too Many Requests |

## `api.hooks`

Flow 生命周期钩子，按数组顺序执行。

```json
{ "type": "ipc", "socket": "/run/billing/hook.sock", "timeout_ms": 100 }
```

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `type` | string | — | 钩子类型，当前仅 `"ipc"` |
| `socket` | string | — | IPC socket 路径 |
| `timeout_ms` | u64 | `100` | 请求超时（毫秒），超时 fail-open |

**CLI 覆盖**：`--ipc-hook-socket /run/billing/hook.sock` 优先于配置文件。

钩子协议详见 [hooks.md](./hooks.md)。

## `push`

节点主动上报到外部管理端点。接收端可以是面板、监控系统或任意 HTTP 服务。

```json
{
  "push": {
    "url": "https://receiver.example.com",
    "node_id": "node-001",
    "api_key": "sk-xxx",
    "heartbeat_interval_seconds": 30,
    "pull_commands": true,
    "command_poll_interval_seconds": 10
  }
}
```

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `url` | string | — | 接收端 URL，设置后启用 push |
| `node_id` | string | — | 本节点标识 |
| `api_key` | string | — | 认证密钥 |
| `api_key_env` | string | — | 从环境变量读取 api_key |
| `heartbeat_interval_seconds` | u64 | `30` | 心跳间隔 |
| `pull_commands` | bool | `false` | 是否轮询远程命令 |
| `command_poll_interval_seconds` | u64 | `10` | 命令轮询间隔 |

协议详见 [push-connector.md](./push-connector.md)。

## `api.event_sinks`

事件投递目标数组。

### JSON Lines 文件

```json
{
  "type": "jsonl",
  "tag": "audit",
  "path": "/var/log/zero/events.jsonl",
  "events": ["flow.completed"],
  "source_id": "node-001"
}
```

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `type` | string | — | `"jsonl"` 或别名 `"file"` |
| `tag` | string | — | 唯一标识 |
| `path` | string | — | 文件路径，相对路径基于配置目录 |
| `events` | string[] | `[]` | 事件类型白名单，空=全收 |
| `source_id` | string | — | 覆盖事件的 source_id |

### Webhook

```json
{
  "type": "webhook",
  "tag": "billing",
  "url": "https://example.com/events",
  "events": ["flow.completed"],
  "api_key": "sk-xxx",
  "api_key_env": "WEBHOOK_KEY"
}
```

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `type` | string | — | `"webhook"` |
| `tag` | string | — | 唯一标识 |
| `url` | string | — | 接收端点 |
| `events` | string[] | `[]` | 事件类型白名单 |
| `api_key` | string | — | 请求头 `Authorization: Bearer {key}` |
| `api_key_env` | string | — | 从环境变量读取 |
| `allow_insecure` | bool | `false` | 跳过 TLS 证书校验（仅测试用） |

投递失败自动重试（指数退避 2s→4s→8s→...→64s，最多 6 次）。

## `api.dead_letter_path`

死信队列文件路径。超出最大重试次数的事件不会丢弃，而是写入此文件持久化。

| 字段 | 类型 | 说明 |
|------|------|------|
| `dead_letter_path` | string | 死信 JSON Lines 文件路径，不设置则事件最终丢弃 |

死信文件格式：每行一个 JSON，包含 `dead_lettered_at_unix_ms` 和 `original_event`。

### 投递状态查询

```bash
zero status  # 包含 sink 投递统计
```
