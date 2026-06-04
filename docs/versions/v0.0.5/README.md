# v0.0.5

This version is still on the unstable `v0.0.x` line. It focuses on protocol correctness, real-node compatibility, relay accounting, and logging reliability.

## Delivered

- Fixed the generic TCP relay direction bug. The default relay now copies client-to-upstream and upstream-to-client instead of copying each side back to itself.
- Added traffic accounting in the default TCP relay path and in custom Shadowsocks/Hysteria2 relay paths.
- Fixed Trojan outbound compatibility with trojan-go/Xray-style TCP nodes by writing the full Trojan TCP request in one packet after TLS setup.
- Changed raw Trojan outbound TLS ALPN default to an empty list. Operators can still configure ALPN explicitly where needed.
- Fixed Shadowsocks AEAD TCP framing:
  - Uses SIP004-style `salt + encrypted length + encrypted payload` chunks.
  - Uses EVP_BytesToKey before HKDF-SHA1 for legacy AEAD ciphers.
  - Wraps outbound streams so post-handshake payload is encrypted/decrypted instead of relayed as raw TCP.
  - Keeps server-to-client Shadowsocks direction on its own response salt/key.
- Registered the Mieru protocol adapter and connected single-hop Mieru TCP outbound through the encrypted `MieruTcpStream` wrapper.
- Improved Mieru outbound stream parsing so incomplete TCP segments do not advance cipher nonce state.
- Fixed file logging by keeping non-blocking appender worker guards alive for the process lifetime.
- Adjusted file log rotation to rotate before writing the next oversized entry.
- Cleaned build warnings around protocol imports, IPC, and adapter registration.

## Explicit Non-Goals

- VMess is not considered Xray/Clash compatible yet. `cipher: auto` is not supported, and forcing an explicit AEAD cipher may still fail against standard Xray VMess AEAD nodes.
- Mieru as an intermediate `relay` chain hop is not supported yet. Mieru single-hop outbound is wired through the encrypted stream wrapper; inbound external-client interoperability remains experimental until covered by real-client validation.
- UDP support remains limited to the already implemented paths; Mieru UDP relay-chain support is not part of this version.

## Validation

- `cargo fmt --all`
- `cargo check --features full,status-api`
- `cargo test -p mieru --features crypto`
- `cargo test -p shadowsocks --features crypto`
- `cargo test -p trojan --features crypto`
- `cargo test -p zero-logging`
- Real-node verification passed for Trojan and Shadowsocks through local mixed HTTP CONNECT.

`cargo test --workspace` may still require a local `protoc` binary because `zero-grpc` uses `prost-build`.
