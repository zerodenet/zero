# Zero Target Architecture

## 1. Purpose

This document defines the target architecture discussed for protocol, carrier, adapter, and runtime ownership. It is a design boundary, not an instruction to perform an open-ended rewrite.

The primary problem is that `zero-transport` currently contains both reusable carrier implementations and protocol-named integration modules such as VLESS, VMess, and Trojan. Moving those modules wholesale into proxy adapters would not solve the ownership problem. Moving them wholesale into protocol crates would also risk duplicating TLS, WebSocket, gRPC, H2, QUIC, relay, and connection lifecycle code for every protocol.

The target therefore separates three concerns:

1. A protocol owns its transport requirements and selection policy.
2. `zero-transport` owns reusable carrier implementations and carrier-stack execution.
3. Proxy adapters map external config/engine types into protocol-owned typed inputs.

## 2. Core Principle

> Protocols choose and describe carriers. `zero-transport` implements and composes carriers. Adapters translate boundary types. Proxy runtime owns lifecycle orchestration.

This means:

- VLESS decides whether a VLESS profile requires raw TCP, TLS, Reality, WebSocket, gRPC, H2, XHTTP, or QUIC.
- VLESS does not implement TLS, WebSocket, gRPC, H2, XHTTP, or QUIC.
- `zero-transport` does not inspect VLESS config or engine leaf variants.
- The VLESS adapter does not decide which carrier stack VLESS should use.
- Generic runtime does not know that the operation belongs to VLESS.

## 3. Target Dependency Direction

```text
zero-traits / zero-core
          |
          v
zero-transport
generic carriers, carrier plans, carrier executor
          |
          v
protocols/* [runtime]
protocol profiles, transport requirements, handshake, codec, session
          |
          v
zero-proxy adapters
config/engine projection and capability registration
          |
          v
zero-proxy runtime
listener, TCP, UDP, relay, accounting, shutdown lifecycle
```

Validation uses a separate feature path:

```text
zero-config
    |
    v
protocols/* [validation only]
```

Required dependency properties:

- `zero-transport` must not depend on protocol crates.
- `zero-transport` must not depend on `zero-config` protocol ADTs.
- Protocol runtime features may depend on `zero-transport` after protocol dependencies have been removed from transport.
- Protocol validation features must not compile protocol data-plane or carrier code.
- `zero-engine` must not depend on concrete protocol crates.
- Proxy is the outer composition layer and may depend on config, engine, protocols, and transport.

## 4. Shared Carrier Model

Protocols must not each implement transport stacks. `zero-transport` should expose a reusable carrier plan and one shared executor.

Illustrative model:

```rust
pub struct CarrierStackPlan {
    pub endpoint: Endpoint,
    pub layers: Vec<CarrierLayer>,
}

pub enum CarrierLayer {
    Tls(ClientTlsProfile),
    Reality(RealityProfile),
    WebSocket(WebSocketProfile),
    Grpc(GrpcProfile),
    H2(H2Profile),
    HttpUpgrade(HttpUpgradeProfile),
    SplitHttp(SplitHttpProfile),
    Quic(QuicProfile),
}

pub enum CarrierBase {
    Dial,
    Existing(TcpRelayStream),
}
```

The shared executor owns carrier construction:

```rust
pub async fn open_carrier(
    plan: CarrierStackPlan,
    base: CarrierBase,
    services: &dyn CarrierRuntimeServices,
) -> Result<TcpRelayStream, TransportError>;
```

`CarrierBase::Existing` allows the same implementation to support transport-over-relay without recreating per-protocol TLS/WS/gRPC wrapping logic.

The exact public types may differ during implementation. The invariant is that carrier composition and execution have one reusable implementation.

## 5. Prepared Protocol Operations

A protocol combines a carrier requirement with a protocol handshake, but does not execute proxy lifecycle itself.

Illustrative outbound model:

```rust
pub struct PreparedProtocolConnect<H> {
    pub carrier: CarrierStackPlan,
    pub handshake: H,
}

pub trait ProtocolHandshake {
    type Output;
    type Error;

    async fn execute(
        self,
        stream: TcpRelayStream,
    ) -> Result<Self::Output, Self::Error>;
}
```

A shared executor can then perform the common sequence:

```text
open carrier stack
    -> execute protocol handshake
    -> normalize established stream/session
    -> return to proxy runtime
```

For inbound traffic the equivalent prepared value contains:

- A carrier listener/accept plan.
- A protocol-owned acceptor or authenticated session factory.
- Protocol response and fallback behavior.
- No listener loop, task spawning, shutdown receiver, or `Proxy` handle.

## 6. Layer Responsibilities

### 6.1 `zero-transport`

Owns:

- TCP, TLS, Reality, WebSocket, gRPC, H2, HTTP Upgrade, split HTTP, and QUIC carrier primitives.
- Carrier stack plans and generic execution.
- Direct dial and wrapping over an existing relay stream.
- Generic stream wrappers, metering primitives, replay buffers, and transport errors.
- Carrier-level pooling only when it is independent of protocol semantics.

Must not own:

