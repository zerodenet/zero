# zero-protocol-vless

VLESS protocol implementation.

Current scope:

- Inbound: UUID user validation, TCP command, IPv4/domain/IPv6 target addresses.
- Outbound: establishes TCP tunnels to upstream VLESS nodes.
- Outbound Reality: VLESS-owned TLS-like security layer exposed as a stream upgrader for `zero-proxy` wiring.
- Inbound Reality: first-stage raw TCP server support for Zero-to-Zero VLESS Reality.
- Runtime wrapping is assembled in `zero-proxy`: TCP, TLS, WebSocket, and WSS.
- Observability attribution: inbound users may configure `credential_id` and `principal_key`.

Still missing: VLESS UDP, MUX, XTLS Vision, gRPC, and broad inbound Reality client interop validation.

Reality interop validation:

```powershell
cargo test -p zero-proxy --test vless relays_tcp_through_vless_reality_xray -- --ignored
```

The ignored interop test starts an Xray Reality server in Docker and verifies Zero's VLESS
Reality outbound path against it. Set `ZERO_XRAY_IMAGE` to override the default image
`ghcr.io/xtls/xray-core:latest`.
