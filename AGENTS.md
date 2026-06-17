# Repository Guidelines

## Structure

Root `src/main.rs` builds the `zero` binary. Reusable crates live under `crates/`. External protocol implementations live under `protocols/`.

### Crates (dependency bottom-up)

- `zero-traits` — `#![no_std]` runtime-neutral abstractions: protocol traits (`TcpTunnelProtocol`, `UdpRelayProtocol`, `UdpPacketPath`, `DatagramCodec`, etc.), `TransportKind`, `InboundTransport`, address types, async socket/listener traits. No runtime dependency.
- `zero-core` — domain primitives: `Session`, `Address`, `Error`. Depends on `zero-traits`.
- `zero-api` — interface contract types (snapshots, commands, events). Independent.
- `zero-config` — config ADTs (`RuntimeConfig`, `InboundConfig`, `InboundProtocolConfig`, `OutboundProtocolConfig`, `RouteConfig`, etc.) and validation. Depends on `zero-api`, `zero-router`.
- `zero-router` — rule set matching and route decisions. Depends on `zero-core`.
- `zero-engine` — the control kernel: engine plan, target resolution, session registry, stats, event log, hooks, groups (urltest/selector), outbound health. Depends on `zero-api`, `zero-config`, `zero-core`, `zero-router`.
- `zero-logging` — non-blocking structured logging with event bridge. Depends on `zero-config`.
- `zero-dns` — configurable DNS resolver, caching, fake-IP, routing. Depends on `zero-config`, `zero-platform-tokio`, `zero-traits`.
- `zero-platform-tokio` — tokio runtime implementation of the traits abstractions (`TokioSocket`, `TokioListener`, `TokioDatagramSocket`, `TcpRelayStream`, `ClientStream`, `TransportConnector`). Depends on `zero-traits`.
- `zero-transport` — concrete transport implementations: QUIC, TLS, WebSocket, gRPC, H2, HTTP-Upgrade, split-HTTP, REALITY. Feature-gated per transport. Depends on `zero-config`, `zero-core`, `zero-engine`, `zero-platform-tokio`, `zero-traits`.
- `zero-proxy` — the orchestration layer: `Proxy`, `ProxyHandle`, `RunningProxy`, listener lifecycle, reload/reconcile, the `ProtocolRegistry` + `ProtocolAdapter` dispatch, `serve_inbound` unified TCP pipeline, TCP/UDP dispatch, upstream connect helpers. Depends on config/core/dns/engine/platform/traits/transport/tun/stack.
- `zero-connector` — event dispatcher and panel push connector. Optional (`event_dispatcher`, `sink_jsonl`, `panel_connector` features).
- `zero-grpc` — gRPC control plane adapter. Optional (`grpc_api` feature).
- `zero-ffi` — C-compatible FFI bindings.
- `zero-tun` — platform-agnostic TUN device abstraction.
- `zero-stack` — user-space network stack (TCP termination + UDP forwarding from raw IP packets).
- `zero-ztls` — generic TLS 1.3 client with custom ClientHello (extracted from REALITY).

### Protocols

External protocol implementations under `protocols/`: `socks5`, `http-connect`, `vless`, `hysteria2`, `shadowsocks`, `trojan`, `vmess`, `mieru`. Each is a standalone crate that depends only on `zero-core` and `zero-traits` (plus its own crypto). The proxy wires them via `ProtocolAdapter` registration.

### Key paths

- `docs/project/` — long-term project notes and architecture rules.
- `docs/protocols/` — per-protocol documentation.
- `examples/` — example configs, versioned.
- `tests/` — binary-level integration tests.
- `crates/proxy/tests/` — proxy-level integration tests (protocol interop, session observability).

## Features & Build

Default build is `--features full,status_api`. Optional features:
- `socks5`, `http_connect`, `mixed`, `vless`
- `hysteria2`, `shadowsocks`, `trojan`, `vmess`, `mieru`, `dns`
- `status_api` enables runtime control endpoint and selector switching
- `event_dispatcher`, `sink_jsonl`, `panel_connector` enable event delivery and panel connector support
- `grpc_api` enables the gRPC control plane adapter

If a config references an uncompiled protocol, it fails early with a clear error.

## Commands

Always use workspace-wide commands by default:
- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test --workspace` (full test suite)
- `cargo clippy --workspace --all-targets`
- `cargo build --release`
- `cargo run -- run <config>` - run proxy with given config
- `cargo run -- status [--json] <config>` - show current status

Run a single test: `cargo test <test_name>`

If you change protocol behavior, config parsing, routing, or runtime wiring, run the full test suite.

## Style

- Use `rustfmt` defaults
- Keep module/function names `snake_case`, types `CamelCase`, packages `zero-*`
- Prefer ASCII, keep directory names short
- Split files around 300 lines

## Tests

- Tests live in sibling `tests/` directories, not inline in logic files
- Name tests after the behavior they test
- Always add/update tests when changing config shape, protocol handling, routing, runtime behavior, or logging

## Boundaries

- Do not move protocol parsing into root binary
- Keep:
  - config ADTs and validation -> `crates/config`
  - routing -> `crates/router`
  - engine plan, session state, stats, events, groups, health -> `crates/engine`
  - listener lifecycle, protocol dispatch (`ProtocolRegistry`/`ProtocolAdapter`), `serve_inbound` pipeline, TCP/UDP dispatch, upstream connect -> `crates/proxy`
  - platform abstraction (socket, listener, stream) -> `crates/traits` + `crates/platform/tokio`
  - transport implementations (TLS, QUIC, WS, etc.) -> `crates/transport`
  - concrete protocol implementations -> `protocols/*`
- Protocol-private config fields (cert/key, cipher, etc.) are read by the protocol's own adapter, never by the proxy runtime directly
- Port conflict detection is authoritative in config validation (`DuplicateInboundListen`); bind-time errors mean external port occupation only
- `direct` and `block` target semantics stay inside `zero-engine`; socket-level direct execution stays in `zero-proxy`
- `mixed` is an inbound multiplexor, not an external protocol

## Control plane dispatch

- `ProtocolAdapter` is the single dispatch point for inbound bind/spawn. Adding a protocol = register an adapter; the runtime does not match on protocol config variants.
- `ProtocolAdapter::bind_inbound` owns the bind logic (TCP or QUIC) and reads its own protocol config. The runtime never touches protocol-private fields.
- Port conflicts surface eagerly (before accept loop spawn) via `bind_inbound_listener`.

## Docs

When changing config, protocol scope, or control surface, update matching docs in the same change:
- `docs/project/` for long-term rules
- `AGENTS.md` for structural and boundary changes
