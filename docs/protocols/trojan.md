# Trojan

Trojan is currently a `partial` protocol capability. Baseline TCP and UDP-over-stream paths exist.

## Current Capability

| Capability | Status | Notes |
|------|------|------|
| TCP inbound | `supported` | TLS ingress and Trojan TCP request |
| TCP outbound | `supported` | Trojan TCP upstream |
| UDP inbound | `partial` | Trojan UDP-over-stream |
| UDP outbound | `partial` | Single-hop and TCP relay-prefix final-hop paths |
| MUX | `unsupported` | Trojan MUX is not provided |

## Remaining Gaps

- External interoperability coverage is incomplete.
- `relay_stream_tls_client_fingerprint_is_not_supported`
- MUX is not implemented.

