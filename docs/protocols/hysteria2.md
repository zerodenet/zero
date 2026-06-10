# Hysteria2

Hysteria2 is currently a `partial` protocol capability. Baseline QUIC stream and QUIC datagram paths exist.

## Current Capability

| Capability | Status | Notes |
|------|------|------|
| TCP inbound | `supported` | QUIC stream |
| TCP outbound | `supported` | QUIC stream |
| UDP inbound | `partial` | QUIC datagram |
| UDP outbound | `partial` | QUIC datagram |
| MUX | `not_applicable` | Multiplexing is provided by QUIC |

## Remaining Gaps

- External interoperability coverage is incomplete.
- `udp_relay_chain_quic_path_not_supported`

