# API 端点规范

本文档定义 Zero 控制面的 HTTP API 端点、请求格式和响应格式。

## 基础信息

### API 版本

- 稳定 API 前缀：`/api/v1/`
- 实验性 API 前缀：`/api/experimental/`

### 通用响应格式

所有 API 响应使用统一的信封格式：

```json
{
  "api_version": "1.0.0",
  "request_id": "req-abc123",
  "ok": true,
  "result": {},
  "error": null
}
```

错误响应格式：

```json
{
  "api_version": "1.0.0",
  "request_id": "req-abc123",
  "ok": false,
  "result": null,
  "error": {
    "code": "not_found",
    "message": "Policy not found",
    "field": "policy_tag",
    "details": {}
  }
}
```

### 错误码

| 错误码 | HTTP 状态码 | 说明 |
| --- | --- | --- |
| `invalid_argument` | 400 | 请求参数无效 |
| `permission_denied` | 403 | 权限不足 |
| `not_found` | 404 | 资源不存在 |
| `conflict` | 409 | 状态冲突 |
| `feature_disabled` | 501 | 功能未启用 |
| `unsupported` | 501 | 不支持的操作 |
| `internal` | 500 | 内部错误 |

---

## Query 端点

### GET /api/v1/capabilities

返回 API 能力和版本信息。

**响应**：

```json
{
  "api_version": "1.0.0",
  "engine_version": "0.0.2",
  "features": ["status-api", "event-sink", "vless", "urltest"],
  "adapters": ["http", "in-process"],
  "permissions": ["read", "control"]
}
```

**权限**：所有人可访问

---

### GET /api/v1/health

返回进程健康状态。

**响应**：

```json
{
  "status": "healthy",
  "started_at_unix_ms": 1713500000000,
  "uptime_ms": 3600000,
  "version": "0.0.2"
}
```

**权限**：所有人可访问

---

### GET /api/v1/config

返回当前有效配置视图（敏感字段脱敏）。

**查询参数**：

- `format`: `full` | `minimal` (默认: `full`)

**响应**：

```json
{
  "inbounds": [
    {
      "tag": "socks5",
      "type": "socks5",
      "listen": "127.0.0.1:1080"
    }
  ],
  "outbounds": [
    {
      "tag": "direct",
      "type": "direct"
    },
    {
      "tag": "server-a",
      "type": "vless",
      "server": "***",
      "port": 443
    }
  ],
  "outbound_groups": [
    {
      "tag": "proxy",
      "type": "selector",
      "members": ["server-a", "direct"]
    }
  ],
  "routes": []
}
```

**权限**：`read`

---

### GET /api/v1/runtime

返回完整运行时状态快照。

**响应**：

```json
{
  "stats": {
    "total_flows": 1234,
    "active_flows": 42,
    "bytes_up": 1024000,
    "bytes_down": 5120000
  },
  "policies": [
    {
      "tag": "proxy",
      "type": "selector",
      "current": "server-a",
      "members": ["server-a", "direct"],
      "health": {}
    }
  ],
  "active_flows": [
    {
      "id": "flow-123",
      "type": "tcp",
      "inbound_tag": "socks5",
      "target_tag": "server-a",
      "remote_addr": "1.2.3.4:443",
      "started_at_unix_ms": 1713500000000,
      "bytes_up": 1024,
      "bytes_down": 4096
    }
  ],
  "recent_flows": []
}
```

**权限**：`read`

---

### GET /api/v1/stats

返回轻量级统计摘要。

**响应**：

```json
{
  "total_flows": 1234,
  "active_flows": 42,
  "bytes_up": 1024000,
  "bytes_down": 5120000,
  "flows_per_second": 2.5,
  "bps_up": 8192,
  "bps_down": 32768
}
```

**权限**：`read`

---

### GET /api/v1/flows

返回 flow 列表，支持分页和过滤。

**查询参数**：

- `status`: `active` | `completed` | `all` (默认: `all`)
- `type`: `tcp` | `udp` | `all` (默认: `all`)
- `inbound_tag`: 按入站过滤
- `target_tag`: 按出站过滤
- `limit`: 最大返回数量 (默认: 100, 最大: 1000)
- `offset`: 分页偏移

**响应**：

```json
{
  "flows": [
    {
      "id": "flow-123",
      "type": "tcp",
      "inbound_tag": "socks5",
      "target_tag": "direct",
      "remote_addr": "1.2.3.4:80",
      "started_at_unix_ms": 1713500000000,
      "ended_at_unix_ms": null,
      "bytes_up": 1024,
      "bytes_down": 4096,
      "outcome": "in_progress"
    }
  ],
  "total": 42,
  "limit": 100,
  "offset": 0
}
```

**权限**：`read`

---

### GET /api/v1/flows/{flow_id}

查询单个 flow 详情。

**路径参数**：

- `flow_id`: flow 唯一标识

**响应**：

```json
{
  "id": "flow-123",
  "type": "tcp",
  "inbound_tag": "socks5",
  "target_tag": "direct",
  "remote_addr": "1.2.3.4:80",
  "started_at_unix_ms": 1713500000000,
  "ended_at_unix_ms": null,
  "bytes_up": 1024,
  "bytes_down": 4096,
  "outcome": "in_progress"
}
```

**错误**：

- `not_found`: flow 不存在

**权限**：`read`

---

### GET /api/v1/policies

返回所有 policy 状态。

**响应**：

