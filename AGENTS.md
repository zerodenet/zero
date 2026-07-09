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
- `zero-proxy` — the orchestration layer: `Proxy`, `ProxyHandle`, `RunningProxy`, reload/reconcile, `ProtocolRegistry` capability dispatch, protocol-local inbound listener entrypoints, `serve_inbound` unified TCP pipeline, TCP/UDP dispatch, upstream connect helpers. Depends on config/core/dns/engine/platform/traits/transport/tun/stack.
- `zero-connector` — event dispatcher and panel push connector. Optional (`event_dispatcher`, `sink_jsonl`, `panel_connector` features).
- `zero-grpc` — gRPC control plane adapter. Optional (`grpc_api` feature).
- `zero-ffi` — C-compatible FFI bindings.
- `zero-tun` — platform-agnostic TUN device abstraction.
- `zero-stack` — user-space network stack (TCP termination + UDP forwarding from raw IP packets).
- `zero-ztls` — generic TLS 1.3 client with custom ClientHello (extracted from REALITY).

### Protocols

External protocol implementations under `protocols/`: `socks5`, `http-connect`, `vless`, `hysteria2`, `shadowsocks`, `trojan`, `vmess`, `mieru`. Each is a standalone crate that depends only on `zero-core` and `zero-traits` (plus its own crypto). The proxy wires them through registered capability objects.

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
  - reload/reconcile, protocol capability dispatch (`ProtocolRegistry`), protocol-local inbound listener entrypoints, `serve_inbound` pipeline, TCP/UDP dispatch, upstream connect -> `crates/proxy`
  - platform abstraction (socket, listener, stream) -> `crates/traits` + `crates/platform/tokio`
  - transport implementations (TLS, QUIC, WS, etc.) -> `crates/transport`
  - concrete protocol implementations -> `protocols/*`
- Protocol-private config fields (cert/key, cipher, identity/user IDs, etc.) are read from config by thin adapters or proxy transport bridges and parsed by protocol-owned constructors/helpers, never by the generic proxy runtime directly. Runtime code receives validated protocol values or opaque protocol/adapter-built keys, not raw protocol config strings.
- Trojan outbound password/TLS identity parsing follows the same rule: `protocols/trojan` owns TCP connect config, UDP flow resume, borrowed/owned TLS profile builders, and relay-TLS override resolution; proxy adapters only unpack leaf variants while transport bridges open transport streams through `zero-traits::ClientTlsProfile` instead of rebuilding Trojan TLS config inline.
- Port conflict detection is authoritative in config validation (`DuplicateInboundListen`); bind-time errors mean external port occupation only
- `direct` and `block` target semantics stay inside `zero-engine`; socket-level direct execution stays in `zero-proxy`
- `mixed` is an inbound multiplexor, not an external protocol, but it is still registered through `MixedAdapter` so runtime code does not special-case it

## Control plane dispatch

