# HTTP CONNECT

HTTP CONNECT is a stable TCP inbound protocol in Zero.

## Current Capability

| Capability | Status | Notes |
|------|------|------|
| TCP inbound | `supported` | HTTP CONNECT tunnel ingress |
| TCP outbound | `unsupported` | Not implemented as an upstream proxy protocol |
| UDP | `not_applicable` | HTTP CONNECT has no UDP ASSOCIATE model |

## Boundary

HTTP CONNECT does not provide encryption, UDP, or MUX semantics. It is a client-facing TCP tunnel ingress only.

