# 架构

仓库可以从以下分层来理解。

## 应用层

- 根 crate `zero`
- `zero-api`

负责 CLI 参数、配置文件路径、进程启动和状态输出。

管控面和可观测性模型遵循 Zero 自身约定。Clash、sing-box 和 Xray 等外部生态系统仅作为设计参考对待；兼容性适配层应存在于适配器、网关或外部工具中，不应约束内核或长期 API。

`zero-api` 定义外部控制、观测和事件导出能力。它不等同于 HTTP 服务，能力也不按传输线划分。HTTP/HTTPS、本地 IPC、文件、gRPC、二进制帧封装、Rust API 和 FFI 都作为特征实现或 feature-gated 适配器/sink 依附于同一核心能力集。

## 配置、决策和状态层

- `zero-config`
- `zero-engine`
- `zero-router`

`zero-config` 拥有配置模型和解析。`zero-router` 拥有规则匹配。`zero-engine` 拥有将配置编译为可执行计划、路由决策、目标解析、模式和组状态、会话、统计、事件和状态导出。

模式语义（`direct / global / rule`）和出站组语义（`selector / url_test / fallback`）也属于此层，不属于客户端。

`zero-engine` 不绑定 Tokio，不启动监听器，不持有协议实现，也不直接建立 socket 连接。`direct` / `block` 是此层内内置的目标语义；实际的网络执行由代理运行时层执行。

`zero-engine` 当前跨越多个决策边界运行：

- `RuntimeConfig`
  - 来自 `zero-config` 的输入
- `EnginePlan`
  - 不可变执行结构
- `EngineState`
  - 可变运行时状态，包括 `OutboundHealth`（熔断器）
- `view`
  - 用于 `status` / 导出 / 日志的标签渲染

热路径优先读取 plan/state 并通过借用边界传递引用；仅管控面和展示面回到字符串标签。

## 代理运行时层

- `zero-proxy`

`zero-proxy` 将 `zero-engine` 决策转化为实际代理执行：

- 启动入站监听器
- 调用协议实现进行握手和帧封装
- 建立直连或上游出站连接
- 运行 TCP 中继、UDP 关联、TLS、url_test 探测和熔断器健康检查
- 验证当前构建已编译配置引用的协议 features

此层可以依赖 Tokio 后端、协议 crates 和 `zero-engine`。它不重新解释配置语义，也不维护独立的模式、组或路由状态集。

### Adapter dispatch boundary

`ProtocolAdapter` does not exist. Protocol dispatch is represented by registered
capability objects.
Inbound bind/preparation dispatch is split out into explicit `InboundListenerCapability` implementations on each registered protocol bridge.
TCP outbound dispatch is split out into explicit `TcpOutboundCapability` implementations on each registered protocol bridge.
UDP flow dispatch is split out into explicit `UdpFlowCapability` implementations on each registered protocol bridge.
UDP packet-path carrier/datagram dispatch is split out into explicit `UdpPacketPathCapability` implementations on each registered protocol bridge.
Focused capability traits (`ProtocolSupportCapability`, `InboundListenerCapability`, `TcpOutboundCapability`, `UdpFlowCapability`, and `UdpPacketPathCapability`) are the runtime-facing protocol surface.
Metadata and feature/support checks live in explicit `ProtocolSupportCapability` implementations, not on a monolithic adapter trait.
Capability preparation entrypoints do not receive `Proxy`. TCP and UDP operation executors receive narrow `OutboundAdapterContext` / `UdpAdapterContext` values from runtime; inbound listener operations receive owned runtime execution inputs only after preparation.
`ProtocolRegistry` stores registered capability objects; a monolithic adapter trait must not be introduced.
`zero-proxy` runtime orchestration does not match on `InboundProtocolConfig` or `ResolvedLeafOutbound` to select a protocol path.
Adding a protocol means registering its capability object and adding protocol-local inbound/outbound code.

Capability implementations are preparation boundaries, not independent orchestration engines. Inbound, TCP, UDP flow, and UDP packet-path capabilities synchronously return boxed protocol-neutral operations. `ProtocolInventory` supplies runtime execution inputs and invokes them through `runtime::inbound_operation`, `runtime::tcp_dispatch::operation`, and `runtime::udp_dispatch::{operation,packet_path_operation}`. Direct, managed datagram, registered association, managed stream packet, transport bridge, two-stream UDP relay, and packet-path carrier/datagram roles all use this shape. Listener task ownership, accept-loop lifecycle, routing, connection/relay sequencing, flow/session lifecycle, observability, and error normalization remain runtime-owned. Protocol crates and transport bridges own wire handshakes and framing only.

External-protocol inbound bind and listener-operation preparation entrypoints live on the owning adapter bridge surface. `InboundListenerCapability::prepare_inbound_listener` validates and materializes protocol-owned accept/profile state without receiving `Proxy`, `BoundInbound`, shutdown channels, or a listener `JoinSet`. `ProtocolInventory` executes the returned `PreparedInboundListenerOperation`; adapters must not spawn listener tasks directly.
Simpler protocols may still use `crates/proxy/src/adapters/<protocol>/inbound.rs`, while VLESS/VMess/Trojan now keep their transport-request build plus listener handoff in adapter-local `listener.rs` bridge modules under `crates/proxy/src/adapters/{vless,vmess,trojan}/`. Adapters own the protocol-facing bind/spawn bridge surface; `Proxy` does not own protocol `run_*_listener_with_bound` methods. Listener-side post-accept defaults such as mux protocol labels, UDP protocol labels, panic messages, and error-log names are transport-owned request metadata, not adapter-local constant bundles.
`mixed` remains an inbound multiplexor rather than an external protocol, but it is registered through `MixedAdapter` so reload and spawn use the same adapter path as other inbounds.

`zero-proxy` keeps facade roots thin:

#### Runtime root ownership inventory

This table is normative for every direct child declared by `src/runtime.rs`.
"Facade" means declarations, re-exports, and narrow delegation only; executable
state machines must live in responsibility-named children. Production callers
name the owning path rather than every leaf function.