- `ProtocolAdapter` must not exist. Inbound bind/spawn, TCP outbound dispatch, UDP flow dispatch, and UDP packet-path roles are implemented explicitly through `InboundListenerCapability`, `TcpOutboundCapability`, `UdpFlowCapability`, and `UdpPacketPathCapability`. Focused capability traits (`ProtocolSupportCapability`, `InboundListenerCapability`, `TcpOutboundCapability`, `UdpFlowCapability`, `UdpPacketPathCapability`) are the runtime-facing split surface; metadata and feature/support checks live in explicit `ProtocolSupportCapability` impls; `ProtocolRegistry` stores registered capability objects; and capability methods receive narrow adapter context values (`InboundAdapterContext`, `OutboundAdapterContext`, `UdpAdapterContext`) instead of exposing full `Proxy` parameters. Adding a protocol = register its capability object; the runtime never matches on protocol config variants or `ResolvedLeafOutbound`.
- `InboundListenerCapability::bind_inbound` owns the bind logic (TCP or QUIC) and reads its own protocol config. The runtime never touches protocol-private fields.
- `InboundListenerCapability::spawn_inbound` owns only the capability-level delegation hook. External protocol bind/spawn entrypoints live under the owning adapter bridge surface: simpler protocols may still use `crates/proxy/src/adapters/<protocol>/inbound.rs`, while VLESS/VMess/Trojan have already collapsed their thin runtime handoff directly into `crates/proxy/src/adapters/{vless,vmess,trojan}.rs`. Protocol request models stay in `inbound/request.rs` only when the adapter still needs a local wrapper. When carrier setup or route-request ownership depends on TLS/WS/gRPC/H2/QUIC/REALITY, keep the owned request object plus raw carrier accept/build helpers in `zero-transport`. Carrier-to-runtime route handoff stays in the owning adapter bridge modules (`<protocol>.rs`, `inbound.rs`, or sibling `inbound/udp*.rs`), not in generic runtime. Proxy request facades must not keep duplicate TLS acceptors, Reality profiles, carrier option fields, or transport-owned route/fallback handoff wrappers inline once those helpers exist in `zero-transport`. When a protocol no longer needs a proxy-local wrapper, delete `crates/proxy/src/adapters/<protocol>/inbound/request.rs` entirely rather than keeping a hollow forwarding file; VLESS/VMess/Trojan are already in that state, and their thin runtime handoff now stays collapsed directly inside the adapter root rather than a separate `inbound.rs` or `inbound/dispatch.rs`. Proxy-owned `direct` / `mixed` / `system` / `tun` entrypoints stay under proxy runtime/inbound modules. The runtime dispatches via `ProtocolRegistry::find_inbound` instead of a `match InboundProtocolConfig`.
- Neutral accept-loop lifecycle, connection task fan-out, and connection-error logging belong in `runtime::listener_loop`. Protocol listener modules may choose which dispatch function to pass, but they should reuse shared helpers such as logged TCP/QUIC socket listener loops instead of duplicating `run_tcp_listener_loop` / `run_quic_stream_listener_loop` wrappers per protocol.
- Accepted inbound TCP/UDP/MUX route bridging lives under `runtime::inbound_route`. Protocol adapters/listeners may accept/authenticate clients and choose fallback/carrier handling, but they must hand protocol-owned route objects into `dispatch_protocol_stream_route` / `dispatch_protocol_mux_route` instead of spelling `.dispatch_inbound_route(...)` closure glue inline. Recorded/metered inbound route glue (for example VLESS `MeteredStream<RecordingStream<_>>` TCP/UDP/MUX handoff) also belongs in `runtime::inbound_route`; adapters must not unwrap recorded client streams or restitch shared MUX/stream UDP relay tasks inline once a shared helper exists.
- Shared stream wrappers used by runtime accounting and fallback replay (`RecordingStream`, `MeteredStream`, `StreamTraffic`, plus the neutral `zero_core::InboundFallbackCapture` impl over those wrappers for VLESS replay handoff) live in `crates/transport`, not `crates/proxy`. `zero-proxy` only consumes/re-exports them as orchestration glue.
- Protocol-owned inbound continuation helpers such as `accept_route_owned_with...` belong in `protocols/*`. Proxy transport request modules may pass closures into those helpers after carrier accept/build completes, but proxy transport bridges must not reintroduce protocol-named accepted-route enums or proxy-local dispatcher traits just to feed shared runtime route helpers.
- `TcpOutboundCapability::connect_tcp` owns the outbound TCP connect (dial + handshake) and `TcpOutboundCapability::apply_relay_hop` owns the relay-chain handshake over an existing stream. Each registered adapter implements this capability explicitly; the runtime dispatches via `ProtocolRegistry::find_outbound_leaf` instead of a `match ResolvedLeafOutbound`.
- When a relay hop also needs protocol-specific transport wrapping, `apply_relay_hop` must open that transport over the existing relay stream before invoking the protocol-owned handshake/session API. Examples include VLESS/VMess final-hop transport builders and Trojan TLS-over-relay.
- TCP runtime code must not unpack protocol-named `EstablishedTcpOutbound` variants. `transport/tcp_outbound.rs` owns TCP outbound result normalization such as extracting a neutral relay stream.
- `UdpFlowCapability::start_udp_flow` owns the single-hop UDP outbound flow establishment. The adapter drives its per-protocol manager through `ProtocolUdpState` to send the datagram; the runtime dispatches via `find_outbound_leaf` instead of a `match ResolvedLeafOutbound`. The same trait-dispatch pattern covers the UDP relay final hop (`start_udp_relay_final_hop`) and the VLESS two-stream XHTTP path (`udp_relay_needs_two_streams` + `start_udp_relay_two_stream`); `start_relay_flow` resolves the final hop's adapter and delegates — no per-protocol match.
- Generic UDP flow helpers and session state live under `runtime::udp_flow`; protocol-specific UDP ASSOCIATE and packet I/O details live in the owning protocol crate or its thin adapter glue, not under generic runtime.
- `runtime::udp_dispatch` root re-exports only generic dispatch result types (`FlowFailure`, `FlowStartResult`, `UdpCandidate`). Protocol-named UDP flow request models stay in their protocol flow submodules and must not be re-exported from the root facade.
- Protocol-specific UDP flow request types and manager-driving methods live under protocol-owned helpers plus the owning adapter bridge modules. For VLESS/VMess/Trojan managed stream flows, `zero-transport` owns `ResolvedLeafOutbound` projection, one-time materialization of leaf-local protocol request/flow/MUX plans, shared bridge state, reload eviction, carrier opening, managed UDP resume construction, the transport-leaf endpoint facts consumed by proxy (`ProtocolTransportLeaf`), UDP bridge-local stage / expected-leaf metadata (`ProtocolUdpTransportBridgeMetadata`, `ProtocolRelayTwoStreamUdpTransportBridgeMetadata`), and managed-stream handler stage metadata (`ProtocolManagedStreamUdpResumeMetadata`) in `crates/transport/src/{vless,vmess,trojan}_transport.rs`; proxy `crates/proxy/src/adapters/common.rs` owns only generic error/runtime wrappers for transport-leaf validation plus neutral shared relay-prefix orchestration, while the adapter roots (`crates/proxy/src/adapters/{vless,vmess,trojan}.rs`) keep only capability forwarding and direct calls into those generic helpers / transport bridge objects. Do not reintroduce proxy `udp.rs`, `udp/managed.rs`, or `udp/transport.rs` shells for those protocols. `runtime::udp_dispatch` must not declare protocol-named flow modules; its only protocol-state bridge is the narrow managed-flow/helper context.
- UDP packet-path carrier descriptors, datagram sources, lookup keys, flow snapshots, and packet-path chain orchestration live under `runtime::udp_flow`. Protocol-specific UDP resume/pump state is opaque to generic runtime and owned by the protocol crate or protocol-local adapter glue. Generic runtime flow state uses neutral variants (`Direct`, `Relay`, `Datagram`, `StreamPacket`, `PacketPathDatagram`) plus opaque `ManagedUdpFlowRef` or packet-path snapshots and must not declare protocol-named outbound variants or carrier enums.
- Runtime UDP flow facades may create tracked `UdpFlowOutbound` values, but they only register protocol resume state with `ProtocolUdpState` and store the returned opaque managed-flow reference. Runtime flow bookkeeping must not store or import `ProtocolUdpFlowSnapshot`.
- Existing protocol UDP flow forwarding uses `runtime::udp_flow::protocol_state` only as a dispatch facade. Protocol-specific snapshot extraction, packet I/O, and manager request construction live in protocol-owned transport bridges or `protocols/*`; do not put password/cipher/cache-key/relay-chain unpacking into generic runtime facades.
- Protocol-owned managed UDP flow plans stay in the protocol crate when a protocol needs to pair resume state with transport-side decisions. For example, `protocols/vless::udp::VlessUdpFlowPlan` owns direct-vs-relay mode, `protocols/vmess::udp::VmessUdpFlowPlan` owns mux concurrency selection, and `protocols/trojan::udp::TrojanUdpFlowPlan` owns direct-vs-relay cache-policy selection; proxy transport bridges may carry those opaque plans plus neutral transport options, but they must not recreate local `enum`/field state for the same protocol decisions.
- `UdpPacketPathCapability` owns packet-path carrier descriptor/snapshot construction, carrier build, and datagram-source classification. Do not add a monolithic adapter trait for packet-path methods.
- UDP packet-path cache identity is protocol/adapter-built. Packet-path runtime may store opaque `cache_key` / `datagram_cache_key` values, but it must not rebuild cache identity from raw protocol-private fields such as Shadowsocks cipher names.
- Packet-path entry build logic consumes datagram codecs supplied by `UdpDatagramSource`; generic packet-path entry code must not construct protocol-specific datagram codecs directly.
- Packet-path datagram sources carry only neutral descriptor identity and an adapter-provided datagram codec; packet-path state must not construct protocol-named snapshots directly.
- Packet-path datagram sources expose a datagram key part for cache identity; `runtime::udp_flow::packet_path_chain::key` must not read protocol-source internals directly.
- A `protocol_runtime::udp` root must not be reintroduced to re-export packet-path helper functions or generic packet-path runtime types. Adapters call `runtime::udp_flow::packet_path` constructors and `runtime::udp_flow::packet_path_chain::carriers::*` when bridging capability methods.
- `runtime::udp_flow::packet_path_chain.rs` must not re-export protocol carrier builder functions; adapters call `packet_path_chain::carriers::*` explicitly when bridging packet-path carrier capabilities.
- Protocol UDP types, codecs, managers, packet-path builders, flow resumes, and inbound UDP request/response models are not re-exported from protocol crate roots. Protocol UDP entrypoints live under each protocol's explicit `udp` module (for example `socks5::udp::*`, `shadowsocks::udp::*`, `hysteria2::udp::*`, `vless::udp::*`, `vmess::udp::*`, `trojan::udp::*`, and `mieru::udp::*`) or behind protocol-owned session APIs.
- Protocol stream/datagram codecs own protocol crypto/framing state. For example, Mieru inbound data-phase encryption/decryption and UDP associate packet decode/response encoding live in `protocols/mieru::udp`, Shadowsocks inbound UDP decode/replay/response encoding lives in `protocols/shadowsocks::udp`, and Trojan inbound UDP stream packet read/write helpers live in `protocols/trojan::udp`; `zero-proxy` may wrap these codecs as Tokio stream/socket adapters but must not directly hold their cipher/session primitives or build/parse protocol frames.
- Ordinary stream-carried inbound UDP relay wrappers (for example Trojan/VLESS/VMess) implement neutral `zero_core::InboundStreamUdpRelay` from protocol-owned relay types. Shared runtime stream UDP glue consumes that trait; adapters must not unpack relay fields just to rebuild generic stream UDP requests.
- MUX-carried inbound UDP relay wrappers (for example VLESS/VMess MUX UDP sub-streams) implement neutral `zero_core::InboundMuxUdpRelay` from protocol-owned relay types. Shared runtime MUX UDP glue consumes that trait; adapters must not unpack relay payload source/responder/auth fields just to rebuild generic MUX UDP requests.
- MUX-carried inbound TCP relay wrappers (for example VLESS/VMess MUX TCP sub-streams) implement neutral `zero_core::InboundMuxTcpRelay` from protocol-owned relay types. Shared runtime MUX TCP glue consumes that trait; adapters must not keep proxy-local bridge traits just to close or relay protocol-owned MUX TCP streams.
- Protocol-specific tunnel control negotiation also stays in the owning protocol crate. For example, Mieru socks5-in-tunnel CONNECT and UDP ASSOCIATE request/response choreography lives in `protocols/mieru::tunnel`; `zero-proxy` only opens the carrier socket and bridges the resulting protocol-owned stream/session objects.
- VMess inbound UDP request payload mode detection/parsing and response packet encoding live in `protocols/vmess::udp`; VLESS inbound UDP packet parsing and response/MUX response encoding live in `protocols/vless::udp`. Proxy inbound glue delegates packet wrapping/parsing to inbound-specific protocol sessions and must not name protocol-private UDP codec, dispatch, packet, response, or response-target models.
- `crates/proxy/src/adapters/mod.rs`, `crates/proxy/src/inbound/mod.rs`, `crates/proxy/src/protocol_registry/mod.rs`, `crates/proxy/src/protocol_registry/registry/mod.rs`, `crates/proxy/src/protocol_registry/defaults/mod.rs`, `crates/proxy/src/protocol_registry/model/mod.rs`, `crates/proxy/src/inventory.rs`, and `crates/proxy/src/inventory/udp.rs` are facades. Keep dispatch, validation, support lookup, metadata, default bind/error helpers, inbound/outbound models, and UDP leaf/relay/packet-path logic in their submodules; do not move adapter resolution or concrete helper logic back into facade roots. `crates/proxy/src/outbound/` must not be reintroduced as a protocol helper facade; protocol-specific outbound glue lives in the owning adapter/protocol modules.
- `crates/proxy/src/register.rs` is also a boundary surface, not a secondary adapter facade. Managed UDP handler registration uses adapter re-exports from the owning protocol bridge module, including the VLESS/VMess/Trojan managed stream handlers now exported from `crates/proxy/src/adapters/mod.rs`. `register.rs` must not reach removed `protocol_runtime` shims or revive proxy transport protocol shells.
- Default TCP bind logic lives in `protocol_registry/defaults/bind.rs`; default unsupported error construction lives in `protocol_registry/defaults/errors.rs`. Inbound bind/spawn models live in `protocol_registry/model/inbound.rs`; outbound runtime facts live in `protocol_registry/model/outbound.rs`.
- Protocol registry unit tests follow the same facade rule: `protocol_registry/registry/tests/mod.rs` only wires test modules, fixtures live in `registry/tests/fixtures.rs`, inbound registry coverage lives in `registry/tests/inbound.rs`, and outbound/block runtime coverage lives in `registry/tests/outbound.rs`.
- `ProtocolInventory` is the runtime-facing facade. Runtime code asks it to bind/spawn inbounds, connect TCP leaves/hops, start UDP leaf flows, start UDP relay final hops, and resolve UDP packet-path candidates. Runtime modules must not resolve adapter trait objects directly.
- `runtime.rs` owns `Proxy` construction and the run loop. Control-plane handle details live in `runtime/handle.rs`; spawned proxy handle details live in `runtime/running.rs`; reload channel bridging lives in `runtime/reload.rs`.
- Concrete protocol crate accessors must not be exposed on `ProtocolInventory`; `inventory/protocols.rs` exposes only neutral proxy-owned helpers such as the direct connector. Compiled adapter collection lives in `crates/proxy/src/register.rs`; inventory dispatch modules must not import protocol crates directly.
- Port conflicts surface eagerly (before accept loop spawn) via `bind_inbound_listener`.
- Per-protocol TCP connect glue stays in the owning adapter capability bridge. For VLESS/VMess/Trojan, the adapter roots (`crates/proxy/src/adapters/{vless,vmess,trojan}.rs`) route `ResolvedLeafOutbound` through the transport-owned bridge projection hook (`ResolvedProtocolTransportLeafProjector`) and the generic helpers in `crates/proxy/src/adapters/common.rs`, then delegate carrier opening through the transport-owned bridge objects (`VlessStreamBridge`, `VmessStreamBridge`, `TrojanTlsBridge`) plus protocol-owned requests. Transport-leaf endpoint facts, TCP open-result normalization (`ProtocolTransportLeaf`, `ProtocolTcpTransportOpenResult`), and bridge-local stage / expected-leaf metadata (`ProtocolTcpTransportBridgeMetadata`) are implemented in `zero-transport`, not in proxy adapter modules. Do not recreate `crates/proxy/src/transport/*` protocol shells, `crates/proxy/src/outbound/<protocol>.rs`, protocol-specific `leaf.rs` wrappers, proxy `tcp.rs` bridge shells, or `tcp/connect/*` forwarding buckets for those protocols.
- VLESS outbound deferred-response handshake selection and stream wrapping live in `protocols/vless::outbound`; `crates/transport::vless_transport` may classify whether the selected carrier requires deferred-response handling, but proxy adapters must not directly drive `DeferredTcpTunnelProtocol` or construct `DeferredVlessResponseStream`.
- VLESS direct outbound transport opening, outbound QUIC selection, and inbound QUIC bind-plan ownership live in `crates/transport/src/vless_transport.rs`. On the proxy side, `crates/proxy/src/adapters/common.rs` keeps only generic adapter-local error/runtime wrappers plus neutral shared relay-prefix orchestration over transport-owned outbound leaves; `zero-transport` owns outbound leaf projection from `ResolvedLeafOutbound`, one-time materialization of protocol request / UDP flow / MUX profile plans, transport-profile mapping, transport-leaf endpoint facts, TCP open-result normalization, and the bridge objects in `crates/transport/src/{vless,vmess,trojan}_transport.rs`; the adapter roots consume those transport-owned bridge objects directly for TCP and UDP capability dispatch.
- VLESS/VMess MUX pool cache state and concurrency reuse live in `protocols/vless::mux_pool` and `protocols/vmess::mux`. `VlessStreamBridge` / `VmessStreamBridge` in `zero-transport` hold shared pool state plus reload eviction; proxy adapter roots call those transport bridge helpers directly, while `crates/proxy/src/adapters/common.rs` keeps only generic wrappers plus neutral shared relay-prefix orchestration such as VLESS two-stream startup. VLESS/VMess protocol crates now own prepared outbound request bundles that already carry the protocol-selected MUX transport profile; transport leaves may pass only neutral `zero-traits::StreamMuxTransportHints` into those prepared bundles and must not re-derive protocol MUX state in hot paths. Legacy `transport/{vless_stream,vmess_stream,trojan_tls}` shells, proxy-local transport-leaf/result trait impls, adapter-local `mux_pool.rs` wrappers, proxy `tcp.rs` / `udp.rs` / `udp/managed.rs` bridge shells, and VLESS/VMess/Trojan `udp/transport.rs` facades must not come back. For VLESS/VMess/Trojan inbounds, transport-owned request objects now live in `crates/transport/src/{vless,vmess,trojan}_transport.rs`; proxy `inbound/request.rs` and `inbound/dispatch.rs` must stay absent once those transport-owned request objects are wired in, and route-object/runtime handoff stays in the adapter root rather than generic runtime.
- VLESS inbound TLS hello peeking, fallback-ALPN replay decision bridging, TLS accept, Reality base-stream upgrade, carrier dispatch, QUIC bind-plan construction, TCP accepted-route/fallback replay wrappers, and stream-route fallback adapters live in `crates/transport/src/vless_transport.rs`, while proxy-side capability forwarding, bind glue, and carrier-to-runtime route handoff live in `crates/proxy/src/adapters/vless.rs`. Runtime keeps neutral route orchestration helpers plus the shared fallback replay relay helper.
- VLESS inbound MUX identity stays protocol-private as the accepted user's UUID carried inside protocol-owned accept state; do not reintroduce a public `VlessInboundMuxContext` shell or expose raw `VlessInboundMuxSession` construction outside `protocols/vless`.
- SOCKS5, Shadowsocks, Mieru, and Hysteria2 inbound bridge layout follows the same adapter split: `inbound.rs` keeps bind/spawn glue, `inbound/request.rs` keeps protocol-owned acceptor/profile request models when still needed, and sibling `udp*.rs` or other local bridge modules keep carrier-to-runtime handoff only. Hysteria2 QUIC cert/key bind-plan ownership now lives in `crates/transport/src/hysteria2_quic.rs` rather than adapter bind glue.
- If an inbound protocol only needs protocol-owned client response formatting after route classification, expose that ability through a shared runtime-facing trait and implement it inside `protocols/*`; the shared runtime `ClientResponseInboundProtocol` wrapper may only call that trait. VLESS uses `zero_core::InboundClientResponse` for `send_ok` / `send_blocked` / `send_upstream_failure` instead of proxy-side response hook functions.
- VLESS inbound XHTTP/HTTPUpgrade/WebSocket/gRPC/H2 carrier selection lives in `crates/transport/src/vless_transport.rs`. `crates/proxy/src/adapters/vless.rs` may bridge accepted routes into shared runtime helpers, but adapter listeners must not reintroduce transport-open or protocol-private accept/session/fallback ownership that belongs in `zero-transport` or `protocols/vless`.
- Generic inbound TLS accept lives in `crates/transport/src/tls.rs::accept_tls_inbound`. Trojan and VMess inbound listeners must reuse that transport helper instead of calling `TlsAcceptor::accept` directly inside proxy glue.
- VMess inbound raw TLS accept, WebSocket framing, gRPC server framing, and owned route-handoff dispatch helpers are transport-owned building blocks (`tls`, `ws`, `grpc`, and `crates/transport/src/vmess_transport.rs`). Proxy-side capability forwarding plus route-object to runtime TCP/UDP/MUX dispatch glue live in `crates/proxy/src/adapters/vmess.rs`.
- Trojan inbound follows the same split: `crates/transport/src/trojan_transport.rs` owns inbound TLS accept building blocks plus owned route-handoff dispatch helpers, while proxy-side capability forwarding plus route-object to runtime stream UDP/TCP dispatch glue live in `crates/proxy/src/adapters/trojan.rs`.
- VMess inbound raw / WebSocket / gRPC carrier selection lives in `crates/transport/src/vmess_transport.rs`. Proxy listener glue must not keep `handle_vmess_raw`, `handle_vmess_ws`, or `handle_vmess_grpc` helpers, and Trojan inbound route dispatch follows the same adapter-local split through `crates/proxy/src/adapters/trojan.rs`.
- UDP relay-chain datagram-over-packet-path helpers (`resolve_udp_packet_path_chain`, `owned_packet_path_carrier`) in `udp_dispatch/start/` still match on `ResolvedLeafOutbound` — these model carrier+datagram protocol *pairs* (SS→SS, SOCKS5→SS, H2→SS), not per-protocol dispatch.

## Docs

When changing config, protocol scope, or control surface, update matching docs in the same change:
- `docs/project/` for long-term rules
- `AGENTS.md` for structural and boundary changes