- `VlessTransportLeaf`, `VmessTransportLeaf`, or other protocol-named integration roots.
- Parsing or matching `InboundProtocolConfig`.
- Matching `ResolvedLeafOutbound`.
- Protocol identity, authentication, framing, MUX semantics, or UDP resume policy.

### 6.2 `protocols/*`

Owns:

- Protocol-private validation.
- Typed client/server profiles.
- Handshake, authentication, framing, codecs, sessions, and protocol errors.
- Protocol-specific MUX and UDP behavior.
- Decisions about which carrier capabilities are required.
- Construction of carrier plans from protocol-owned typed options.
- Protocol-specific deferred response, fallback, cache, and resume policy.

Must not own:

- Socket listener loops.
- Tokio task fan-out and shutdown orchestration.
- Proxy session accounting.
- Duplicate TLS, WS, gRPC, H2, QUIC, or relay implementations.
- `zero-config` or `zero-engine` union types.

### 6.3 Proxy adapters

An adapter is a boundary mapper, not a protocol runtime.

It may:

- Confirm that a config or engine leaf belongs to the adapter.
- Extract fields from config ADTs or `ResolvedLeafOutbound`.
- Convert common address, tag, path, and source-directory values into protocol-owned typed profiles.
- Map protocol preparation errors into proxy errors.
- Register protocol-produced prepared operations as capabilities.

It must not:

- Select TLS, WS, gRPC, H2, XHTTP, or QUIC for a protocol.
- Interpret UUID, password, cipher, flow, or protocol-private identity semantics.
- Build carrier stacks.
- Execute handshakes.
- Open sockets, spawn tasks, run listeners, or retain `Proxy`.
- Own protocol MUX pools, UDP managers, or session caches.

Example adapter shape:

```rust
impl TcpOutboundCapability for VlessAdapter {
    fn prepare_tcp_connect(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation>, TcpOutboundFailure> {
        let profile = project_vless_client_profile(leaf)?;
        let prepared = profile.prepare_connect()?;
        Ok(Box::new(SharedProtocolConnectOperation::new(prepared)))
    }
}
```

`project_vless_client_profile` performs type conversion only. `prepare_connect` belongs to VLESS and decides the required carrier plan.

### 6.4 Proxy runtime

Owns:

- Listener lifecycle and eager bind behavior.
- TCP/UDP dispatch and relay sequencing.
- Execution of opaque prepared operations.
- Connection task fan-out, shutdown, reload, and cancellation.
- Session lifecycle, accounting, logging, and error normalization.
- Generic packet-path and managed-flow execution.

Must not:

- Match concrete protocol config variants.
- Retain protocol-private state.
- Interpret carrier selection rules.
- Require edits when a new protocol uses existing capability contracts.

## 7. Target Layout

```text
crates/transport/src/
├── carrier_plan.rs
├── carrier_executor.rs
├── tls/
├── reality/
├── ws/
├── grpc/
├── h2/
├── http_upgrade/
├── split_http/
├── quic/
├── stream.rs
└── metered.rs

protocols/vless/src/
├── validation.rs
├── profile.rs
├── inbound/
├── outbound/
├── mux/
├── udp/
└── transport.rs

crates/proxy/src/adapters/vless/
├── mod.rs
├── inbound.rs
└── outbound.rs

crates/proxy/src/runtime/
├── inbound_operation/
├── tcp_dispatch/
├── udp_dispatch/
└── udp_flow/
```

Simple protocols do not need to reproduce this full directory structure. A raw TCP protocol may expose one profile and one handshake and use `CarrierStackPlan::tcp` directly.

## 8. Example Protocol Flows

### Simple HTTP CONNECT

```text
config leaf
  -> HTTP adapter maps fields
  -> HttpClientProfile
  -> raw TCP carrier plan + HTTP CONNECT handshake
  -> shared executor
  -> proxy runtime accounting/relay
```

### VLESS over TLS and WebSocket

```text
config/engine VLESS leaf
  -> VLESS adapter maps typed options
  -> VlessClientProfile
  -> VLESS produces TCP + TLS + WS carrier plan
  -> zero-transport opens the carrier stack
  -> VLESS executes its handshake
  -> proxy runtime owns relay and lifecycle
```

### Trojan over an existing relay stream

```text
relay prefix produces TcpRelayStream
  -> Trojan profile produces TLS carrier plan
  -> zero-transport executes plan with CarrierBase::Existing
  -> Trojan executes handshake
  -> proxy runtime continues relay chain
```

## 9. Adding a New Protocol

A developer should normally change only:

```text
protocols/<name>/
crates/config/                 # ADT and structural validation delegation
crates/proxy/src/adapters/     # boundary mapping
crates/proxy/src/register.rs   # registration
Cargo feature definitions
protocol and integration tests
docs/protocols/
```

The developer should not need to modify:

```text
crates/proxy/src/runtime/tcp_dispatch/
crates/proxy/src/runtime/udp_dispatch/
crates/proxy/src/runtime/udp_flow/
crates/transport/src/tls/
crates/transport/src/ws/
crates/transport/src/quic/
existing protocol modules
```

Expected onboarding sequence:

