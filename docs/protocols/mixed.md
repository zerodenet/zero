# Mixed

`mixed` is a kernel inbound multiplexer, not an external proxy protocol.

## Current Capability

| Capability | Status | Notes |
|------|------|------|
| TCP inbound | `supported` | Auto-detects SOCKS5 CONNECT and HTTP CONNECT |
| UDP inbound | `supported` | Uses the SOCKS5 UDP ASSOCIATE path |
| outbound | `unsupported` | `mixed` is not an outbound protocol |

## Boundary

GUI clients can expose `mixed` as the default local entry. After detection, traffic enters the normal TCP or UDP pipe.

