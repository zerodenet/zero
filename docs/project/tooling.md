# 工程规则

This document describes the current workspace layout, build entry points, and
feature policy. It records current facts, not release history.

## Naming

- Package names use `zero-*`.
- External field names, protocol names, status values, error codes, event names,
  and capability codes use `snake_case`.
- Directory names stay short, for example `crates/engine`, `crates/proxy`, and
  `protocols/socks5`.
- The root binary entry point is `src/main.rs`.

## Workspace Commands

Use workspace-wide commands by default:

```powershell
cargo fmt --all
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo build --release
```

Run the proxy:

```powershell
cargo run -- run <config>
cargo run -- status --json <config>
```

Run one test:

```powershell
cargo test <test_name>
```

Run the full test suite when changing protocol behavior, config parsing,
routing, runtime wiring, or logging.

## Root Features

The root `zero` package is the external build entry point. It forwards protocol
and control-plane features to internal crates.

| Feature | Description |
| --- | --- |
| `default` | Same as `full,status_api` |
| `full` | Enables all protocol capabilities and `dns` |
| `dns` | DNS subsystem |
| `socks5` | SOCKS5 inbound/outbound, including TCP CONNECT and UDP ASSOCIATE |
| `http_connect` | HTTP CONNECT inbound |
| `mixed` | Same-port inbound auto-detection for SOCKS5 TCP/UDP and HTTP CONNECT TCP; depends on `socks5` and `http_connect` |
| `vless` | VLESS inbound/outbound and related transports |
| `hysteria2` | Hysteria2 inbound/outbound |
| `shadowsocks` | Shadowsocks inbound/outbound |
| `trojan` | Trojan inbound/outbound |
| `vmess` | VMess inbound/outbound |
| `mieru` | Mieru inbound/outbound |
| `status_api` | Local HTTP control plane |
| `event_dispatcher` | Event dispatch infrastructure and sink delivery status |
| `sink_jsonl` | JSON Lines event sink; depends on `event_dispatcher` |
| `panel_connector` | Panel/remote connector; depends on `status_api` and `event_dispatcher` |
| `grpc_api` | gRPC control-plane adapter |

`zero-proxy` also has internal transport-oriented feature wiring. For example,
VLESS enables TLS, WebSocket, gRPC, H2, HTTPUpgrade, and XHTTP (formerly
SplitHTTP, config field `split_http`) related transport capabilities. The
standalone VLESS QUIC transport is deprecated by XTLS (replaced by XHTTP
`stream-one` over H3); the `quic` config field is retained for backward
compatibility. External build users should prefer root features rather
than depending on internal crate feature shapes.

If a config references an uncompiled protocol, the kernel fails early with a
clear error.

## Code Boundaries

- `zero-traits` and `zero-core` do not bind to Tokio.
- Protocol implementations live in `protocols/*`.
- `zero-config` owns config ADTs and validation.
- `zero-router` owns rule matching.
- `zero-engine` owns decisions, plan/state, groups, sessions, stats, and events.
- `direct` and `block` target semantics stay in `zero-engine`; socket-level
  execution stays in `zero-proxy`.
- Listener lifecycle, transports, Tokio wiring, and protocol runtime adapters
  stay in `zero-proxy`.
- The root `zero` binary does not implement protocol details.

## Documentation Boundaries

- Config shape changes require matching config docs and examples.
- Protocol scope changes require matching project docs, capability matrix, and
  examples.
- Runtime layering changes require matching `docs/project/` updates.
- Documentation describes current facts only. It must not use version-history
  wording such as "since" or "as of".
