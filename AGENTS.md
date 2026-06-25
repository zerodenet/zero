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
- `zero-proxy` — the orchestration layer: `Proxy`, `ProxyHandle`, `RunningProxy`, reload/reconcile, the `ProtocolRegistry` + `ProtocolAdapter` dispatch, protocol-local inbound listener entrypoints, `serve_inbound` unified TCP pipeline, TCP/UDP dispatch, upstream connect helpers. Depends on config/core/dns/engine/platform/traits/transport/tun/stack.
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
  - reload/reconcile, protocol dispatch (`ProtocolRegistry`/`ProtocolAdapter`), protocol-local inbound listener entrypoints, `serve_inbound` pipeline, TCP/UDP dispatch, upstream connect -> `crates/proxy`
  - platform abstraction (socket, listener, stream) -> `crates/traits` + `crates/platform/tokio`
  - transport implementations (TLS, QUIC, WS, etc.) -> `crates/transport`
  - concrete protocol implementations -> `protocols/*`
- Protocol-private config fields (cert/key, cipher, identity/user IDs, etc.) are read and parsed by the protocol's own adapter, never by the proxy runtime directly. Runtime code receives validated protocol values or opaque adapter-built keys, not raw protocol config strings.
- Port conflict detection is authoritative in config validation (`DuplicateInboundListen`); bind-time errors mean external port occupation only
- `direct` and `block` target semantics stay inside `zero-engine`; socket-level direct execution stays in `zero-proxy`
- `mixed` is an inbound multiplexor, not an external protocol, but it is still registered through `MixedAdapter` so runtime code does not special-case it

## Control plane dispatch

