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

**Protocol implementors** (SOCKS5, HTTP CONNECT, VLESS, Hysteria2, Shadowsocks, Trojan) only implement this trait. Each handler provides:

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

## Transport Layer

- `zero-transport`

Unified transport abstractions: TLS, WebSocket, gRPC, H2, HTTPUpgrade, QUIC, SplitHTTP, Hysteria2 QUIC, VLESS transport. Also contains the shared `RateLimiter` (GCRA) used by the kernel relay path.

## Support Crates

- `zero-api` -- control plane API types
- `zero-connector` -- event dispatcher connectors (JSONL sink, webhook, push)
- `zero-crypto` -- crypto utilities (Reality, TLS)
- `zero-logging` -- structured logging
- `zero-web` -- web utilities (WebSocket)
- `zero-ffi` -- C-compatible embedded interface

## Abstraction Layer

- `zero-traits`

I/O, DNS, and similar abstractions, not bound to a specific runtime.

## Platform Layer

- `zero-platform-tokio`
- Reserved directories for other platforms

Currently only the Tokio backend is implemented.

## Dependency Direction

Top-down only:

- `zero` may depend on `config`, `engine`, `proxy`
- `proxy` may depend on `engine`, `config`, `protocols/*`, platform layer
- `engine` may depend on `config`, `router`, `core`, and `api`
- `protocols/*` may depend on `core`
- `core` may depend on `traits`

No reverse dependencies.
