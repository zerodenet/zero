# Architecture

The repository can be understood as the following layers.

## Application Layer

- Root crate `zero`
- `zero-api`

Responsible for CLI arguments, config file paths, process startup, and status output.

The control plane and observability model follow Zero's own conventions. External ecosystems like Clash, sing-box, and Xray are treated as design references only; compatibility shims belong in adapters, gateways, or external tooling and should not constrain the kernel or long-term API.

`zero-api` defines external control, observation, and event export capabilities. It is not synonymous with the HTTP service, nor are capabilities split along transport lines. HTTP/HTTPS, local IPC, file, gRPC, binary framing, Rust API, and FFI all attach to the same core capability set as trait implementations or feature-gated adapters/sinks.

## Configuration, Decision, and State Layer

- `zero-config`
- `zero-engine`
- `zero-router`

`zero-config` owns configuration models and parsing. `zero-router` owns rule matching. `zero-engine` owns compilation of config into executable plans, routing decisions, target resolution, mode and group state, sessions, statistics, events, and state export.

Mode semantics (`direct / global / rule`) and outbound group semantics (`selector / urltest / fallback`) also belong to this layer, not the client.

`zero-engine` is not bound to Tokio, does not start listeners, does not hold protocol implementations, and does not directly establish socket connections. `direct` / `block` are built-in target semantics within this layer; actual network execution is performed by the proxy runtime layer.

`zero-engine` currently operates across decision boundaries:

- `RuntimeConfig`
  - Input from `zero-config`
- `EnginePlan`
  - Immutable execution structures
- `EngineState`
  - Mutable runtime state, including `OutboundHealth` (circuit breaker)
- `view`
  - Tag rendering for `status` / export / logging

The hot path prefers reading plan/state and passing references along borrow boundaries; only the control plane and display surface go back to string tags.

## Proxy Runtime Layer

- `zero-proxy`

`zero-proxy` translates `zero-engine` decisions into real proxy execution:

- Starts inbound listeners
- Invokes protocol implementations for handshake and framing
- Establishes direct or upstream outbound connections
- Runs TCP relay, UDP association, TLS, urltest probing, and circuit breaker health checks
- Validates that the current build has compiled the protocol features referenced by config

This layer may depend on the Tokio backend, protocol crates, and `zero-engine`. It does not re-interpret config semantics or maintain a separate set of mode, group, or route state.

### InboundProtocol trait and `serve_inbound()` kernel pipeline

As of v0.0.4, all TCP protocol inbound handlers are unified through a single trait and a single kernel entry point.

**`InboundProtocol` trait** -- the protocol-server boundary:

```rust
#[async_trait]
pub trait InboundProtocol: Send + Sync {
    type ClientStream: AsyncRead + AsyncWrite + Unpin + Send;

    async fn accept(&self, stream: TcpRelayStream) -> Result<(Session, Self::ClientStream), EngineError>;

    async fn send_ok(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn send_blocked(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn send_upstream_failure(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn relay(&self, client: Self::ClientStream, upstream: TcpRelayStream,
                   up_bps: Option<u64>, down_bps: Option<u64>) -> Result<(), EngineError>;
}
```

**Protocol implementors** (SOCKS5, HTTP CONNECT, VLESS, Hysteria2, Shadowsocks, Trojan, VMess, Mieru) only implement this trait. Each handler provides:

- `accept` -- authenticate and extract the target address into a `Session`
- `send_ok` -- notify the client that the tunnel is established (protocol-specific response)
- `send_blocked` -- notify the client the request was blocked (protocol-specific error)
- `send_upstream_failure` -- notify the client the upstream is unreachable
- `relay` -- bidirectional relay; default is raw TCP `io::copy` with optional rate limiting; override for AEAD-framed (Shadowsocks) or QUIC-stream (Hysteria2) relays

**`serve_inbound()`** is the single kernel entry point for ALL TCP protocols. Protocol handlers never touch the engine, config, or resolver directly. The function owns every protocol-agnostic capability:

1. **URL rewrite** -- applies `route.url_rewrite` rules to rewrite the session target domain before routing
2. **Kernel rate limits** -- applies per-inbound defaults from config (`up_bps` / `down_bps`); per-user limits set during `accept` take priority
3. **Session preparation** -- `prepare_session` (engine-side metadata)
4. **Route and establish** -- `route_and_establish_tcp` (condition matching + outbound connection)
5. **Protocol reply** -- `send_ok` / `send_blocked` / `send_upstream_failure` as appropriate
6. **Idle timeout** -- wraps relay in `tokio::time::timeout` using `InboundConfig.idle_timeout_secs` (default 300s)
7. **Session lifecycle** -- track / finish with `SessionOutcome`, structured logging

Adding a new cross-cutting capability only requires changing `serve_inbound()` -- protocol handlers remain unaffected.

### Kernel primitive: circuit breaker

`zero-engine` maintains `OutboundHealth` per outbound tag. Before connecting to any outbound, `establish_tcp_candidate` checks health via `check_outbound_health()`. If 5 failures accumulate within a 30-second sliding window, the outbound is quarantined for 60 seconds. After quarantine, one probe connection is allowed; success clears the unhealthy state, failure resets the cooldown.

## Protocol Layer

- `zero-core`
- `protocols/*`

`zero-core` holds common types and interfaces. Specific protocols live under `protocols/*`.

Protocols are feature-gated into `zero-proxy`. The core decision layer always compiles; protocols and control plane capabilities are selectively compiled, avoiding pulling modules not needed for embedded scenarios.

## Network Stack Layer

- `zero-stack`