| Runtime root | Responsibility and lifecycle | Architectural owner | Production callers | Shape |
| --- | --- | --- | --- | --- |
| `engine_facade` | Narrow engine query/event/accounting access used by orchestration | proxy runtime over `zero-engine` | TCP/UDP lifecycle modules, handle | execution boundary |
| `handle`, `running`, `reload` | Control handle, spawned runtime ownership, reload channel and eviction | proxy runtime | public `Proxy` API, orchestration | facade families |
| `listeners`, `listener_loop` | Bind orchestration and neutral TCP/QUIC accept-loop lifecycle | proxy runtime | adapter listeners, `Proxy::run` | execution plus facade |
| `http_redirect`, `inbound_fallback` | HTTP redirect and VLESS fallback replay after carrier classification | proxy runtime; classification stays transport-owned | HTTP/VLESS adapter handoff | feature-owned exception |
| `inbound_route` | Post-accept stream, MUX, and recorded-route handoff | proxy runtime | adapter listener dispatch | facade family |
| `tcp_ingress` | Post-accept client response, rate limit, relay, accounting, session lifecycle | proxy runtime | `inbound_route`, proxy-owned inbounds | facade plus lifecycle |
| `tcp_dispatch` | Neutral outbound leaf selection and relay-prefix execution | proxy runtime | `tcp_ingress`, probes/health paths | facade family |
| `datagram_udp` | Datagram-protocol inbound request lifecycle | proxy runtime | Shadowsocks/Hysteria2 listeners | feature-owned facade |
| `stream_udp` | Stream-carried inbound UDP relay lifecycle | proxy runtime | VLESS/VMess/Trojan/Mieru route handoff | feature-owned execution |
| `mux_session`, `mux_tcp`, `mux_udp` | Accepted MUX session and substream TCP/UDP handoff | proxy runtime | VLESS/VMess accepted MUX routes | feature-owned execution |
| `packet_session_udp` | Packet-session inbound UDP relay lifecycle | proxy runtime | VLESS/VMess/Trojan/Mieru handoff | feature-owned facade |
| `udp_association` | Neutral SOCKS5 inbound UDP association lifecycle | proxy runtime | SOCKS5 adapter | feature-owned facade |
| `udp_ingress` | Neutral inbound packet submission into one session dispatcher | proxy runtime | datagram, stream, MUX, association bridges | execution boundary |
| `udp_dispatch` | Per-inbound-session route/start/forward state machine only | proxy runtime | `udp_ingress`, `UdpPipe` | facade plus session state |
| `udp_flow` | Neutral persistent UDP flow models; managed and registered execution families | proxy runtime; opaque protocol state stays protocol/transport-owned | `udp_dispatch`, inventory capabilities | facade family |
| `udp_socket` | Direct socket endpoint operations | proxy runtime/platform bridge | UDP dispatch/flow execution | execution helper |
| `udp_delivery` | Response delivery and accounting after an outbound flow produces packets | proxy runtime | UDP lifecycle/flow response tasks | execution helper |
| `pipe` | Kernel-facing TCP/UDP orchestration entry objects | proxy runtime | inbound route and UDP ingress paths | boundary facade |
| `orchestration` | Top-level run-loop task assembly, reload selection, and shutdown | proxy runtime | narrow `Proxy::run_until` entry | execution boundary |
| `path` | Neutral TCP/UDP path categories and outbound endpoint facts | proxy runtime | registry facts and TCP/UDP dispatch | model boundary |

Explicit naming exceptions are limited to `pipe` (the public kernel pipeline
concept) and feature-owned protocol-shape families (`datagram_udp`,
`stream_udp`, `packet_session_udp`, and `mux_*`). Bare `state`, `handler`,
`flow`, and `association` names are acceptable only below one of these roots
when the complete path identifies direction and lifecycle.

The persistent `udp_flow` family must not import `UdpDispatch`. Managed bridge
startup receives `UdpFlowStartContext`, which exposes only persistent flow
start and resume registration over `inbound_tag + UdpFlowState`. Response
accounting, response DTOs, and client delivery helpers live under
`udp_delivery`, not in a generic `udp_flow/helpers.rs` bucket.

#### Adapter ownership inventory

| Adapter | Protocol-owned behavior | Transport-owned behavior | Proxy delegation/state | Registered capabilities |
| --- | --- | --- | --- | --- |
| `direct` | none | direct connector primitives | proxy-owned connector; no protocol-private state | support, TCP outbound |
| `http` | HTTP CONNECT handshake/parser | inbound acceptor construction | listener handoff; stateless | support, inbound, TCP outbound |
| `mixed` | SOCKS5/HTTP protocol implementations | mixed accept classification | proxy-owned multiplexor; stateless | support, inbound |
| `socks5` | handshake, UDP association framing/classification | inbound acceptor | listener/association handoff and upstream handler registration; stateless | support, inbound, TCP outbound, UDP flow, packet path, upstream provider |
| `shadowsocks` | cipher/session/framing and UDP codec | inbound profile and carrier opening | listener plus managed datagram handler assembly; stateless | support, inbound, TCP outbound, UDP flow, packet path, managed provider |
| `hysteria2` | authenticated QUIC/session and UDP semantics | QUIC bind/profile/connection projection | listener plus managed handler assembly; stateless | support, inbound, TCP outbound, UDP flow, managed provider |
| `mieru` | tunnel negotiation, session/framing, UDP plan | inbound profile and carrier opening | listener and managed-flow delegation; stateless | support, inbound, TCP outbound, UDP flow, managed provider |
| `vless` | handshake, accepted routes, MUX/UDP plan and pool | leaf projection, carrier plans/opening, shared stream bridge | collapsed capability root holding one transport bridge | support, inbound, TCP outbound, UDP flow, packet path, managed provider |
| `vmess` | handshake, crypto/framing, accepted routes, MUX/UDP plan and pool | leaf projection, carrier plans/opening, shared stream bridge | collapsed capability root holding one transport bridge | support, inbound, TCP outbound, UDP flow, managed provider |
| `trojan` | handshake, TLS profile resolution, accepted routes and UDP plan | leaf projection, TLS carrier opening, shared stream bridge | collapsed capability root holding one transport bridge | support, inbound, TCP outbound, UDP flow, managed provider |

The inventory describes ownership, not permission to grow forwarding layers.
For each row, registration must derive all focused views from the same adapter
allocation; protocol-private parsing and transport-plan construction remain on
the owners named in the first two columns.

