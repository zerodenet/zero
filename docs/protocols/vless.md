# VLESS

VLESS is currently a `partial` protocol capability. Baseline TCP and UDP-over-stream paths exist, while UDP MUX, some final-hop transports, and interoperability coverage remain incomplete.

## Current Capability

| Capability | Status | Notes |
|------|------|------|
| TCP inbound | `supported` | Baseline VLESS TCP ingress |
| TCP outbound | `supported` | Baseline VLESS TCP upstream |
| UDP inbound | `partial` | UDP-over-stream baseline |
| UDP outbound | `partial` | Single-hop and selected relay-chain final-hop paths |
| MUX | `partial` | TCP MUX exists; UDP MUX is not implemented |

## Remaining Gaps

- `mux_udp_is_not_implemented`
- `udp_relay_chain_final_transport_limited`
- `non_reality_tls_fingerprint_passthrough_is_incomplete`
- `relay_stream_tls_client_fingerprint_is_not_supported`

