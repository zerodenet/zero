# Configuration Model Reference

All control plane configuration lives under the `api` key. This page documents `v0.0.4` API configuration fields.

For the full configuration model (inbounds, outbounds, route, runtime), see [config.md](../project/config.md).

## Complete Example

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

Local HTTP control interface.

| Field | Type | Default | Description |
|------|------|------|------|
| `enabled` | bool | `false` | Whether to start the HTTP control server |
| `listen` | object | -- | Listen address; required when `enabled=true` |
| `listen.address` | string | -- | Bind IP, `127.0.0.1` for local only, `0.0.0.0` for public |
| `listen.port` | u16 | -- | Listen port |
| `api_key` | string | -- | Bearer token; no auth if unset (local only recommended) |
| `api_key_env` | string | -- | Read api_key from env var; lower priority than `api_key` |

**CLI override**: `--status-listen 127.0.0.1:9090` takes priority over the config file. They cannot be used together.

### Rate Limiting

Built-in rate limiting, no configuration required:

| Category | Limit | Response |
|------|------|------|
| Query (GET) | 100 req/s | 429 Too Many Requests |
| Command (POST) | 10 req/s | 429 Too Many Requests |
| SSE concurrent | 5 connections | 429 Too Many Requests |

## `api.hooks`

Flow lifecycle hooks, executed in array order.

```json
{ "type": "ipc", "socket": "/run/billing/hook.sock", "timeout_ms": 100 }
```

| Field | Type | Default | Description |
|------|------|------|------|
| `type` | string | -- | Hook type, currently only `"ipc"` |
| `socket` | string | -- | IPC socket path |
| `timeout_ms` | u64 | `100` | Request timeout in milliseconds; fail-open on timeout |

**CLI override**: `--ipc-hook-socket /run/billing/hook.sock` takes priority over the config file.

Hook protocol details: see [hooks.md](./hooks.md).

## `push`

Node proactively reports to an external management endpoint. The receiver can be a panel, monitoring system, or any HTTP service.

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

| Field | Type | Default | Description |
|------|------|------|------|
| `url` | string | -- | Receiver URL; push is enabled when set |
| `node_id` | string | -- | This node's identifier |
| `api_key` | string | -- | Authentication key |
| `api_key_env` | string | -- | Read api_key from env var |
| `heartbeat_interval_seconds` | u64 | `30` | Heartbeat interval |
| `pull_commands` | bool | `false` | Whether to poll for remote commands |
| `command_poll_interval_seconds` | u64 | `10` | Command poll interval |

Protocol details: see [push-connector.md](./push-connector.md).

## `api.event_sinks`

Event delivery target array.

### JSON Lines File

```json
{
  "type": "jsonl",
  "tag": "audit",
  "path": "/var/log/zero/events.jsonl",
  "events": ["flow.completed"],
  "source_id": "node-001"
}
```

| Field | Type | Default | Description |
|------|------|------|------|
| `type` | string | -- | `"jsonl"` or alias `"file"` |
| `tag` | string | -- | Unique identifier |
| `path` | string | -- | File path; relative paths resolve against the config directory |
| `events` | string[] | `[]` | Event type whitelist; empty = accept all |
| `source_id` | string | -- | Override event source_id |

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

| Field | Type | Default | Description |
|------|------|------|------|
| `type` | string | -- | `"webhook"` |
| `tag` | string | -- | Unique identifier |
| `url` | string | -- | Receiver endpoint |
| `events` | string[] | `[]` | Event type whitelist |
| `api_key` | string | -- | Request header `Authorization: Bearer {key}` |
| `api_key_env` | string | -- | Read from env var |
| `allow_insecure` | bool | `false` | Skip TLS certificate verification (testing only) |

Failed deliveries automatically retry (exponential backoff 2s->4s->8s->...->64s, max 6 attempts).

## `api.dead_letter_path`

Dead letter queue file path. Events exceeding max retry count are not discarded but written to this file for persistence.

| Field | Type | Description |
|------|------|------|
| `dead_letter_path` | string | Dead letter JSON Lines file path; events are eventually discarded if unset |

Dead letter file format: one JSON object per line, containing `dead_lettered_at_unix_ms` and `original_event`.

### Delivery Status Query

```bash
zero status  # includes sink delivery statistics
```

## v0.0.4 Config Additions (non-API)

The following new configuration fields were added in v0.0.4. They live outside the `api` section but are visible via `GET /api/v1/config` and relevant to control plane consumers.

| Field | Location | Description |
|------|------|------|
| `idle_timeout_secs` | `inbounds[*]` | TCP relay idle timeout in seconds (default 300) |
| `url_rewrite` | `route.url_rewrite[]` | Domain rewrite rules (`from` / `from_regex` -> `to`) before routing |
| `domain-regex` | `route.rules[*].condition` | New condition type matching domains against regex patterns |
| `up_bps` / `down_bps` | `inbounds[*].protocol` (Hysteria2, Shadowsocks, Trojan) | Per-inbound GCRA rate limits |

For full details, see [config.md](../project/config.md).