- `adapters/mod.rs` only declares concrete adapter modules and re-exports adapter types that genuinely live under adapters. Registry construction and protocol dispatch stay outside this facade; for VLESS/VMess/Trojan, managed UDP stream handler registration now comes through the adapter roots themselves because proxy `udp.rs` shells stay absent, adapter roots call the neutral owners directly, and `adapters/identity.rs` stays limited to adapter identity, support predicates, and transport-bridge classification.
- Adapter UDP modules construct their assembly-time managed-flow handler directly at the owning `udp.rs` capability boundary. They do not add a one-function `udp/handler.rs` forwarding shell or use `udp/managed.rs`, because reusable managed-flow execution belongs to `runtime::udp_flow::managed` rather than the protocol adapter.
- `inbound/mod.rs` only declares proxy-owned inbound listener modules (`direct`, `system`, `tun`). External protocol request models plus bind/spawn/listener/session glue stay under the owning adapter bridge surface: `src/adapters/<protocol>/inbound*` when a local wrapper still exists, otherwise in adapter-local bridge modules such as `src/adapters/{vless,vmess,trojan}/listener.rs`.
- `src/runtime/inbound_route.rs`, `src/runtime/inbound_route/stream.rs`, `src/runtime/inbound_route/mux.rs`, and `src/runtime/inbound_route/recorded.rs` are facade roots. `src/runtime/inbound_route.rs` only re-exports neutral post-accept stream-route, mux-route, and recorded-route helpers; `src/runtime/inbound_route/stream.rs` keeps only the stream-route facade over `stream/{model,dispatch,no_client}.rs`; `src/runtime/inbound_route/mux.rs` keeps only the mux-route facade over `mux/{model,dispatch,no_client}.rs`; and `src/runtime/inbound_route/recorded.rs` keeps only the recorded post-accept facade over `recorded/{model,helpers,dispatch,request}.rs`. Shared route handoff logic stays in those submodules rather than accumulating back into the root files.
- `src/runtime/tcp_ingress.rs` is the post-accept TCP execution surface over `tcp_ingress/{contract,lifecycle}.rs`; it owns client-response policy, rate limiting, `TcpPipe` entry, relay accounting, and session lifecycle after a protocol or transport bridge has already produced `Session + client stream`. The historical `inbound_protocol` module name must not return because runtime does not own a generic protocol accept phase. Neutral UDP packet submission, endpoint/socket operations, and response delivery accounting are named separately as `udp_ingress`, `udp_socket`, and `udp_delivery`.
- `src/transport/mod.rs` is the explicit proxy transport boundary facade, not a second concrete transport implementation crate. It re-exports the neutral stream/QUIC contracts consumed by proxy and owns proxy-specific direct connection, TCP outbound normalization, relay-chain handoff, metering, and rate-limited relay glue. Concrete carrier implementations stay in `zero-transport`; migration residue such as a standalone `tcp_flow.rs` helper bucket must not return.
- `src/runtime/tcp_dispatch.rs`, `src/runtime/datagram_udp.rs`, `src/runtime/packet_session_udp.rs`, `src/runtime/udp_association.rs`, `src/runtime/udp_dispatch/managed.rs`, `src/runtime/udp_flow/managed/mod.rs`, `src/runtime/udp_flow/managed/flow.rs`, `src/runtime/udp_flow/managed/model.rs`, `src/runtime/udp_flow/managed/bridge.rs`, `src/runtime/udp_flow/managed/state.rs`, `src/runtime/udp_flow/managed/datagram.rs`, `src/runtime/udp_flow/managed/datagram/connection.rs`, `src/runtime/udp_flow/managed/connection.rs`, `src/runtime/udp_flow/managed/cache.rs`, `src/runtime/udp_flow/managed/cache/stream.rs`, `src/runtime/udp_flow/managed/stream.rs`, `src/runtime/udp_flow/managed/stream_manager.rs`, `src/runtime/udp_flow/managed/datagram_manager.rs`, `src/runtime/udp_flow/managed/stream_manager/connector.rs`, `src/runtime/udp_flow/managed/stream_manager/manager.rs`, `src/runtime/udp_flow/managed/datagram_manager/connector.rs`, `src/runtime/udp_flow/managed/datagram_manager/manager.rs`, `src/runtime/udp_flow/managed/bridge/transport.rs`, `src/runtime/udp_flow/managed/bridge/transport/two_stream.rs`, `src/runtime/udp_flow/managed/bridge/stream_packet.rs`, `src/runtime/udp_flow/managed/state/start.rs`, `src/runtime/udp_flow/registered/mod.rs`, `src/runtime/udp_flow/registered/state.rs`, `src/runtime/udp_flow/registered/upstream.rs`, `src/runtime/udp_flow/registered/upstream/contract.rs`, `src/runtime/udp_flow/registered/upstream/runtime.rs`, `src/runtime/udp_flow/registered/upstream/runtime/association.rs`, `src/runtime/udp_flow/registered/upstream/state/handlers.rs`, and `src/transport/tcp_outbound.rs` only re-export neutral runtime helpers. Their implementation lives in `tcp_dispatch/{leaf,relay}.rs`, `transport/tcp_outbound/{error,model,connect,relay,result}.rs`, `datagram_udp/{contract,lifecycle}.rs`, `packet_session_udp/{contract,lifecycle}.rs`, `udp_association/{contract,lifecycle}.rs`, `udp_dispatch/managed/{model,start,forward}.rs`, `managed/flow/{request,resume}.rs`, `managed/model/{handler,send}.rs`, `managed/bridge/{stream_packet,transport,error}.rs`, `managed/bridge/stream_packet/{handler,request,start}.rs`, `managed/bridge/transport/{direct,relay,two_stream}.rs`, `managed/bridge/transport/two_stream/{predicate,start,flow}.rs`, `managed/state/{model,registry,start,forward,error}.rs`, `managed/state/start/{datagram,stream,dispatch}.rs`, `managed/datagram/{response,connection,state}.rs`, `managed/datagram/connection/{model,sender,flow,ops}.rs`, `managed/connection/{model,response,tuple,packet}.rs`, `managed/cache/{key,stream,datagram}.rs`, `managed/cache/stream/{model,insert,send}.rs`, `managed/stream/{model,start,forward}.rs`, `managed/stream_manager/{connector,manager}.rs`, `managed/stream_manager/connector/{flow,packet,tuple}.rs`, `managed/stream_manager/manager/{model,send,relay,mismatch}.rs`, `managed/datagram_manager/{connector,manager}.rs`, `managed/datagram_manager/connector/{flow,socket}.rs`, `managed/datagram_manager/manager/{model,flow,socket,mismatch}.rs`, `registered/state/{model,lifecycle,start}.rs`, `registered/upstream/contract/{target,resume,model,transport,handler}.rs`, `registered/upstream/{runtime,state}.rs`, `registered/upstream/runtime/{association,control,handler}.rs`, `registered/upstream/runtime/association/{model,lifecycle,response}.rs`, and `registered/upstream/state/handlers/{model,dispatch,view,lifecycle}.rs`.
- `src/outbound/` does not exist. Per-protocol TCP outbound glue lives in the owning adapter capability bridge or transport bridge surface; protocol handshake/session details live in `protocols/*`. For VLESS/VMess/Trojan, proxy `tcp.rs` shells stay absent and the adapter roots forward directly into transport-owned bridge objects.
- `protocol_registry/mod.rs` only re-exports focused capability traits, adapter contexts, registry models, and registry.
- `protocol_registry/defaults/mod.rs` only wires default helper modules. TCP bind defaults live in `defaults/bind.rs`; unsupported error construction lives in `defaults/errors.rs`.
- `protocol_registry/model/mod.rs` only wires registry model modules. Inbound bind/spawn models live in `model/inbound.rs`; outbound runtime facts live in `model/outbound.rs`.
- `protocol_registry/registry/mod.rs` only owns the registry struct and submodule wiring. The low-level register helper lives in `registry/build.rs`; compiled protocol collection lives in `src/register.rs`; inbound dispatch, outbound dispatch, metadata, support lookup, validation, and runtime fact lookup live in `registry/{inbound,outbound,metadata,support,validation,runtime}.rs`.
- `protocol_registry/registry/tests/mod.rs` only wires registry test modules. Shared fixtures live in `registry/tests/fixtures.rs`; inbound coverage lives in `registry/tests/inbound.rs`; outbound and `block` kernel fact coverage live in `registry/tests/outbound.rs`.
- `inventory.rs` only owns the runtime-facing `ProtocolInventory` shell. Inbound, TCP, UDP, metadata, direct connector access, and runtime-fact lookups live in sibling modules.
- `inventory/protocols.rs` exposes only neutral proxy-owned protocol helpers such as the direct connector; concrete protocol object construction stays in protocol-local integration code or the compiled registration surface.
- `inventory/udp.rs` only wires UDP inventory submodules. Single-hop leaf dispatch, relay final-hop dispatch, and packet-path adapter probing live in `inventory/udp/{leaf,relay,packet_path}.rs`.

Runtime modules depend on `ProtocolInventory` operations, not adapter trait object lookup. Adapter resolution stays behind `ProtocolInventory` and `ProtocolRegistry`; adapters extract config variants and dispatch into protocol/transport-owned bridge helpers. Protocol identity and cipher parsing is protocol-owned: runtime modules receive validated protocol values, or opaque protocol/adapter-built keys when they need stable cache identity.
Trojan follows the same ownership rule for outbound transport identity: `protocols/trojan` owns TCP connect config composition, UDP flow resume construction, borrowed/owned TLS profile shaping, and relay override resolution; proxy glue only passes protocol-built TLS profile objects into neutral transport TLS primitives.

