# API Contract

This page describes the current control plane contract for external consumers.

## Naming

All external JSON field names, enum values, feature names, adapter names, sink
names, command methods, query variants, and error codes use `snake_case`.

Examples:

```json
{
  "error": { "code": "permission_denied" },
  "features": ["status_api", "config_snapshot", "runtime_snapshot"],
  "adapters": [{ "kind": "in_process", "enabled": true }]
}
```

Command methods use dotted namespaces because they name kernel capabilities:

```json
{
  "method": "policies.select",
  "params": {
    "policy_tag": "proxy",
    "target_tag": "direct"
  }
}
```

## Response Envelope

HTTP and IPC responses use `zero_api::ApiResponse`.

| Field | Meaning |
|------|------|
| `api_id` | Protocol identifier; current value is `"zero.api.v1"` |
| `id` | Request correlation ID, mainly used by IPC multiplexing |
| `ok` | Whether the request succeeded |
| `result` | Successful response payload |
| `error` | Structured error payload |

Consumers should branch on `ok` first, then parse either `result` or `error`.

## Event Envelope

Events use `zero_api::ApiEvent`.

| Field | Meaning |
|------|------|
| `schema_id` | Event schema identifier; current value is `"zero.event.v1"` |
| `event_id` | Stable event identity for de-duplication |
| `event_type` | Machine-readable event name |
| `sequence` | Monotonic sequence within the event source |
| `occurred_at_unix_ms` | Event timestamp |
| `source_id` | Optional node/source identifier |
| `principal_key` | Optional traffic attribution key |
| `labels` | Optional external labels |
| `payload` | Event-specific payload |

Consumers should route by `event_type` string. Unknown event types should be
ignored unless the consumer explicitly needs to fail closed.

## Capability Discovery

Use `GET /api/v1/capabilities` or the IPC `capabilities` query to discover the
current build and runtime surface.

The response reports:

- enabled adapters
- configured event sinks
- compiled or enabled features
- permissions granted to the current caller
- protocol and event schema identifiers

Capability discovery is descriptive. It does not grant extra authority and does
not expose panel-specific business concepts.

## Error Handling

Error codes are stable machine strings in `snake_case`.

| Code | Meaning |
|------|------|
| `not_found` | Requested resource does not exist |
| `invalid_argument` | Request shape or field value is invalid |
| `permission_denied` | Caller lacks the required permission |
| `feature_disabled` | Capability is not enabled in the current build/runtime |
| `conflict` | Current state rejects the operation |
| `unsupported` | Operation is not part of the current control surface |
| `internal` | Kernel-side error |

Do not parse `error.message` for control flow. It is human-readable context.

## Consumer Shape

External GUI and panel integrations should keep their own business state outside
the kernel:

- user accounts, plans, quotas, billing, tenants, and audit policy stay in the
  external system
- kernel attribution uses `principal_key`, `source_id`, and `labels`
- runtime decisions are made through query snapshots, events, and commands
- direct mutation of engine internals is not part of the control plane
