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

Mode semantics (`direct / global / rule`) and outbound group semantics (`selector / url_test / fallback`) also belong to this layer, not the client.

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
- Runs TCP relay, UDP association, TLS, url_test probing, and circuit breaker health checks
- Validates that the current build has compiled the protocol features referenced by config

This layer may depend on the Tokio backend, protocol crates, and `zero-engine`. It does not re-interpret config semantics or maintain a separate set of mode, group, or route state.

### InboundProtocol trait and `serve_inbound()` kernel pipeline

All TCP protocol inbound handlers are unified through a single trait and a single kernel entry point.

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

Protocol capability facts exposed to GUI and control-plane consumers are filled
from the proxy runtime protocol inventory for the current binary. `zero-api`
defines the wire model; `zero-engine` does not maintain a protocol matrix.
The neutral descriptor and behavior traits live in `zero-traits::protocol`, so
protocol crates can expose metadata and TCP/UDP behavior without depending on
the API or proxy runtime crates. Each protocol crate owns its
`ProtocolMetadata` descriptor and implements protocol behavior traits where the
handshake semantics fit those traits. `TcpTunnelProtocol` covers stream-level
tunnel handshakes such as SOCKS5, Trojan, VMess, and VLESS. `TcpSessionProtocol`
covers handshakes that return protocol state, such as Shadowsocks and Mieru.
`DeferredTcpTunnelProtocol` covers handshakes that write the request now and
defer response validation to a stream wrapper, such as VLESS Reality
single-hop.
`UdpRelayProtocol` covers UDP relay-association handshakes such as SOCKS5 UDP
ASSOCIATE.
`UdpPacketTunnelProtocol`, `UdpPacketFraming`, and
`UdpPacketStreamFraming` cover UDP-over-stream protocols. `UdpDatagramFraming`
covers protocols that carry one complete protocol packet in one UDP datagram,
such as Shadowsocks UDP. VLESS uses packet framing for tunnel bytes; Trojan
uses stream framing for length-prefixed UDP packets; Shadowsocks uses datagram
framing. Mieru and Hysteria2 UDP are integrated through proxy runtime managers
because their session state is coupled to encrypted stream or QUIC connection
management. Protocol crates own packet handshake and framing semantics where
those semantics can be separated cleanly, while the proxy owns transport setup,
socket setup, routing, session lifecycle, stats, events, and response bridging.

`TcpTunnelProtocol` is only for protocol handshakes that return an established
tunnel over the same stream. `TcpSessionProtocol` is for protocol handshakes
that return state needed by later relay code. `DeferredTcpTunnelProtocol` is
for cases where consuming the response during establishment would break the
stream semantics required by the relay path. `UdpPacketTunnelProtocol` is for
establishing a UDP packet tunnel over a connected stream, and
`UdpPacketFraming` is for per-datagram tunnel bytes. `UdpPacketStreamFraming`
is for protocols whose packet boundary is part of the connected stream format.
`UdpDatagramFraming` is for protocol datagrams transported directly over UDP.
Transport setup such as TLS, Reality, WebSocket, gRPC, H2, QUIC, and
HTTPUpgrade remains in the proxy/transport layer.

## Network Stack Layer

- `zero-stack`

Implements the `TcpStack` / `UdpStack` / `NetworkStack` traits (defined in `zero-traits`) to convert between raw IP packets and `AsyncRead + AsyncWrite` streams or datagram I/O.

Two implementations share the same trait:

| Stack | Strategy | TCP termination | Driver needed |
|-------|----------|-----------------|---------------|
| `UserNetworkStack` | User-space TCP state machine (`UserTcpStack`) | SYN/SYN-ACK/ACK handshake, seq tracking, MSS negotiation, FIN/RST handling | TUN device |
| `SystemStack` | OS kernel TCP listener (`SystemTcpStack`) | Delegated to OS kernel | None on Linux/macOS |

The stack is pluggable: the TUN inbound handler consumes `NetworkStack`, so the choice of `UserStack` vs `SystemStack` is a configuration decision; no code changes are needed.

### User-space TCP (zero-stack/src/tcp.rs)

`UserTcpStack` maintains a minimal per-connection TCP state machine:
- **SYN -> SYN-ACK (with MSS option)**
- **ACK -> Established -> data transfer**
- **FIN -> ACK -> CloseWait -> FIN-ACK -> closed**
- **RST -> immediate teardown**

