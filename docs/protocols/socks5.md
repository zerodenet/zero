# SOCKS5

SOCKS5 is a stable core protocol in Zero.

## Current Capability

| Capability | Status | Notes |
|------|------|------|
| TCP inbound | `supported` | SOCKS5 CONNECT |
| UDP inbound | `supported` | SOCKS5 UDP ASSOCIATE |
| TCP outbound | `supported` | Upstream SOCKS5 CONNECT |
| UDP outbound | `supported` | Upstream SOCKS5 UDP ASSOCIATE |
| Auth | `supported` | No-auth and username/password |

## Boundary

When SOCKS5 is used as a local entry, it is only the client-facing ingress. For example, `socks5 UDP ASSOCIATE -> vmess -> vmess` is tracked as a VMess relay-chain path, not as a cross-protocol chain.