- `ProtocolAdapter` remains the registration compatibility source for inbound bind/spawn and UDP connect while TCP outbound dispatch is implemented explicitly through `TcpOutboundCapability`. Focused capability traits (`ProtocolSupportCapability`, `InboundListenerCapability`, `TcpOutboundCapability`, `UdpFlowCapability`, `UdpPacketPathCapability`) are the runtime-facing split surface; metadata and feature/support checks live in explicit `ProtocolSupportCapability` impls rather than on `ProtocolAdapter`; `ProtocolRegistry` stores registered capability objects; and capability methods receive narrow adapter context values (`InboundAdapterContext`, `OutboundAdapterContext`, `UdpAdapterContext`) instead of exposing full `Proxy` parameters. Adding a protocol = register an adapter; the runtime never matches on protocol config variants or `ResolvedLeafOutbound`.
- `ProtocolAdapter::bind_inbound` owns the bind logic (TCP or QUIC) and reads its own protocol config. The runtime never touches protocol-private fields.
- `ProtocolAdapter::spawn_inbound` owns the accept-loop spawn (clones the proxy, extracts the listener, calls the protocol-local `crate::inbound::run_<protocol>_listener_with_bound` module function). The runtime dispatches via `ProtocolRegistry::find_inbound` instead of a `match InboundProtocolConfig`. `mixed` is registered through `MixedAdapter`; the runtime does not special-case it.
- `TcpOutboundCapability::connect_tcp` owns the outbound TCP connect (dial + handshake) and `TcpOutboundCapability::apply_relay_hop` owns the relay-chain handshake over an existing stream. Each registered adapter implements this capability explicitly; the runtime dispatches via `ProtocolRegistry::find_outbound_leaf` instead of a `match ResolvedLeafOutbound`.
- `ProtocolAdapter::start_udp_flow` owns the single-hop UDP outbound flow establishment. The adapter drives its per-protocol manager through `ProtocolUdpState` to send the datagram; the runtime dispatches via `find_outbound_leaf` instead of a `match ResolvedLeafOutbound`. The same trait-dispatch pattern covers the UDP relay final hop (`start_udp_relay_final_hop`) and the VLESS two-stream XHTTP path (`udp_relay_needs_two_streams` + `start_udp_relay_two_stream`); `start_relay_flow` resolves the final hop's adapter and delegates — no per-protocol match.
- Generic UDP flow helpers and session state live under `runtime::udp_flow`; protocol-specific UDP ASSOCIATE handling lives under `protocol_runtime::socks5_udp_associate`, not under generic runtime.
- Protocol-specific UDP flow request types and manager-driving methods live under `protocol_runtime::udp::flows`; `runtime::udp_dispatch` must not declare a protocol-named `protocol_flows` module.
- UDP packet-path carrier snapshots and protocol-specific UDP flow snapshots live under `protocol_runtime::udp`; generic runtime flow state uses neutral variants (`Direct`, `Relay`, `Datagram`, `StreamPacket`) plus an opaque protocol snapshot reference and must not declare protocol-named outbound variants or carrier enums.
- UDP packet-path cache identity is adapter-built. Packet-path runtime may store opaque `cache_key` / `datagram_cache_key` values and parsed protocol values such as `CipherKind`, but it must not rebuild cache identity from raw protocol-private fields such as Shadowsocks cipher names.
- Protocol stream/datagram codecs own protocol crypto/framing state. For example, Mieru inbound data-phase encryption/decryption lives in `protocols/mieru::MieruInboundDataCodec`, and Shadowsocks inbound UDP decode/replay/response encoding lives in `protocols/shadowsocks::ShadowsocksInboundUdpCodec`; `zero-proxy` may wrap these codecs as Tokio stream/socket adapters but must not directly hold their cipher/session primitives or build/parse protocol frames.
- `crates/proxy/src/adapters/mod.rs`, `crates/proxy/src/inbound/mod.rs`, `crates/proxy/src/outbound/mod.rs`, `crates/proxy/src/protocol_adapter.rs`, `crates/proxy/src/protocol_adapter/registry.rs`, `crates/proxy/src/protocol_adapter/defaults.rs`, `crates/proxy/src/protocol_adapter/model.rs`, `crates/proxy/src/inventory.rs`, and `crates/proxy/src/inventory/udp.rs` are facades. Keep dispatch, validation, support lookup, metadata, adapter default bind/error helpers, adapter inbound/outbound models, and UDP leaf/relay/packet-path logic in their submodules; do not move adapter resolution or concrete helper logic back into facade roots.
- Adapter default TCP bind logic lives in `protocol_adapter/defaults/bind.rs`; default unsupported error construction lives in `protocol_adapter/defaults/errors.rs`. Adapter inbound bind/spawn models live in `protocol_adapter/model/inbound.rs`; outbound runtime facts live in `protocol_adapter/model/outbound.rs`.
- Protocol registry unit tests follow the same facade rule: `protocol_adapter/registry/tests.rs` only wires test modules, fixtures live in `registry/tests/fixtures.rs`, inbound registry coverage lives in `registry/tests/inbound.rs`, and outbound/block runtime coverage lives in `registry/tests/outbound.rs`.
- `ProtocolInventory` is the runtime-facing facade. Runtime code asks it to bind/spawn inbounds, connect TCP leaves/hops, start UDP leaf flows, start UDP relay final hops, and resolve UDP packet-path candidates. Runtime modules must not resolve adapter trait objects directly.
- `runtime.rs` owns `Proxy` construction and the run loop. Control-plane handle details live in `runtime/handle.rs`; spawned proxy handle details live in `runtime/running.rs`; reload channel bridging lives in `runtime/reload.rs`.
- Concrete protocol crate accessors must not be exposed on `ProtocolInventory`; `inventory/protocols.rs` exposes only neutral proxy-owned helpers such as the direct connector. Compiled adapter collection lives in `crates/proxy/src/register.rs`; inventory dispatch modules must not import protocol crates directly.
- Port conflicts surface eagerly (before accept loop spawn) via `bind_inbound_listener`.
- Per-protocol TCP connect logic lives in `crates/proxy/src/outbound/<protocol>.rs` (`connect_tcp` + `apply_tcp_hop`); only the owning `crates/proxy/src/adapters/<protocol>/tcp.rs` module calls it after extracting the leaf variant.
- UDP relay-chain datagram-over-packet-path helpers (`resolve_udp_packet_path_chain`, `owned_packet_path_carrier`) in `udp_dispatch/start/` still match on `ResolvedLeafOutbound` — these model carrier+datagram protocol *pairs* (SS→SS, SOCKS5→SS, H2→SS), not per-protocol dispatch.

## Docs

When changing config, protocol scope, or control surface, update matching docs in the same change:
- `docs/project/` for long-term rules
- `AGENTS.md` for structural and boundary changes
