# HTTP API Endpoints

This document is an index of the current local HTTP control plane. The detailed
wire contract lives in [http-api.md](../control-plane-api/http-api.md).

## Base Path

All HTTP control endpoints are under `/api/v1/`.

HTTP and IPC responses share the `zero_api::ApiResponse` envelope:

```json
{
  "api_id": "zero.api.v1",
  "ok": true,
  "result": {}
}
```

Errors use `snake_case` machine codes:

| Code | HTTP | Meaning |
|------|------|------|
| `not_found` | 404 | Resource does not exist |
| `invalid_argument` | 400 | Request argument is invalid |
| `permission_denied` | 403 | Caller lacks permission |
| `feature_disabled` | 501 | Capability is not enabled |
| `conflict` | 409 | State conflict |
| `unsupported` | 501 | Operation is not exposed |
| `internal` | 500 | Internal error |

## Query Endpoints

| Endpoint | Meaning |
|------|------|
| `GET /api/v1/capabilities` | Build/runtime capabilities, adapters, sinks, permissions |
| `GET /api/v1/health` | Process health |
| `GET /api/v1/config` | Current configuration snapshot |
| `GET /api/v1/runtime` | Runtime snapshot with stats, flows, log state |
| `GET /api/v1/stats` | Immediate statistics snapshot |
| `GET /api/v1/flows` | Active flow list |
| `GET /api/v1/flows/{flow_id}` | Single active flow |
| `GET /api/v1/policies` | Policy/group state |
| `GET /api/v1/policies/{policy_tag}` | Single policy/group state |
| `GET /api/v1/sinks` | Event sink delivery status |
| `GET /api/v1/tun_status` | TUN runtime state |
| `GET /api/v1/events` | Event log snapshot |

`GET /api/v1/stats` and the stats section inside `GET /api/v1/runtime` are
computed from the current in-memory counters when the request is handled.

## Command Endpoint

All write/control operations use `POST /api/v1/commands`.

```json
{
  "method": "policies.select",
  "params": {
    "policy_tag": "proxy",
    "target_tag": "direct"
  }
}
```

Current command methods:

| Method | Meaning |
|------|------|
| `policies.select` | Select a member for a selector policy |
| `policies.probe` | Trigger a url_test probe |
| `flows.close` | Close an active flow |
| `config.validate` | Validate a config object |
| `config.apply` | Apply a config object to runtime state |
| `mode.set` | Set global routing mode |
| `tun.start` | Start TUN |
| `tun.stop` | Stop TUN |
| `diagnostics.probe_target` | Probe a target TCP endpoint |
| `diagnostics.dns_lookup` | Resolve a hostname |
| `diagnostics.trace_route` | Trace routing decision for a target |

## Event Stream

`GET /api/v1/events/stream` returns Server-Sent Events.

Query parameters:

| Parameter | Meaning |
|------|------|
| `types` | Comma-separated event type whitelist |
| `since` | Replay events after the given sequence |

The server also accepts `Last-Event-ID` for replay.

`stats.sampled` is emitted once per second while the runtime is active. It is a
coarse system event for GUI refresh and sink delivery, not a replacement for
on-demand query snapshots.

## Authentication

When an API key is configured, callers use:

```http
Authorization: Bearer <token>
```

or:

```http
X-Zero-Api-Key: <token>
```

Without configured HTTP auth, requests are treated as local administrative
control. Public listeners should configure an API key and firewall boundary.