```json
{
  "policies": [
    {
      "tag": "proxy",
      "type": "selector",
      "current": "server-a",
      "members": ["server-a", "server-b", "direct"],
      "health": {}
    },
    {
      "tag": "auto",
      "type": "urltest",
      "current": "server-b",
      "members": ["server-a", "server-b"],
      "interval_ms": 30000,
      "health": {
        "server-a": {
          "rtt_ms": 120,
          "status": "alive",
          "last_probe_at_unix_ms": 1713500000000
        },
        "server-b": {
          "rtt_ms": 85,
          "status": "alive",
          "last_probe_at_unix_ms": 1713500000000
        }
      }
    }
  ]
}
```

**权限**：`read`

---

### GET /api/v1/policies/{policy_tag}

查询单个 policy 详情。

**路径参数**：

- `policy_tag`: policy 标识

**响应**：

```json
{
  "tag": "auto",
  "type": "urltest",
  "current": "server-b",
  "members": ["server-a", "server-b"],
  "interval_ms": 30000,
  "health": {
    "server-a": {
      "rtt_ms": 120,
      "status": "alive",
      "last_probe_at_unix_ms": 1713500000000
    }
  }
}
```

**错误**：

- `not_found`: policy 不存在

**权限**：`read`

---

## Command 端点

### POST /api/v1/commands

执行控制命令。统一入口，通过 `method` 字段区分命令类型。

**请求**：

```json
{
  "id": "req-123",
  "method": "policies.select",
  "params": {
    "policy_tag": "proxy",
    "target_tag": "direct"
  }
}
```

**支持的方法**：

#### 1. policies.select

切换 selector 的当前成员。

**Params**：

```json
{
  "policy_tag": "proxy",
  "target_tag": "direct"
}
```

**响应 Result**：

```json
{
  "policy_tag": "proxy",
  "previous": "server-a",
  "current": "direct"
}
```

**错误**：

- `not_found`: policy 不存在或不是 selector 类型
- `invalid_argument`: target_tag 不在 members 中

**权限**：`control`

---

#### 2. policies.probe

触发 urltest 立即执行一轮探测。

**Params**：

```json
{
  "policy_tag": "auto"
}
```

**响应 Result**：

```json
{
  "policy_tag": "auto",
  "probe_triggered": true
}
```

**错误**：

- `not_found`: policy 不存在或不是 urltest 类型

**权限**：`control`

---

#### 3. flows.close

主动关闭一个活动 flow。

**Params**：

```json
{
  "flow_id": "flow-123"
}
```

**响应 Result**：

```json
{
  "flow_id": "flow-123",
  "closed": true
}
```

**错误**：

- `not_found`: flow 不存在或已结束

**权限**：`control`

---

#### 4. config.validate

校验一份配置是否有效，不改变运行时状态。

**Params**：

```json
{
  "config": {
    "inbounds": [],
    "outbounds": [],
    "routes": []
  }
}
```

**响应 Result**：

```json
{
  "valid": true,
  "errors": [],
  "warnings": []
}
```

**权限**：`config`

---

**通用响应**：

```json
{
  "id": "req-123",
  "ok": true,
  "result": {},
  "error": null
}
```

**权限**：依具体命令而定

---

## 事件流端点

### GET /api/v1/events/stream

Server-Sent Events (SSE) 实时事件流。

**查询参数**：

- `types`: 逗号分隔的事件类型过滤，如 `flow.completed,policy.selected`
- `since`: 起始事件 ID，用于断点续传

**请求头**：

- `Accept: text/event-stream`
- `Last-Event-ID`: 可选，用于断点续传

**事件格式**：

```
id: event-456
event: flow.completed
data: {"schema_version":"1.0","event_id":"event-456","event_type":"flow.completed","occurred_at_unix_ms":1713500000000,"payload":{}}
```

**支持的事件类型**：

| 事件类型 | 说明 |
| --- | --- |
| `flow.started` | flow 开始 |
| `flow.completed` | flow 完成（成功/失败） |
| `policy.selected` | policy 当前选择变化 |
| `policy.probe.completed` | urltest 完成一轮探测 |
| `stats.sampled` | 周期性统计采样 |
| `config.changed` | 配置变更 |
| `engine.started` | 引擎启动 |
| `engine.stopped` | 引擎停止 |
| `engine.warning` | 告警事件 |

**权限**：`read`

---

## 兼容性端点（过渡）

### GET /status

返回简化状态，兼容现有调用方。

**响应**：同 `/api/v1/runtime` 的简化版。

### GET /runtime

同 `/api/v1/runtime`。

### GET /config

同 `/api/v1/config`。

### POST /selectors/{group}/{target}

切换 selector，兼容现有调用方。

> **注意**：这些兼容性端点将在 v0.1.0 后标记为 deprecated，v0.2.0 移除。新代码应使用 `/api/v1/` 下的标准端点。

---

## 认证

### Bearer Token 认证

在配置中设置 `api.secret`，请求时携带：

```
Authorization: Bearer <secret>
```

### 无认证模式

本地监听（`127.0.0.1`）且未配置 secret 时，默认所有请求拥有全部权限。

> **警告**：公网监听必须配置认证，否则控制面完全暴露。

---

## CORS

默认允许来自 `http://localhost:*` 和 `http://127.0.0.1:*` 的跨域请求，便于本地面板开发。

生产环境可通过 `api.cors_allowed_origins` 配置额外允许的源。

---

## 速率限制

本地监听默认无速率限制。

公网监听建议配置速率限制（待实现）：

- Query: 100 req/s
- Command: 10 req/s
- Event Stream: 最多 5 个并发连接