Implements the `TcpStack` / `UdpStack` / `NetworkStack` traits (defined in `zero-traits`) to convert between raw IP packets and `AsyncRead + AsyncWrite` streams or datagram I/O.

Two implementations share the same trait:

| Stack | Strategy | TCP termination | Driver needed |
|-------|----------|-----------------|---------------|
| `UserNetworkStack` | User-space TCP state machine (`UserTcpStack`) | SYN/SYN-ACK/ACK handshake, seq tracking, MSS negotiation, FIN/RST handling | TUN device |
| `SystemStack` | OS kernel TCP listener (`SystemTcpStack`) | Delegated to OS kernel | None on Linux/macOS |

The stack is pluggable: the TUN inbound handler consumes `NetworkStack`, so the choice of `UserStack` vs `SystemStack` is a configuration decision — no code changes needed.

### User-space TCP (zero-stack/src/tcp.rs)

`UserTcpStack` maintains a minimal per-connection TCP state machine:
- **SYN → SYN-ACK (with MSS option)**
- **ACK → Established → data transfer**
- **FIN → ACK → CloseWait → FIN-ACK → closed**
- **RST → immediate teardown**

Payload extraction feeds into the proxy pipeline via mpsc channels. Response packets (SYN-ACK, ACK, FIN) are emitted through an outbound channel → TUN device writer task.

### System TCP (zero-stack/src/system.rs)

`SystemTcpStack` wraps a `tokio::net::TcpListener` — traffic must be redirected to this listener by the OS:
- Linux: `iptables -t nat REDIRECT`
- macOS: `pf.conf rdr rule`
- Windows: requires a TUN device (wintun) or system proxy

## TUN Device Layer

- `zero-tun`

Platform-agnostic `TunDevice` trait (`AsyncRead + AsyncWrite`) for virtual network interfaces:

| Platform | Backend | Dependency |
|----------|---------|------------|
| Linux | `/dev/net/tun` ioctl | Kernel built-in |
| macOS | utun socket | Kernel built-in |
| Windows | Wintun driver | `wintun.dll` (deployed by GUI/installer) |

On Windows, `wintun.dll` is a platform dependency — like how Linux requires `/dev/net/tun` and macOS requires utun. The kernel crate (`zero-tun`) declares the dependency via the `wintun` crate; deployment of the DLL to the target system is the GUI/installer layer's responsibility.

### TUN inbound (zero-proxy/src/inbound/tun.rs)

The TUN inbound handler reads raw IP packets from a `TunDevice`, feeds them to a `NetworkStack`, and dispatches established TCP connections through `serve_inbound()`:

```
TunDevice::read() → packet → TcpStack::feed()
     ↑                              ↓
outbound writer task ← SYN-ACK/ACK/FIN
     
TcpStack::accept() → UserTcpStream → serve_inbound()
```

UDP datagrams are forwarded through a local relay socket with non-blocking response polling.

## Transport Layer

- `zero-transport`

Unified transport abstractions: TLS, WebSocket, gRPC, H2, HTTPUpgrade, QUIC, SplitHTTP, Hysteria2 QUIC, VLESS transport. Also contains the shared `RateLimiter` (GCRA) used by the kernel relay path.

## Support Crates

- `zero-api` -- control plane API types
- `zero-connector` -- event dispatcher connectors (JSONL sink, webhook, push)
- `zero-logging` -- structured logging
- `zero-ffi` -- C-compatible embedded interface
- `zero-grpc` -- gRPC control plane adapter (`grpc-api` feature)
- `zero-dns` -- DNS subsystem (system / UDP / DoH / DoT / Fake IP)

## Abstraction Layer

- `zero-traits`

Runtime-agnostic abstractions:

| Trait | Purpose |
|-------|---------|
| `AsyncSocket` / `TcpListener` / `DatagramSocket` | I/O |
| `TcpStack` / `UdpStack` / `NetworkStack` | Network packet → stream/datagram conversion |
| `DnsResolver` / `TlsConnector` / `TlsAcceptor` | Platform services |

## Platform Layer

- `zero-platform-tokio`
- Reserved directories for other platforms

Currently only the Tokio backend is implemented.

## Inbound Protocols

All inbound handlers implement `InboundProtocol` and feed into `serve_inbound()`:

| Handler | Protocol | Notes |
|---------|----------|-------|
| `socks5` | SOCKS5 | CONNECT + UDP ASSOCIATE |
| `http-connect` | HTTP CONNECT | |
| `mixed` | Auto-detect | SOCKS5 / HTTP CONNECT on one port |
| `vless` | VLESS | TCP + UDP-over-TCP |
| `hysteria2` | Hysteria2 | QUIC |
| `shadowsocks` | Shadowsocks | AEAD + 2022-blake3 |
| `trojan` | Trojan | TCP |
| `vmess` | VMess | Experimental AEAD implementation; not compatible with `cipher: auto` exports yet |
| `mieru` | Mieru | Registered adapter; TCP single-hop outbound uses encrypted stream wrapper; relay-chain hop not supported yet |
| `direct` | Direct | Fixed-target forwarder, no handshake |
| `tun` | TUN | Virtual network interface, consumes `NetworkStack` |
| `system` | System | OS-level traffic redirect, consumes `SystemTcpStack` |

## Dependency Direction

Top-down only:

- `zero` → `config`, `engine`, `proxy`, `api`, `connector` (optional), `grpc` (optional)
- `proxy` → `engine`, `config`, `protocols/*`, `transport`, `stack`, `tun`, `dns`
- `engine` → `config`, `router`, `core`, `api`
- `stack` → `traits`
- `tun` → `traits`
- `protocols/*` → `core`
- `core` → `traits`

No reverse dependencies.