Payload extraction feeds into the proxy pipeline via mpsc channels. Response packets (SYN-ACK, ACK, FIN) are emitted through an outbound channel to the TUN device writer task.

### System TCP (zero-stack/src/system.rs)

`SystemTcpStack` wraps a `tokio::net::TcpListener`; traffic must be redirected to this listener by the OS:
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

On Windows, `wintun.dll` is a platform dependency, like how Linux requires `/dev/net/tun` and macOS requires utun. The kernel crate (`zero-tun`) declares the dependency via the `wintun` crate; deployment of the DLL to the target system is the GUI/installer layer's responsibility.

### TUN inbound (zero-proxy/src/inbound/tun.rs)

The TUN inbound handler reads raw IP packets from a `TunDevice`, feeds them to a `NetworkStack`, and dispatches established TCP connections through `serve_inbound()`:

```
TunDevice::read() -> packet -> TcpStack::feed()
     ->                             -> outbound writer task -> SYN-ACK/ACK/FIN
     
TcpStack::accept() -> UserTcpStream -> serve_inbound()
```

UDP datagrams are handled by the kernel UDP dispatch path. The dispatch layer
owns route decision, fallback candidate selection, session lifecycle, stats, and
event integration for UDP flows. Per-protocol UDP support is exposed by
`capabilities.protocols`.

Outbound flows are classified by [`UdpPathCategory`] (Direct, Relay, Datagram,
StreamPacket) and dispatched accordingly. UDP relay chains use the generic
[`UdpPacketPath`] + [`DatagramCodec`] trait model: the previous hop provides a
packet path (send/recv raw payloads), and the next hop encodes its protocol
datagram through that path. Adding new chain combinations requires implementing
the two traits, not creating protocol-pair modules.

## Transport Layer

- `zero-transport`

Unified transport abstractions: TLS, WebSocket, gRPC, H2, HTTPUpgrade, QUIC, SplitHTTP, Hysteria2 QUIC, VLESS transport. Also contains the shared `RateLimiter` (GCRA) used by the kernel relay path.

## Support Crates

- `zero-api` -- control plane API types
- `zero-connector` -- event dispatcher connectors (JSONL sink, webhook, push)
- `zero-logging` -- structured logging
- `zero-ffi` -- C-compatible embedded interface
- `zero-grpc` -- gRPC control plane adapter (`grpc_api` feature)
- `zero-dns` -- DNS subsystem (system / UDP / DoH / DoT / Fake IP)

## Abstraction Layer

- `zero-traits`

Runtime-agnostic abstractions:

| Trait | Purpose |
|-------|---------|
| `AsyncSocket` / `TcpListener` / `DatagramSocket` | I/O |
| `TcpStack` / `UdpStack` / `NetworkStack` | Network packet to stream/datagram conversion |
| `ProtocolMetadata` / `TcpTunnelProtocol` / `DeferredTcpTunnelProtocol` / `TcpSessionProtocol` / `UdpRelayProtocol` / `UdpPacketTunnelProtocol` / `UdpPacketFraming` / `UdpPacketStreamFraming` / `UdpDatagramFraming` | Protocol metadata and outbound behavior boundaries |
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
| `http_connect` | HTTP CONNECT | |
| `mixed` | Auto-detect | SOCKS5 / HTTP CONNECT on one port |
| `vless` | VLESS | TCP + UDP-over-TCP |
| `hysteria2` | Hysteria2 | QUIC |
| `shadowsocks` | Shadowsocks | AEAD + 2022-blake3 |
| `trojan` | Trojan | TCP + UDP |
| `vmess` | VMess | Experimental AEAD implementation; not compatible with `cipher: auto` exports yet |
| `mieru` | Mieru | TCP + UDP over encrypted stream wrapper |
| `direct` | Direct | Fixed-target forwarder, no handshake |
| `tun` | TUN | Virtual network interface, consumes `NetworkStack` |
| `system` | System | OS-level traffic redirect, consumes `SystemTcpStack` |

## Dependency Direction

Top-down only:

- `zero` -> `config`, `engine`, `proxy`, `api`, `connector` (optional), `grpc` (optional)
- `proxy` -> `engine`, `config`, `protocols/*`, `transport`, `stack`, `tun`, `dns`
- `engine` -> `config`, `router`, `core`, `api`
- `stack` -> `traits`
- `tun` -> `traits`
- `protocols/*` -> `core`
- `core` -> `traits`

No reverse dependencies.