UDP runtime flow snapshots use path categories rather than protocol variants. `runtime::udp_flow` records `Direct`, `Relay`, `Datagram`, `StreamPacket`, and `PacketPathDatagram` flow shape plus endpoint/session metadata. Generic packet-path carrier descriptors, datagram sources, lookup keys, flow snapshots, and chain orchestration live under `runtime::udp_flow`; protocol-specific UDP resume/pump state is opaque to generic runtime and owned by the protocol crate or protocol-local adapter glue. `runtime::udp_dispatch` tracks externally managed protocol flows through opaque `ManagedUdpFlowRef` values, not protocol snapshot enums.
Runtime UDP flow facades may create tracked `UdpFlowOutbound` values, but they only register opaque protocol resume state through `runtime::udp_flow::registered` and store the returned managed-flow reference. Runtime flow bookkeeping does not store or import protocol-named UDP flow snapshots.
The `runtime::udp_dispatch` root facade re-exports only the session route candidate from `udp_dispatch/candidate.rs` plus flow-owned result types (`FlowFailure`, `FlowStartResult`, `UdpCandidate`). `FlowFailure` and `FlowStartResult` are defined in `runtime::udp_flow::result`, while persistent snapshots are defined in `runtime::udp_flow::snapshot`, because they describe flow establishment/state rather than per-session route selection. Protocol-named UDP flow request models stay in proxy transport bridges or `protocols/*` helpers instead of being collected at the root facade. The `src/runtime/udp_dispatch/managed.rs` facade keeps only neutral tracked-start request models plus dispatch entrypoint re-exports; its tracked-start / managed-flow registration / relay-forward glue lives in `src/runtime/udp_dispatch/managed/{model,start,forward}.rs`. For VLESS/VMess/Trojan managed stream flows, `zero-transport` owns `ResolvedLeafOutbound` projection, transport-leaf endpoint facts, one-time protocol plan materialization, carrier opening, shared bridge state, managed-flow connection/connector traits, UDP bridge-local stage / expected-leaf metadata, and managed-stream handler stage metadata. On the proxy side, `src/runtime/udp_flow/managed/bridge.rs` owns the neutral managed UDP relay/two-stream orchestration, `src/adapters/identity.rs` stays limited to adapter identity and bridge classification, and the adapter roots `src/adapters/{vless,vmess,trojan}.rs` keep only capability forwarding and direct calls into those generic helpers / transport bridge objects. Proxy `udp.rs`, `udp/managed.rs`, and `udp/transport.rs` shells for those protocols must stay absent. `runtime::udp_dispatch` keeps only the narrow context needed to invoke registered handlers and managed-flow helpers without exposing protocol state.
That remaining adapter-root code for VLESS/VMess/Trojan is intentional: once the protocol-specific proxy `tcp.rs` / `udp.rs` shells are gone, the root may keep the bridge object plus explicit capability impls, but it must not regrow protocol-private parsing, transport-plan construction, accepted-route ownership, mux-pool fields, or request builders.
The UDP taxonomy is directional and responsibility-based: `runtime::udp_dispatch` is the per-inbound-session routing/start/forward state machine; `runtime::udp_flow` owns neutral flow models and persistent state; `runtime::udp_flow::managed` owns reusable execution machinery for resumable stream/datagram flows; and `runtime::udp_flow::registered` owns the handler set assembled by `register.rs` plus neutral invocation state. `runtime::udp_association` is an inbound relay-loop shape, while `registered/upstream` owns outbound upstream association lifecycle. Protocol-specific snapshot extraction, upstream request shaping, packet I/O, and manager send request construction live in protocol-owned adapter modules or `protocols/*`, not in generic runtime facades.
When a managed UDP stream protocol needs to pair resume state with transport-side policy, that plan is also protocol-owned. For example, `protocols/vless::udp::VlessUdpFlowPlan` owns the direct/relay mode selection, `protocols/vmess::udp::VmessUdpFlowPlan` owns mux concurrency selection, and `protocols/trojan::udp::TrojanUdpFlowPlan` owns the direct/relay cache-policy split; proxy transport bridges may carry those opaque plans plus neutral transport options, but they do not mirror the same protocol decisions as local enums or fields.

`runtime.rs` owns the `Proxy` shell and run loop. Control-plane handle details live behind the facade root `runtime/handle.rs` with query/command/event/util splits under `runtime/handle/`, the spawned runtime handle lives in `runtime/running.rs`, and reload channel bridging lives in `runtime/reload.rs`.