1. Implement protocol validation, profiles, handshake, codec, and sessions.
2. Express transport requirements using the shared carrier plan.
3. Add config ADTs and delegate private validation to the protocol crate.
4. Add a thin adapter that projects config/engine values into protocol profiles.
5. Register the adapter and feature.
6. Add protocol unit tests, proxy loopback/interop tests, and feature-isolation checks.

If a protocol requires a carrier capability that does not exist, add that capability once to `zero-transport`; all protocols may then reuse it.

## 10. Migration of Current Protocol-Named Transport Modules

Current modules must be decomposed by responsibility rather than moved wholesale.

| Current area | Protocol-owned destination | Shared transport destination | Adapter remainder |
|---|---|---|---|
| `vless_transport` | VLESS profiles, selection policy, deferred response, MUX/UDP decisions | Carrier stack execution and generic carrier profiles | VLESS config/leaf mapping |
| `vmess_transport` | VMess profiles, handshake/MUX/UDP decisions | TLS/WS/gRPC execution | VMess config/leaf mapping |
| `trojan_transport` | Trojan TLS requirements, handshake, UDP resume/cache policy | TLS over dial/existing stream | Trojan config/leaf mapping |
| `socks5_transport` | SOCKS5 authentication and UDP association protocol behavior | Raw TCP/UDP socket carriers | SOCKS5 config/leaf mapping |
| `shadowsocks_transport` | Cipher/session profile and packet codec behavior | Generic datagram carrier and stream wrappers | Shadowsocks config/leaf mapping |
| `mieru_transport` | Mieru tunnel/MUX/UDP policy | Generic carrier execution | Mieru config/leaf mapping |
| `hysteria2_quic` | Hysteria2 authentication and protocol profile | Generic QUIC listener/connection/stream | Hysteria2 config/leaf mapping |

## 11. Migration Stages

### Stage A: Establish the shared carrier contract

- Inventory duplicated carrier-opening paths.
- Define the minimal carrier plan and executor needed by current protocols.
- Cover direct dial and existing relay stream.
- Preserve current protocol APIs while routing one protocol through the shared executor.

Exit criteria:

- One carrier-rich protocol uses the shared executor for TCP direct and relay paths.
- No new generic accept stage or forwarding facade is introduced.
- Existing interop tests pass.

### Stage B: Move protocol decisions to protocol profiles

- Start with VLESS, VMess, and Trojan because their current transport integration is largest.
- Move carrier selection and protocol-specific plan construction into protocol-owned profiles.
- Keep carrier execution in `zero-transport`.
- Reduce adapters to typed mapping and error conversion.

Exit criteria:

- Adapter code contains no carrier selection branches.
- Protocol crates do not duplicate carrier implementations.
- Direct and relay behavior remains covered.

### Stage C: Remove protocol integration from `zero-transport`

- Decompose SOCKS5, Shadowsocks, Mieru, and Hysteria2 integration modules.
- Remove protocol crate dependencies from `zero-transport`.
- Remove config protocol ADT imports from transport.

Exit criteria:

```powershell
rg "InboundProtocolConfig|ResolvedLeafOutbound" crates/transport/src
```

returns zero matches, and `crates/transport/Cargo.toml` has no protocol dependencies.

### Stage D: Make runtime operations fully opaque

- Stop carrying `ResolvedLeafOutbound` through generic runtime models.
- Let registry/adapters project a leaf once into an owned prepared operation.
- Keep runtime sequencing and accounting independent of protocol types.

Exit criteria:

```powershell
rg "ResolvedLeafOutbound" crates/proxy/src/runtime
```

returns zero production-code matches.

## 12. Verification

Each migration stage requires:

- Focused protocol unit and interop tests.
- Direct and relay tests for affected TCP paths.
- UDP/packet-path tests when affected.
- Reload and cache eviction tests when state ownership changes.
- Minimal protocol feature checks.
- `cargo check --workspace`.
- `cargo test --workspace`.
- `cargo clippy --workspace --all-targets`.

Boundary tests should verify responsibilities and dependency directions, not exact file paths.

## 13. Non-goals

- Do not move existing protocol integration modules wholesale into adapters.
- Do not make each protocol implement its own TLS/WS/gRPC/H2/QUIC stack.
- Do not create a new integration crate solely to relocate the same mixed responsibilities.
- Do not introduce another monolithic adapter trait.
- Do not restore a generic protocol accept stage in proxy runtime.
- Do not split modules only to satisfy a line-count target.
- Do not block unrelated product development on completing every migration stage.

## 14. Completion Definition

The target architecture is reached when:

- Protocols own transport requirements and protocol-specific selection policy.
- `zero-transport` owns all reusable carrier implementations and carrier-stack execution.
- Protocol crates reuse carrier execution instead of duplicating it.
- Adapters contain only boundary mapping, registration, and error conversion.
- Runtime executes opaque prepared operations and remains protocol-neutral.
- Adding a protocol that uses existing capabilities does not modify generic runtime or existing protocols.
- Minimal feature builds and full workspace verification pass.

Until these conditions hold, the current architecture should be described as a hardened baseline with known integration ownership debt.
