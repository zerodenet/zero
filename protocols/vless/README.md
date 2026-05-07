# zero-protocol-vless

VLESS protocol implementation.

Current scope:

- Inbound: UUID user validation, TCP command, IPv4/domain/IPv6 target addresses.
- Outbound: establishes TCP tunnels to upstream VLESS nodes.
- Outbound Reality: VLESS-owned TLS-like security layer exposed as a stream upgrader for `zero-proxy` wiring.
- Runtime wrapping is assembled in `zero-proxy`: TCP, TLS, WebSocket, and WSS.
- Observability attribution: inbound users may configure `credential_id` and `principal_key`.

Still missing: VLESS UDP, MUX, XTLS Vision, gRPC, and inbound Reality.