Per-protocol outbound TCP preparation lives in the owning adapter capability bridge. Adapters return `PreparedTcpConnectOperation` / `PreparedTcpRelayOperation` objects and never invoke their executors. `ProtocolInventory` supplies runtime context and executes them. Direct, ordinary socket-handshake, shared session-handshake, and VLESS/VMess/Trojan transport-bridge operations all converge in `runtime::tcp_dispatch::operation`; runtime owns dialing, relay sequencing, traffic recording, result normalization, and error mapping, while protocol/transport leaf contracts execute wire handshakes. `src/outbound/<protocol>.rs` helper modules, `src/adapters/<protocol>/tcp/connect/*` forwarding shells, and VLESS/VMess/Trojan proxy `tcp.rs` bridge shells must not be reintroduced.
VLESS outbound deferred-response handshake selection and response-stream wrapping stay in `protocols/vless::outbound`; `crates/transport/src/vless_transport.rs` may classify whether the chosen transport requires deferred-response handling, but proxy adapters must not directly drive `DeferredTcpTunnelProtocol` or construct `DeferredVlessResponseStream`.
VLESS direct outbound transport opening, including QUIC selection, also lives in `crates/transport/src/vless_transport.rs`; on the proxy side, `zero-transport` now owns `ResolvedLeafOutbound` projection plus one-time materialization of leaf-local protocol request / flow / MUX plans, transport-profile mapping, transport-leaf endpoint facts, TCP open-result normalization, and the transport bridge objects for VLESS/VMess/Trojan, while `src/adapters/identity.rs` stays at named/bridge helper level, `src/protocol_registry/{model/defaults}` own neutral outbound runtime facts and default mismatch errors, `src/transport/tcp_outbound.rs` stays the neutral TCP bridge facade over `transport/tcp_outbound/{error,model,connect,relay,result}.rs`, `src/runtime/udp_flow/managed/bridge.rs` owns neutral managed UDP relay orchestration, and the adapter roots consume those bridge objects directly for TCP/UDP capability dispatch.
The same ownership rule now covers VLESS inbound base transport setup: TLS ClientHello peeking, fallback-ALPN replay classification, TLS accept, Reality upgrade, carrier dispatch, owned inbound transport-plan construction, TCP accepted-route/fallback replay wrappers, stream-route fallback adapters, the listener-side recorded mux defaults, and the QUIC bind plan all live in `crates/transport/src/vless_transport.rs`; `src/adapters/vless.rs` owns capability forwarding and bind/spawn delegation; `src/adapters/vless/listener.rs` builds the transport-owned request, calls the shared logged TCP/QUIC listener loops directly, and hands accepted routes into recorded post-accept runtime helpers; and shared fallback replay relay plus `src/runtime/inbound_route.rs` stay protocol-neutral and post-accept only.
VLESS client response formatting now uses the shared runtime `ClientResponseInboundProtocol` wrapper backed by the common `zero_core::InboundClientResponse` trait instead of a protocol-specific proxy `InboundProtocol` impl file or proxy-side response hook functions; `protocols/vless::inbound::VlessInbound` implements that trait and transport route glue only passes the protocol object into the neutral wrapper.
VLESS inbound XHTTP / HTTPUpgrade / WebSocket / gRPC / H2 carrier selection follows the same split in `crates/transport/src/vless_transport.rs`; the proxy-side `src/adapters/vless.rs` bridge keeps only route-object to runtime TCP/UDP/MUX handoff, while fallback replay relay stays in shared runtime glue.
VLESS inbound MUX identity stays protocol-private as the accepted user's UUID carried inside protocol-owned accept state; do not reintroduce a separate public mux-context shell between inbound parsing and `VlessInboundMuxServer` construction.
Trojan and VMess inbound base TLS accept follow the same split through `crates/transport/src/tls.rs::accept_tls_inbound`; proxy listeners do not call `TlsAcceptor::accept` directly.
VMess inbound WebSocket and gRPC carrier framing are also transport-owned (`crates/transport/src/ws.rs`, `crates/transport/src/grpc.rs`); protocol accept-route dispatch lives in `src/adapters/vmess.rs`, `crates/transport/src/vmess_transport.rs` owns carrier accept/build helpers, owned inbound transport-plan construction, and route-handoff dispatch helpers, and proxy listener glue keeps only runtime TCP/UDP/MUX dispatch handoff.
VMess inbound raw / WebSocket / gRPC carrier selection is centralized in `crates/transport/src/vmess_transport.rs`; proxy listener glue does not keep per-carrier helper functions or local mux-default bundles, and Trojan inbound route dispatch follows the same pattern through transport-owned route-handoff helpers in `src/adapters/trojan.rs`.
Inbound bridge layout follows responsibility count rather than a uniform directory template. HTTP, Mixed, Shadowsocks, Mieru, and Hysteria2 keep bind/spawn delegation, listener lifecycle, and post-accept runtime handoff in a single `src/adapters/<protocol>/inbound.rs`; an `inbound/` directory must not exist solely to contain `listener.rs`. SOCKS5 retains `src/adapters/socks5/inbound/` because listener lifecycle and client UDP association are separate sibling responsibilities. Sibling top-level UDP adapter modules continue to own outbound UDP-specific handoff. In the current tree these simpler protocols no longer keep proxy-local `inbound/request.rs`; `zero-transport` owns their acceptor/profile builders through `crates/transport/src/{socks5_transport,shadowsocks_transport,mieru_transport,hysteria2_quic}.rs`. Hysteria2 QUIC cert/key bind-plan ownership also lives in `crates/transport/src/hysteria2_quic.rs` instead of the adapter bind hook. For VLESS/VMess/Trojan, the owned inbound request objects now live in `zero-transport`; proxy `inbound/request.rs` and `inbound/dispatch.rs` should stay absent once no local wrapper remains; adapter-local `listener.rs` bridges build those requests, call the logged listener loops directly, and hand accepted routes into post-accept runtime helpers; and `src/runtime/inbound_route.rs` stays post-accept only.
SOCKS5 UDP association now uses a neutral association bridge: `protocols/socks5` owns packet classification plus response framing through `zero_core::{InboundUdpAssociation, InboundUdpAssociationResponder, InboundUdpAssociationResponse}`, while `src/runtime/udp_association.rs` stays a facade over `udp_association/{contract,lifecycle}.rs` for UDP dispatch/accounting/send orchestration. Adapter-local SOCKS5 association response/dispatch bridge files must not return.
Neutral TCP / QUIC accept-loop lifecycle, per-connection task fan-out, and connection-error logging live in `src/runtime/listener_loop.rs`. Protocol listener modules choose only the protocol dispatch function and transport-owned request object they pass into those shared helpers; protocol listeners call the logged TCP/QUIC socket listener loops directly, they do not duplicate `run_tcp_listener_loop` / `run_quic_stream_listener_loop`, and proxy does not keep a second generic accept-stage handoff layer above those loops.
Relay-chain TCP hops follow the same split: adapters reuse transport-over-stream builders in `zero-transport` for VLESS/VMess/Trojan and then invoke protocol-owned handshake/session APIs over the wrapped stream, while `src/runtime/tcp_dispatch.rs` stays a facade over `tcp_dispatch/{leaf,relay}.rs` for neutral leaf selection, first-hop normalization, and relay-prefix orchestration.
VLESS/VMess outbound MUX pool cache state and concurrency reuse live in `protocols/vless::mux_pool` and `protocols/vmess::mux`. `VlessStreamBridge` / `VmessStreamBridge` in `zero-transport` now hold the shared pool state plus reload eviction; proxy adapter roots call those transport bridge helpers directly; `src/runtime/udp_flow/managed/bridge.rs` owns neutral two-stream startup such as the VLESS paired relay path; and `src/adapters/identity.rs` must not regrow shared outbound wrappers. Shared transport opener glue and cached MUX connection wiring live in `crates/transport/src/transport_plan.rs` and `crates/transport/src/mux_stack.rs`, while `crates/transport/src/{vless,vmess}_transport.rs` still own the protocol-specific leaf projection, prepared-request materialization, and MUX carrier build/use details. `protocols/vless` and `protocols/vmess` now own prepared outbound request bundles that already capture the protocol-selected MUX transport profile; `zero-transport` may pass only neutral `zero-traits::StreamMuxTransportHints` into those prepared bundles while owning outbound leaf projection, split-http relay decisions, transport-leaf endpoint facts, TCP open-result normalization, and the owned inbound request objects for VLESS/VMess/Trojan. Trojan transport no longer rebuilds `ClientTlsConfig` from protocol fields; it consumes protocol-owned owned TLS profiles through `zero-traits::ClientTlsProfile`. Adapter roots stay at strict capability forwarding and hold only the transport bridge object rather than protocol-specific pool fields; proxy `tcp.rs`, `udp.rs`, and `udp/managed.rs` bridge shells for VLESS/VMess/Trojan must stay absent. For inbound protocols that need protocol-private transport setup, proxy `src/adapters/<protocol>/inbound.rs` continues to own carrier-specific bind/spawn delegation when a local wrapper is still needed; VLESS/VMess/Trojan keep only the thin capability hook in `src/adapters/{vless,vmess,trojan}.rs`; adapter-local `listener.rs` bridges pass transport-owned request objects directly into shared logged listener loops plus post-accept runtime helpers; and `zero-transport` owns carrier accept/build helpers, transport-neutral primitives, owned inbound request/transport-plan construction, and TLS/WS/gRPC/H2/QUIC/REALITY carrier setup.
TCP runtime code does not unpack protocol-named `EstablishedTcpOutbound` variants. The transport TCP outbound model owns result normalization, including extracting a neutral relay stream for relay-chain prefix execution.

