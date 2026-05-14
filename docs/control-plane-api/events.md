# 事件目录

所有事件以归一化信封格式输出，通过 SSE、IPC 流或 Sink 投递消费。

## 事件信封

```json
{
  "schema_version": "zero.event.v1",
  "event_id": "flow.completed:42:1713500000000",
  "event_type": "flow.completed",
  "occurred_at_unix_ms": 1713500000000,
  "source_id": null,
  "sequence": 1024,
  "principal_key": "user-001",
  "labels": {},
  "payload": { }
}
```

| 字段 | 说明 |
|------|------|
| `schema_version` | 事件格式版本 |
| `event_id` | 唯一标识，格式 `{type}:{flow_id}:{timestamp}` |
| `event_type` | 事件类型，用于过滤 |
| `occurred_at_unix_ms` | 事件时间戳（毫秒） |
| `source_id` | 节点标识（sink 投递时注入） |
| `sequence` | 单调递增序号，用于 SSE 断点续传 |
| `principal_key` | 关联的用户标识 |
| `payload` | 事件负载（类型相关） |

## 事件类型总览

| 事件 | 触发时机 | 频率 |
|------|---------|------|
| `engine.started` | 进程启动 | 启动时 1 次 |
| `engine.stopped` | 进程停止 | 停止时 1 次 |
| `engine.warning` | 非致命异常 | 按需 |
| `flow.started` | flow 建立 | 每个新连接 |
| `flow.updated` | 活动 flow 流量快照 | 每 10s / flow |
| `flow.completed` | flow 结束/被关闭/被阻断 | 每个结束的 flow |
| `policy.selected` | selector 切换 | 按需 |
| `stats.sampled` | 统计采样 | 每 30s |

---

## 负载规范

### engine.started

```json
{
  "version": "0.0.2",
  "started_at_unix_ms": 1713500000000
}
```

### engine.warning

```json
{
  "code": "ipc_hook_unreachable",
  "message": "ipc hook unreachable (Connection refused); allowing flow (fail-open)"
}
```

| code | 说明 |
|------|------|
| `ipc_hook_unreachable` | IPC hook 进程不可达，fail-open 放行 |

### flow.started

```json
{
  "flow_id": "42",
  "network": "tcp",
  "inbound": { "tag": "socks5", "protocol": "socks5" },
  "auth": { "scheme": "noauth", "credential_id": null, "principal_key": null, "attributes": {} },
  "target": { "host": "example.com", "port": 443 },
  "route": { "mode": "rule", "target": null },
  "policy": null,
  "outbound": { "tag": "server-a", "protocol": "vless" },
  "traffic": { "bytes_up": 0, "bytes_down": 0, "packets_up": null, "packets_down": null },
  "timing": { "started_at_unix_ms": 1713500000000, "ended_at_unix_ms": null, "duration_ms": null },
  "outcome": "direct-relayed"
}
```

### flow.updated

每 10 秒对所有活动 flow 发射。

```json
{
  "flow_id": "42",
  "network": "tcp",
  "inbound_tag": "socks5",
  "outbound_tag": "server-a",
  "bytes_up": 1024000,
  "bytes_down": 5120000,
  "inbound_rx_bytes": 1024000,
  "inbound_tx_bytes": 5120000,
  "outbound_rx_bytes": 5120000,
  "outbound_tx_bytes": 1024000,
  "throughput_up_bps": 8192,
  "throughput_down_bps": 32768,
  "snapshot_at_unix_ms": 1713500010000
}
```

### flow.completed

与 `flow.started` 同结构，`timing` 包含 `ended_at_unix_ms` 和 `duration_ms`，`traffic` 包含最终流量统计，`outcome` 为最终结果。

outcome 值：

| 值 | 说明 |
|-----|------|
| `direct-relayed` | 直连成功 |
| `chained-relayed` | 链式转发成功 |
| `blocked` | 被路由规则拒绝 |
| `failed` | 连接失败 |
| `cancelled` | 被 `flows.close` 关闭 |

### policy.selected

```json
{
  "policy_tag": "proxy",
  "policy_kind": "selector",
  "selected": "server-a",
  "previous": "direct"
}
```

### stats.sampled

```json
{
  "active_sessions": 42,
  "total_started": 1234,
  "completed_sessions": 1192,
  "failed_sessions": 3,
  "blocked_sessions": 10,
  "direct_sessions": 800,
  "chained_sessions": 392,
  "bytes_up": 1024000000,
  "bytes_down": 5120000000,
  "udp_upstream": {
    "active_associations": 5,
    "created_associations": 50,
    "reused_associations": 45,
    "closed_associations": 48,
    "idle_timeouts": 1,
    "dropped_associations": 0,
    "failed_association_attempts": 0,
    "send_failures": 0,
    "recv_failures": 0,
    "packets_sent": 10000,
    "packets_received": 9500
  }
}
```

## 消费方式

| 方式 | 过滤 | 回放 |
|------|------|------|
| SSE (`GET /api/v1/events/stream?types=...`) | event_type 白名单 | `?since=<seq>` / `Last-Event-ID` |
| IPC (`{"type":"subscribe","events":[...]}`) | event_type 白名单 | 不支持（实时流） |
| CLI (`zero events`) | 无 | 不支持 |
| Sink (`event_sinks[].events`) | event_type 白名单 | 不支持（持久投递） |
