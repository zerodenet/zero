# HTTP JSON API

## 基础信息

- 稳定前缀：`/api/v1/`
- 兼容端点（已废弃，v0.2.0 移除）：`/status`、`/runtime`、`/config`、`POST /selectors/{group}/{target}`
- 认证：`Authorization: Bearer <token>` 或 `X-Zero-Api-Key: <token>`（未配置时无认证模式，默认所有权限）
- CORS：所有端点返回 `Access-Control-Allow-Origin: *`
- 限流：Query 100/s，Command 10/s，SSE 5 并发

## 通用响应格式

成功：
```json
{
  "api_version": "zero.api.v1",
  "request_id": "req-abc123",
  "ok": true,
  "result": { },
  "error": null
}
```

失败：
```json
{
  "api_version": "zero.api.v1",
  "request_id": null,
  "ok": false,
  "result": null,
  "error": {
    "code": "not-found",
    "message": "Policy not found",
    "field_path": "policy_tag",
    "details": null
  }
}
```

错误码（kebab-case，与 JSON serde 格式一致）：

| code | HTTP | 说明 |
|------|------|------|
| `not-found` | 404 | 资源不存在 |
| `invalid-argument` | 400 | 参数无效 |
| `permission-denied` | 403 | 权限不足 |
| `feature-disabled` | 501 | 功能未编译 |
| `conflict` | 409 | 状态冲突 |
| `unsupported` | 501 | 不支持的操作 |
| `internal` | 500 | 内部错误 |

---

## Query 端点

### GET /api/v1/capabilities

API 版本和能力列表。

```json
{
  "api_version": "zero.api.v1",
  "schema_version": "zero.event.v1",
  "adapters": [{ "kind": "in-process", "enabled": true }],
  "sinks": [{ "kind": "none", "enabled": false }],
  "features": ["query", "config-snapshot", "runtime-snapshot", "flow-snapshot", "policy-snapshot"],
  "permissions": ["read"]
}
```

### GET /api/v1/health

进程健康状态。

```json
{
  "engine_version": "0.0.2",
  "started_at_unix_ms": 1713500000000,
  "healthy": true
}
```

### GET /api/v1/config

当前配置快照。

参数：`?format=full|minimal`（默认 full）

### GET /api/v1/runtime

完整运行时状态：统计、策略、活动流、最近完成的流。

### GET /api/v1/stats

轻量统计摘要。`active_sessions`, `total_started`, `completed_sessions`, `failed_sessions`, `bytes_up`, `bytes_down` 等。

### GET /api/v1/flows

活动流列表，支持过滤。

| 参数 | 默认 | 说明 |
|------|------|------|
| `limit` | 100 | 最大返回数 |
| `inbound_tag` | — | 按入站过滤 |
| `principal_key` | — | 按用户过滤 |

### GET /api/v1/flows/{flow_id}

单流详情。不存在返回 404。

### GET /api/v1/policies

所有 policy 状态（selector / fallback / urltest），包含当前选择和健康探测结果。

### GET /api/v1/policies/{policy_tag}

单个 policy 详情。不存在返回 404。

---

## Command 端点

### POST /api/v1/commands

统一命令入口。

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

支持的方法：

#### policies.select

切换 selector 的当前出站。

Params：`policy_tag` (string), `target_tag` (string)

Response：
```json
{ "policy_tag": "proxy", "selected": "direct" }
```

错误：
- `not_found` — policy 不存在
- `invalid_argument` — target 不在 members 中

#### policies.probe

触发 urltest 立即执行一轮探测。

Params：`policy_tag` (string)

Response：
```json
{ "policy_tag": "auto", "probe_triggered": true }
```

错误：`not_found` — policy 不存在或不是 urltest 类型

#### flows.close

主动关闭活动 flow。

Params：`flow_id` (string)

Response：
```json
{ "flow_id": "flow-123", "closed": true }
```

错误：`not_found` — flow 不存在或已结束

#### config.validate

校验配置有效性，不改变运行时状态。

Params：`config` (object, 完整配置)

Response：
```json
{ "valid": true }
```

错误：`invalid-argument` — 配置无效（cause 字段包含详情）

#### diagnostics.probe_target

对指定出站做一次 TCP 可达性探测。

Params：`target_tag` (string)

Response：
```json
{
  "target_tag": "server-a",
  "server": "1.2.3.4",
  "port": 443,
  "reachable": true,
  "latency_ms": 12
}
```

错误：`not-found` — target 不存在

权限：`admin`

#### diagnostics.dns_lookup

解析域名。

Params：`hostname` (string)

Response：
```json
{
  "hostname": "example.com",
  "resolved_addresses": ["93.184.216.34", "2606:2800:220:1:248:1893:25c8:1946"],
  "count": 2
}
```

权限：`admin`

#### diagnostics.trace_route

查看路由规则对指定目标的匹配结果。

Params：`target` (string), `port` (number), `protocol` (string, 可选，默认 `"tcp"`)

Response：
```json
{
  "target": "1.1.1.1",
  "port": 443,
  "protocol": "tcp",
  "effective_mode": "rule",
  "route_action": { "route": "proxy" }
}
```

权限：`admin`

---

## 鉴权

内核采用简单的 Bearer Token 鉴权，不做细粒度权限隔离：

- **未配置 token**（本地监听 `127.0.0.1`）：所有端点无鉴权，全部可用
- **已配置 token**：所有请求必须携带 `Authorization: Bearer <token>` 或 `X-Zero-Api-Key: <token>`，认证通过后全部端点可用

这是一个内核，不是多租户 SaaS。权限隔离应在上层面板/网关实现，不应侵入内核。

---

## 事件流端点

### GET /api/v1/events/stream

Server-Sent Events (SSE) 实时事件流。

| 参数/头 | 说明 |
|---------|------|
| `?types=flow.completed,policy.selected` | 事件类型过滤 |
| `?since=<sequence>` | 断点续传，从指定 sequence 之后开始 |
| `Last-Event-ID: <sequence>` | 同上，HTTP 头形式 |

事件格式：
```
id: 42
event: flow.completed
data: {"schema_version":"zero.event.v1","event_id":"...","event_type":"flow.completed",...}
```

连接断开后可使用 `Last-Event-ID` 续传，服务端先发送追赶事件再切回实时流。详见 [events.md](./events.md)。

---

## 兼容端点（过渡）

| 旧路径 | 映射到 |
|--------|--------|
| `GET /status` | `GET /api/v1/runtime` |
| `GET /config` | `GET /api/v1/config` |
| `GET /runtime` | `GET /api/v1/runtime` |
| `GET /events` | `GET /api/v1/events` (快照) |
| `POST /commands` | `POST /api/v1/commands` |
| `POST /selectors/{group}/{target}` | `policies.select` 命令 |