`UdpPacketPathCapability` owns packet-path carrier descriptor/snapshot construction, carrier build, and datagram-source classification. UDP packet-path cache identity is protocol/adapter-built. Packet-path runtime may store opaque carrier `cache_key` and datagram `datagram_cache_key` values; it must not reconstruct cache identity from raw protocol-private config strings such as Shadowsocks cipher names.
Packet-path entry build logic consumes datagram codecs supplied by `UdpDatagramSource`; generic packet-path entry code must not construct protocol-specific datagram codecs directly.
Packet-path datagram sources carry only neutral descriptor identity and an adapter-provided datagram codec; packet-path state must not construct protocol-named snapshots directly.
Packet-path datagram sources expose a datagram key part for cache identity; `runtime::udp_flow::packet_path_chain::key` must not read protocol-source internals directly.
A `protocol_runtime::udp` root must not be reintroduced to re-export packet-path helper functions or generic packet-path runtime types. Adapters call `runtime::udp_flow::packet_path` constructors and `runtime::udp_flow::packet_path_chain::carriers::*` when bridging capability methods.
`runtime::udp_flow::packet_path_chain.rs` does not re-export protocol carrier builder functions; adapters call `packet_path_chain::carriers::*` explicitly when bridging packet-path carrier capabilities.

Protocol UDP types, codecs, managers, packet-path builders, flow resumes, and inbound UDP request/response models are not re-exported from protocol crate roots. Protocol UDP entrypoints live under each protocol's explicit `udp` module (for example `socks5::udp::*`, `shadowsocks::udp::*`, `hysteria2::udp::*`, `vless::udp::*`, `vmess::udp::*`, `trojan::udp::*`, and `mieru::udp::*`) or behind protocol-owned session APIs.
Protocol stream/datagram codecs own protocol crypto/framing state. For example, Mieru inbound data-phase encryption/decryption and UDP associate packet decode/response encoding live in `protocols/mieru::udp`, Shadowsocks inbound UDP decode/replay/response encoding lives in `protocols/shadowsocks::udp`, and Trojan inbound UDP stream packet read/write helpers live in `protocols/trojan::udp`; `zero-proxy` only wraps those codecs as Tokio stream/socket adapters and must not hold their cipher/session primitives or build/parse protocol frames directly.
Ordinary stream-carried inbound UDP relay wrappers also stay protocol-owned. Protocol crates expose relay objects that implement neutral `zero_core::InboundStreamUdpRelay`, and shared runtime stream UDP glue consumes that trait instead of unpacking protocol-specific stream/responder/auth fields inside adapters.

MUX-carried inbound UDP relay wrappers follow the same rule. Protocol crates expose protocol-owned MUX UDP relay objects that implement neutral `zero_core::InboundMuxUdpRelay`, and shared runtime MUX UDP glue consumes that trait instead of unpacking protocol-specific payload source/responder/auth fields inside adapters.
MUX-carried inbound TCP relay wrappers follow the same rule. Protocol crates expose protocol-owned MUX TCP relay objects that implement neutral `zero_core::InboundMuxTcpRelay`, and shared runtime MUX TCP glue consumes that trait instead of keeping proxy-local bridge traits for protocol-owned close/relay behavior.
Protocol-specific tunnel control negotiation also stays in the owning protocol crate. For example, Mieru socks5-in-tunnel CONNECT and UDP ASSOCIATE request/response choreography lives in `protocols/mieru::tunnel`; `zero-proxy` only opens the carrier socket and bridges the resulting protocol-owned stream/session objects.
VMess inbound UDP request payload mode detection/parsing and response packet encoding live in `protocols/vmess::udp`; VLESS inbound UDP packet parsing and response/MUX response encoding live in `protocols/vless::udp`. Proxy inbound glue delegates packet wrapping/parsing to inbound-specific protocol sessions and must not name protocol-private UDP codec, dispatch, packet, response, or response-target models.

Inbound UDP follows the same ownership rule for both datagram and stream-backed protocols.
`crates/proxy/src/inbound/{datagram_udp,stream_udp,mux_udp}.rs` own only route submission, response accounting, task polling, and relay-loop orchestration.
Protocol-specific responders own request decoding, response encoding, protocol session tracking, and read buffers:

- Shadowsocks and Hysteria2 datagram responders keep client/protocol session tracking inside `protocols/shadowsocks` and `protocols/hysteria2`.
- VLESS, VMess, Mieru, and Trojan stream responders keep packet I/O and response framing inside their protocol crates.
- Proxy protocol-named inbound modules may construct a responder and pass it into shared glue, but they must not hold protocol-private pending dispatch state, client maps, codec state, or responder read buffers.

Adding an inbound-capable protocol therefore means adding protocol-owned inbound/UDP responder APIs, thin proxy adapter/listener glue that accepts the carrier and then hands protocol-owned route objects into `runtime::inbound_route::{dispatch_protocol_stream_route, dispatch_protocol_mux_route}` / `serve_inbound()` or the shared UDP relay glue, an adapter capability implementation, and registration in `register.rs`.
Protocol-owned continuation helpers such as `accept_route_owned_with...` stay in `protocols/*`; proxy transport request dispatch may pass closures into those helpers after carrier accept/build completes, but proxy transport bridges do not keep protocol-named accepted-route enums or proxy-local dispatcher traits just to reach shared runtime route glue.
When transport request surfaces need to hand a protocol-owned route into shared runtime glue, `zero-transport` wraps that route in neutral `OpaqueStreamRoute` / `OpaqueMuxRoute` shells from `crates/transport/src/inbound_route.rs` so request interfaces stay transport-owned while runtime orchestration still binds only to `zero_core::{InboundStreamRoute, InboundMuxStreamRoute}`. The same boundary applies inside protocol crates: VLESS and VMess keep raw inbound MUX frame/event enums plus frame-reading helpers protocol-private and expose only semantic session/server shells to transport and runtime glue.
When an inbound protocol needs recorded or metered client streams before route dispatch, that unwrap/accounting logic still belongs under `runtime::inbound_route` rather than the adapter. Adapters may decide carrier/fallback flow, but shared `MeteredStream<RecordingStream<_>>` TCP/UDP/MUX orchestration should be expressed through runtime helpers, not reimplemented per protocol listener.
The shared stream wrappers that make that possible (`RecordingStream`, `MeteredStream`, `StreamTraffic`, and the neutral `zero_core::InboundFallbackCapture` bridge used for VLESS fallback replay over them) belong to `crates/transport`; `zero-proxy` should only wire those neutral wrappers into runtime orchestration.

### 内核管道和入站协议管道

代理运行时围绕一个通用内核管道边界组织：

```text
KernelPipe -> TcpPipe / UdpPipe -> 协议特征 -> 协议 crates
```

`KernelPipe` 是运行时编排边界，不是协议特征。它依赖于运行时状态，如路由、会话、统计、事件、传输设置和任务生命周期，因此它属于 `zero-proxy`。

`TcpPipe` 拥有常规 TCP 流的 TCP 路由执行和出站建立。`UdpPipe` 拥有将 UDP 数据包提交到 UDP 运行时状态机的功能。`UdpDispatch` 仍然是 UDP 管道的内部状态持有者，用于 direct socket、上游关联、协议运行时状态聚合、响应任务、会话核算和 fallback 处理。

协议 crates 不实现 `KernelPipe`。它们实现协议无关的行为特征，用于握手、会话状态、流数据包帧封装或 datagram 帧封装。

