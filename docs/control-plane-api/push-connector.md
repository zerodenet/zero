# Push Connector

Push Connector 是节点主动上报到外部管理端点的组件，支持心跳上报和远程命令接收。
接收端可以是面板、监控系统或任意 HTTP 服务。

## 架构

```
┌──────────────────────┐         HTTP POST        ┌──────────────────────┐
│    Zero Node          │ ────── heartbeat ──────> │    Receiver           │
│                      │ <────── commands ─────── │                      │
│  PushConnector       │                          │  /api/v1/nodes/       │
│    heartbeat loop    │                          │    {id}/heartbeat     │
│    command poll      │                          │    {id}/commands      │
└──────────────────────┘                          └──────────────────────┘
```

单向主动连接：节点 → 接收端。接收端不主动连接节点，命令通过以下两种方式投递：

1. **心跳响应嵌入** — 接收端在心跳响应中直接返回待执行命令
2. **轮询拉取** — 节点定时 GET 接收端的命令端点

## 与 GUI 和事件 Sink 的职责边界

`zero-connector` crate 同时承载 Push Connector 和 EventDispatcher，但两者不是同一条数据链路，也不替代 GUI 控制连接：

| 组件 | 上游能力 | 对外职责 | 是否消费 flow 实时事件 |
|------|----------|----------|------------------------|
| GUI connection（IPC/HTTP/gRPC） | `QueryService`、`CommandService`、`EventSource` | 本地或远程交互、状态查询、命令和实时界面更新 | 是；订阅建立时可先收到 `flow.snapshot` |
| EventDispatcher（JSONL/Webhook Sink） | `EventSource` | 过滤、序列化、重试、死信和持久事件投递 | 是；只投递生命周期增量，不投递 `flow.snapshot` |
| PushConnector（Panel Connector） | 窄统计提供器、`CommandService` | 周期心跳、节点汇总统计和远程命令接收 | 否 |

因此，GUI 收取 flow 的路径不是 PushConnector；GUI 与 EventDispatcher 共享控制面的事件语义，但分别维护自己的传输和生命周期。PushConnector 中的“轮询”仅指从面板拉取远程命令，不表示轮询内核事件日志。

## 配置

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
| `url` | string | — | 接收端 API 根 URL，设置后启用 connector |
| `node_id` | string | — | 本节点标识，`url` 设置后必填 |
| `api_key` | string | — | 接收端 API 密钥，或通过 `api_key_env` 从环境变量读取 |
| `heartbeat_interval_seconds` | u64 | `30` | 心跳间隔 |
| `pull_commands` | bool | `false` | 是否主动轮询命令 |
| `command_poll_interval_seconds` | u64 | `10` | 命令轮询间隔 |

`url` 为空时 connector 不启动。

## 心跳协议

### POST /api/v1/nodes/{node_id}/heartbeat

**请求：**
```json
{
  "node_id": "node-001",
  "build_id": "<build-id>",
  "uptime_seconds": 3600,
  "active_flows": 42,
  "bytes_up": 1024000,
  "bytes_down": 5120000
}
```

请求头：`Authorization: Bearer {api_key}`

**响应（无命令）：**
```json
{ "ok": true }
```

**响应（嵌入命令）：**
```json
{
  "ok": true,
  "commands": [
    {
      "method": "policies.select",
      "params": {
        "policy_tag": "proxy",
        "target_tag": "server-b"
      }
    }
  ]
}
```

命令在当前心跳周期内立即执行，无需等待下一次轮询。

### 支持的命令

| method | 说明 |
|--------|------|
| `policies.select` | 切换 selector，params: `policy_tag`, `target_tag` |

命令执行结果通过日志记录，不在心跳中回传。未来可扩展 `POST /api/v1/nodes/{node_id}/commands/{cmd_id}/result`。

## 命令轮询

当 `pull_commands` 为 `true` 时，节点定时拉取命令。

### GET /api/v1/nodes/{node_id}/commands

请求头：`Authorization: Bearer {api_key}`

**响应：**
```json
[
  {
    "id": "cmd-001",
    "method": "policies.select",
    "params": { "policy_tag": "proxy", "target_tag": "direct" }
  }
]
```

空数组表示无待执行命令。

轮询间隔由 `command_poll_interval_seconds` 控制（默认 10s）。

## Keepalive 与重连

- 每次心跳成功重置 `last_success` 时间戳
- 心跳失败后进入指数退避：1s → 2s → 4s → 8s → ... → 最大 64s
- 连续失败期间暂停命令轮询（避免无效请求）
- 断连超过 2 分钟记录 `warn` 日志
- 接收端应实现心跳超时检测（如 90s 无心跳视为节点离线）

## 面版侧参考实现

最小可用的面版端点（Python/Flask）：

```python
from flask import Flask, request, jsonify

app = Flask(__name__)
nodes = {}  # node_id -> last_seen

@app.post("/api/v1/nodes/<node_id>/heartbeat")
def heartbeat(node_id):
    body = request.get_json()
    nodes[node_id] = body
    resp = {"ok": True}

    # 嵌入待执行命令
    pending = get_pending_commands(node_id)
    if pending:
        resp["commands"] = pending

    return jsonify(resp)

@app.get("/api/v1/nodes/<node_id>/commands")
def commands(node_id):
    return jsonify(get_pending_commands(node_id))

def get_pending_commands(node_id):
    # 从数据库读取待执行命令
    return []
```

## 安全

- 所有请求携带 `Authorization: Bearer {api_key}`
- 建议接收端侧使用 HTTPS
- `api_key` 支持从环境变量读取（`api_key_env`），避免写入配置文件
