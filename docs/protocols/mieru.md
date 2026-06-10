# Mieru

Mieru is currently a `partial` protocol capability. Baseline TCP stream and UDP associate paths exist.

## Current Capability

| Capability | Status | Notes |
|------|------|------|
| TCP inbound | `experimental` | Inbound TCP remains experimental in the descriptor |
| TCP outbound | `partial` | Encrypted stream upstream |
| UDP inbound | `partial` | UDP associate path |
| UDP outbound | `partial` | Single-hop and TCP relay-prefix final-hop paths |
| MUX | `not_applicable` | No separate MUX configuration |

## Remaining Gaps

- External interoperability coverage is incomplete.
- Inbound TCP remains experimental in the descriptor.