### InboundProtocol 特征和 `serve_inbound()` TCP 生命周期

所有 TCP 协议入站处理程序通过单一特征和单一内核入口点统一。

**`InboundProtocol` 特征** -- 协议-服务器边界：

```rust
#[async_trait]
pub trait InboundProtocol: Send + Sync {
    type ClientStream: AsyncRead + AsyncWrite + Unpin + Send;

    async fn send_ok(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn send_blocked(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn send_upstream_failure(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn relay(&self, client: Self::ClientStream, upstream: TcpRelayStream,
                   up_bps: Option<u64>, down_bps: Option<u64>) -> Result<(), EngineError>;
}
```

**协议实现者**（SOCKS5、HTTP CONNECT、VLESS、Hysteria2、Shadowsocks、Trojan、VMess、Mieru）在把 `Session + client stream` 交给 `serve_inbound()` 之后，仅实现此特征。每个处理程序提供：

- `send_ok` -- 通知客户端隧道已建立（协议特定响应）
- `send_blocked` -- 通知客户端请求被阻止（协议特定错误）
- `send_upstream_failure` -- 通知客户端上游不可达
- `relay` -- 双向中继；默认为原始 TCP `io::copy`，支持可选速率限制；可覆盖用于 AEAD 帧封装（Shadowsocks）或 QUIC 流（Hysteria2）中继

**`serve_inbound()`** 是常规 TCP 协议的单一 TCP 入站生命周期入口点。协议处理程序从不直接接触 engine、config 或 resolver。该函数拥有协议无关的 TCP 生命周期工作，并调用 `TcpPipe` 进行路由执行和出站建立：

1. **URL 重写** -- 在路由之前应用 `route.url_rewrite` 规则重写会话目标域名
2. **内核速率限制** -- 应用来自配置的按入站默认值（`up_bps` / `down_bps`）；协议 accept 阶段已设置的按用户限制优先
3. **会话准备** -- `prepare_session`（引擎端元数据）
4. **路由并建立** -- `TcpPipe`（条件匹配 + 出站连接）
5. **协议回复** -- 视情况发送 `send_ok` / `send_blocked` / `send_upstream_failure`
6. **空闲超时** -- 使用 `InboundConfig.idle_timeout_secs`（默认 300s）通过 `tokio::time::timeout` 包裹中继
7. **会话生命周期** -- 用 `SessionOutcome` 跟踪 / 完成，结构化日志

添加新的跨切面 TCP 生命周期能力通常需要更改 `serve_inbound()` 或 `TcpPipe`；协议处理程序保持不变。

### 内核原语：熔断器

`zero-engine` 为每个出站标签维护 `OutboundHealth`。在连接到任何出站之前，TCP 管道的候选建立路径通过 `check_outbound_health()` 检查健康状态。如果在 30 秒滑动窗口内累积 5 次失败，该出站被隔离 60 秒。隔离期满后，允许一个探测连接；成功则清除不健康状态，失败则重置冷却期。

## 协议层

- `zero-core`
- `protocols/*`

`zero-core` 持有通用类型和接口。特定协议位于 `protocols/*` 下。

协议通过 feature gate 编译到 `zero-proxy` 中。核心决策层始终编译；协议和管控面能力选择性编译，避免拉入嵌入式场景不需要的模块。

暴露给 GUI 和管控面消费者的协议能力事实从代理运行时协议清单中为当前二进制填充。`zero-api` 定义传输模型；`zero-engine` 不维护协议矩阵。协议无关的描述符和行为特征位于 `zero-traits::protocol`，因此协议 crates 可以暴露元数据和 TCP/UDP 行为，而无需依赖 API 或代理运行时 crates。每个协议 crate 拥有其 `ProtocolMetadata` 描述符，并在握手语义匹配这些特征的地方实现协议行为特征。`TcpTunnelProtocol` 覆盖流级隧道握手，如 SOCKS5、Trojan 和 VLESS。`TcpSessionProtocol` 覆盖返回协议状态的握手，如 Shadowsocks、VMess 和 Mieru。`DeferredTcpTunnelProtocol` 覆盖立即写入请求并将响应验证推迟到流封装器的握手，如 VLESS Reality 单跳。`UdpRelayProtocol` 覆盖 UDP 中继关联握手，如 SOCKS5 UDP ASSOCIATE。`UdpPacketTunnelProtocol`、`UdpPacketFraming` 和 `UdpPacketStreamFraming` 覆盖 UDP-over-stream 协议。`UdpDatagramFraming` 覆盖在一个 UDP datagram 中承载一个完整协议数据包的协议，如 Shadowsocks UDP。VLESS 使用数据包帧封装处理隧道字节；Trojan 使用流帧封装处理长度前缀的 UDP 数据包；Shadowsocks 使用 datagram 帧封装。Mieru 和 Hysteria2 UDP 通过代理运行时 manager 集成，因为它们的会话状态与加密流或 QUIC 连接管理耦合。协议 crates 在可以干净分离的地方拥有数据包握手和帧封装语义，而代理拥有传输设置、socket 设置、路由、会话生命周期、统计、事件和通用响应任务调度；协议响应编码和会话状态保持在协议 crate 或协议拥有的 responder API 中。

`TcpTunnelProtocol` 仅用于在同一流上返回已建立隧道的协议握手。`TcpSessionProtocol` 用于返回后续中继代码所需状态的协议握手。`DeferredTcpTunnelProtocol` 用于在建立期间消费响应会破坏中继路径所需的流语义的情况。`UdpPacketTunnelProtocol` 用于在已连接流上建立 UDP 数据包隧道，`UdpPacketFraming` 用于每个 datagram 的隧道字节。`UdpPacketStreamFraming` 用于数据包边界属于已连接流格式的协议。`UdpDatagramFraming` 用于直接在 UDP 上传输的协议 datagram。传输设置如 TLS、Reality、WebSocket、gRPC、H2、QUIC 和 HTTPUpgrade 保持在代理/传输层。

## 网络栈层

- `zero-stack`

实现 `TcpStack` / `UdpStack` / `NetworkStack` 特征（定义在 `zero-traits` 中），在原始 IP 数据包和 `AsyncRead + AsyncWrite` 流或 datagram I/O 之间转换。

两种实现共享同一特征：

| 栈 | 策略 | TCP 终止 | 所需驱动 |
|-------|----------|-----------------|---------------|
| `UserNetworkStack` | 用户空间 TCP 状态机 (`UserTcpStack`) | SYN/SYN-ACK/ACK 握手、seq 跟踪、MSS 协商、FIN/RST 处理 | TUN 设备 |
| `SystemStack` | OS 内核 TCP 监听器 (`SystemTcpStack`) | 委托给 OS 内核 | Linux/macOS 无需 |

该栈可插拔：TUN 入站处理程序消费 `NetworkStack`，因此 `UserStack` 与 `SystemStack` 的选择是配置决策；无需代码更改。

### 用户空间 TCP (zero-stack/src/tcp.rs)

`UserTcpStack` 为每个连接维护最小 TCP 状态机：
- **SYN -> SYN-ACK（包含 MSS 选项）**
- **ACK -> Established -> 数据传输**
- **FIN -> ACK -> CloseWait -> FIN-ACK -> 关闭**
- **RST -> 立即拆除**

载荷提取通过 mpsc channel 进入代理管道。响应数据包（SYN-ACK、ACK、FIN）通过出站 channel 发送到 TUN 设备写入任务。

### 系统 TCP (zero-stack/src/system.rs)

`SystemTcpStack` 包装 `tokio::net::TcpListener`；流量必须由 OS 重定向到此监听器：
- Linux：`iptables -t nat REDIRECT`
- macOS：`pf.conf rdr rule`
- Windows：需要 TUN 设备（wintun）或系统代理

## TUN 设备层

- `zero-tun`

平台无关的 `TunDevice` 特征（`AsyncRead + AsyncWrite`），用于虚拟网络接口：

| 平台 | 后端 | 依赖 |
|----------|---------|------------|
| Linux | `/dev/net/tun` ioctl | 内核内置 |
| macOS | utun socket | 内核内置 |
| Windows | Wintun 驱动 | `wintun.dll`（由 GUI/安装器部署） |

在 Windows 上，`wintun.dll` 是平台依赖，就像 Linux 需要 `/dev/net/tun` 和 macOS 需要 utun。内核 crate (`zero-tun`) 通过 `wintun` crate 声明依赖；DLL 到目标系统的部署是 GUI/安装器层的责任。

### TUN 入站 (zero-proxy/src/inbound/tun.rs)

TUN 入站处理程序从 `TunDevice` 读取原始 IP 数据包，喂入 `NetworkStack`，并通过 `serve_inbound()` 分发已建立的 TCP 连接：

```
TunDevice::read() -> 数据包 -> TcpStack::feed()
     ->                             -> 出站写入任务 -> SYN-ACK/ACK/FIN
     
TcpStack::accept() -> UserTcpStream -> serve_inbound()
```

UDP datagram 由内核 UDP 分发路径处理。分发层拥有路由决策、fallback 候选选择、会话生命周期、统计和 UDP 流的事件集成。按协议 UDP 支持由 `capabilities.protocols` 暴露。

出站流按 [`UdpPathCategory`]（Direct、Relay、Datagram、StreamPacket）分类并相应分发。UDP 中继链使用通用 [`UdpPacketPath`] + [`DatagramCodec`] 特征模型：上一跳提供数据包路径（发送/接收原始载荷），下一跳通过该路径编码其协议 datagram。添加新的链组合需要实现这两个特征，而不是创建按协议对模块。

## 传输层

- `zero-transport`

统一传输抽象：TLS、WebSocket、gRPC、H2、HTTPUpgrade、XHTTP（原 SplitHTTP，配置字段 `split_http`，含 `mode`）、QUIC（共享 H3 基座；VLESS 独立 QUIC 传输已被 XTLS 弃用）、Hysteria2 QUIC、VLESS 传输。还包含内核中继路径使用的共享 `RateLimiter`（GCRA）。TLS 客户端指纹（自定义 ClientHello，用于单跳和 relay-stream 最终跳）经 `zero-ztls` 自定义 TLS 1.3 客户端栈实现（从 REALITY 抽取，uTLS 级浏览器指纹匹配）。

## 支撑 Crates

- `zero-api` -- 管控面 API 类型
- `zero-connector` -- 事件分发 connector（JSONL sink、webhook、push）
- `zero-crypto` -- 加密基础设施（Reality TLS 1.3、密钥交换、证书操作）
- `zero-ztls` -- 通用 TLS 1.3 客户端实现，支持自定义 ClientHello（从 REALITY TLS 1.3 栈抽取），供 `zero-transport` 在单跳与 relay-stream 最终跳路径上做 TLS 客户端指纹匹配（uTLS 级浏览器指纹）
- `zero-logging` -- 结构化日志
- `zero-ffi` -- C 兼容嵌入式接口
- `zero-grpc` -- gRPC 管控面适配器（`grpc_api` feature）
- `zero-dns` -- DNS 子系统（system / UDP / DoH / DoT / Fake IP）

## 抽象层

- `zero-traits`

运行时无关的抽象：

| 特征 | 用途 |
|-------|---------|
| `AsyncSocket` / `TcpListener` / `DatagramSocket` | I/O |
| `TcpStack` / `UdpStack` / `NetworkStack` | 网络数据包到流/datagram 的转换 |
| `ProtocolMetadata` / `TcpTunnelProtocol` / `DeferredTcpTunnelProtocol` / `TcpSessionProtocol` / `UdpRelayProtocol` / `UdpPacketTunnelProtocol` / `UdpPacketFraming` / `UdpPacketStreamFraming` / `UdpDatagramFraming` | 协议元数据和出站行为边界 |
| `DnsResolver` / `TlsConnector` / `TlsAcceptor` | 平台服务 |

## 平台层

- `zero-platform-tokio`
- 为其他平台预留的目录

当前仅实现 Tokio 后端。

## 入站协议

所有入站处理程序实现 `InboundProtocol` 并喂入 `serve_inbound()`：

| 处理程序 | 协议 | 备注 |
|---------|----------|-------|
| `socks5` | SOCKS5 | CONNECT + UDP ASSOCIATE |
| `http` | HTTP CONNECT | |
| `mixed` | 自动检测 | 同一端口上的 SOCKS5 TCP CONNECT、SOCKS5 UDP ASSOCIATE 和 HTTP CONNECT TCP |
| `vless` | VLESS | TCP + UDP-over-TCP |
| `hysteria2` | Hysteria2 | QUIC |
| `shadowsocks` | Shadowsocks | AEAD + 2022-blake3 |
| `trojan` | Trojan | TCP + UDP |
| `vmess` | VMess | 实验性 AEAD TCP、TCP/UDP MUX 和 UDP-over-stream 实现；`cipher: auto` 被规范化为当前 AEAD 基线 |
| `mieru` | Mieru | TCP + UDP，通过加密流封装器 |
| `direct` | Direct | 固定目标转发器，无握手 |
| `tun` | TUN | 虚拟网络接口，消费 `NetworkStack` |
| `system` | System | OS 级流量重定向，消费 `SystemTcpStack` |

## 依赖方向

仅自上而下：

- `zero` -> `config`, `engine`, `proxy`, `api`, `connector`（可选）, `grpc`（可选）
- `proxy` -> `engine`, `config`, `protocols/*`, `transport`, `stack`, `tun`, `dns`
- `transport` -> `config`, `core`, `engine`, `ztls`
- `engine` -> `config`, `router`, `core`, `api`
- `stack` -> `traits`
- `tun` -> `traits`
- `protocols/*` -> `core`, `traits`
- `core` -> `traits`

无反向依赖。

### Adapter subdirectory rule

Adapter subdirectories require multiple real sibling responsibilities. Mieru keeps its managed stream-packet flow in `src/adapters/mieru/udp.rs` because it has no sibling packet-path or association module. Hysteria2 and Shadowsocks retain `udp/{flow,packet_path}.rs`, while SOCKS5 retains `udp/{flow,packet_path,upstream_association}.rs`. VLESS, VMess, and Trojan retain adapter-local `listener.rs` because their capability roots and transport listener bridges are separate responsibilities. Transport-owned plans normalize protocol endpoint and resume state into `ManagedDatagramStartPlan` or `ManagedStreamPacketBridgePlan`; proxy adapters add runtime-only context and must not unpack those values into protocol-specific start requests again.
