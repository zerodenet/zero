# Build Features

Zero uses Cargo features to control which capability subsets are included in the compiled binary, allowing binary size and dependency surface to be trimmed on demand.

## Presets

| Preset | Includes | Use case |
|--------|---------|----------|
| `default` | `full` + `status-api` | Client-side local use |
| `full` | All inbound/outbound protocols + DNS UDP | Full proxy node |

```bash
# Default build (client scenario, no connectors needed)
cargo build --release

# Equivalent
cargo build --release --features full,status-api
```

## Inbound Protocols

Each inbound protocol is independently feature-gated and may be trimmed as needed.

| Feature | Protocol | Extra dependencies |
|---------|------|----------|
| `socks5` | SOCKS5 inbound | -- |
| `http-connect` | HTTP CONNECT inbound | -- |
| `mixed` | Mixed inbound (same port SOCKS5 + HTTP CONNECT) | Implies `socks5` + `http-connect` |
| `vless` | VLESS inbound | TLS / Reality / WebSocket / gRPC / H2 / QUIC transport |
| `hysteria2` | Hysteria2 inbound | QUIC (quinn) |
| `shadowsocks` | Shadowsocks inbound | AEAD encryption + 2022-blake3 |
| `trojan` | Trojan inbound | TLS |
| `vmess` | VMess inbound | Experimental AEAD implementation |
| `mieru` | Mieru inbound | XChaCha20-Poly1305 session framing |
| -- | `direct` inbound | Always compiled, no feature gate required (fixed-target forwarder) |
| -- | `tun` inbound | Always compiled, no feature gate required (virtual network interface: Linux ioctl, macOS utun socket, Windows Wintun) |

```bash
# Trim example: SOCKS5 + HTTP CONNECT only
cargo build --release --no-default-features \
  --features socks5,http-connect,status-api
```

## Outbound Protocols

| Feature | Protocol | Extra dependencies |
|---------|------|----------|
| `socks5` | SOCKS5 outbound | -- |
| `vless` | VLESS outbound | Same transport stack as inbound |
| `hysteria2` | Hysteria2 outbound | QUIC (quinn) |
| `shadowsocks` | Shadowsocks outbound | Same encryption as inbound |
| `trojan` | Trojan outbound | TLS |
| `vmess` | VMess outbound | Experimental AEAD implementation; `cipher: auto` is not supported yet |
| `mieru` | Mieru outbound | Single-hop TCP routing; relay-chain hop is not supported yet |

`direct` and `block` outbounds are always available, no feature gate required -- they need no protocol implementation.

## DNS

| Feature | Description |
|---------|------|
| `dns-udp` | UDP DNS server backend (for self-hosted DNS resolution) |

> When `dns-udp` is not enabled, DNS falls back to the system resolver (`tokio::net::lookup_host`). DNS caching and Fake IP are always available, no feature gate required.

## Control Plane (Server Deployment)

The following features deploy Zero as a server/panel node and are **not in the default `full` preset**.

| Feature | Description | Implies |
|---------|------|------|
| `status-api` | HTTP status API (`/api/v1/*`) | -- |
| `grpc-api` | gRPC control plane endpoint | `dep:zero-grpc` |
| `event-dispatcher` | Event dispatcher: delivers zero events to external sinks | `dep:zero-connector` |
| `sink-jsonl` | JSON Lines file sink (event persistence) | `event-dispatcher` |
| `panel-connector` | Panel connector: heartbeat + remote commands, node reporting | `status-api` + `event-dispatcher` |

```bash
# Server build (with panel connector)
cargo build --release --features full,status-api,panel-connector
```

**`panel-connector` dependency surface:**

- `status-api` -- HTTP control endpoint
- `event-dispatcher` -- event delivery infrastructure
- `zero-connector` crate -- PushConnector (heartbeat/command polling), EventDispatcher (event distribution), Webhook sink

## Client vs Server

```
Client scenario:  full + status-api  (default)
                  ├─ Inbound/outbound protocols
                  ├─ DNS UDP
                  └─ HTTP status endpoint (local debugging)

Server scenario:  + panel-connector
                  ├─ Event dispatch (→ webhook / jsonl)
                  └─ Panel heartbeat reporting + remote commands
```

## Relation to Protocol Implementations

Protocol crates are compiled through the root Cargo features listed above. Protocol presence in the workspace does not by itself mean production compatibility with every external ecosystem export.

| Protocol | Feature | Notes |
|------|---------|------|
| VMess | `vmess` | Experimental AEAD implementation. `cipher: auto` from Xray/Clash exports is not supported yet |
| Mieru | `mieru` | Registered adapter with single-hop TCP outbound support. Relay-chain hop support is not implemented |
| HTTP CONNECT outbound | -- | Outbound direction not implemented |

Asymmetric inbound/outbound features are normal -- some protocols do not need the opposite direction.

## Binary Size Reference

| Features | Binary size (release, stripped) |
|----------|-------------------------------|
| `default` (`full` + `status-api`) | ~15 MB |
| `--no-default-features` + SOCKS5 inbound/outbound + direct | ~5 MB |

---

# Kernel Primitives (v0.0.4)

These cross-cutting capabilities live in the kernel pipeline and apply to all TCP protocols uniformly.

## Idle Timeout

Every TCP relay is wrapped in an idle timeout. If no data flows in either direction for the configured duration, the session is cleanly terminated.

- **Default**: 300 seconds (5 minutes)
- **Config**: `InboundConfig.idle_timeout_secs` (optional, per-inbound)
- **Scope**: Applied in `serve_inbound()` via `tokio::time::timeout` around `protocol.relay()`
- **Behavior**: Idle timeout is not an error -- the session finishes with its current outcome (`DirectRelayed` or `ChainedRelayed`)

## Outbound Health / Circuit Breaker

`zero-engine` maintains an `OutboundHealth` tracker per outbound tag. Before connecting to any outbound (except `direct` and `block`), the kernel checks whether the outbound is healthy.

- **Failure threshold**: 5 failures within a 30-second sliding window
- **Quarantine duration**: 60 seconds -- the outbound is skipped for all new connections
- **Probe**: After quarantine expires, one connection is allowed as a probe; success restores health, failure resets the cooldown
- **Tracking**: `record_outbound_failure()` on connection errors, `record_outbound_success()` on relay completion
- **Scope**: Applies to fallback group candidate selection and all chained outbound connections
- **Error type**: `EngineError::UnhealthyOutbound { tag }` -- treated as a connection failure, triggering the next fallback candidate

## URL Domain Rewrite

Domain-based URL rewriting applied before routing. Rules are matched first-match-wins; once a rule fires, no further rules are evaluated.

- **Config**: `route.url_rewrite` (array of `UrlRewriteRule`)
- **Match types**:
  - `from` -- exact domain match
  - `from_regex` -- regex pattern match with capture group substitution (`$1`, `$2`, etc.)
- **Replacement**: `to` field specifies the replacement domain
- **HTTP redirect**: `status_code` field (e.g. `302`) triggers an HTTP redirect response for HTTP CONNECT; non-HTTP protocols silently rewrite
- **Scope**: Applied in `serve_inbound()` before route lookup; also applied in HTTP CONNECT's own handler for immediate redirects

```json
{
  "route": {
    "url_rewrite": [
      { "from": "old.example.com", "to": "new.example.com" },
      { "from_regex": "^(.+)\\.mirror\\.example\\.com$", "to": "$1.example.com" },
      { "from": "temp.example.com", "to": "permanent.example.com", "status_code": 301 }
    ]
  }
}
```

## Domain-Regex Router Condition

New route condition type `domain-regex` matches the target domain against one or more regex patterns.

- **Config**: `{ "type": "domain-regex", "values": ["^.*\\.google\\..*$", "^.*\\.youtube\\..*$"] }`
- **Matching**: Patterns are compiled once at startup (`regex::Regex`), then matched against the target domain at decision time
- **Capture groups**: Not used for routing -- purely for matching. Use `url_rewrite.from_regex` for capture-based rewriting
- **Scope**: Part of the rule condition system, composable with `and`/`or`

```json
{
  "condition": { "type": "domain-regex", "values": ["^.*\\.google\\..*$"] },
  "action": { "type": "route", "outbound": "proxy" }
}
```

## GCRA Rate Limiting

Token-bucket rate limiting using the Generic Cell Rate Algorithm (GCRA). Limits per-byte throughput on TCP relay streams.

- **Config**: Per-inbound `up_bps` and `down_bps` on `InboundProtocolConfig` (Hysteria2, Shadowsocks, Trojan)
- **Per-user**: Protocol `accept` handlers can set per-user limits (e.g. SOCKS5 via `AuthHandler::rate_limit_for()`); per-user limits take priority over per-inbound defaults
- **Kernel integration**: `apply_kernel_rate_limits()` in `serve_inbound()` fills in defaults for sessions where per-user limits were not set
- **Transport**: `RateLimiter` wraps `AsyncWrite` in `tcp_relay.rs`; non-blocking -- integrates via `poll_write`
- **Burst tolerance**: 16 KB headroom per stream to avoid starving small writes
- **Scope**: Applied during `protocol.relay()` in the bidirectional relay path

```json
{
  "tag": "hysteria2-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "hysteria2",
    "password": "secret",
    "up_bps": 10485760,
    "down_bps": 52428800
  }
}
```

## TUN (Virtual Network Interface)

TUN creates a virtual network interface that captures IP packets at Layer 3 and routes them through the proxy kernel. Always compiled, no feature gate required.

### Architecture

```
TunDevice (zero-tun)          → platform backends (Linux ioctl, macOS utun, Windows Wintun)
    ↓
NetworkStack (zero-traits)    → TcpStack / UdpStack traits
    ↓
UserTcpStack (zero-stack)     → user-space TCP state machine (SYN→SYN-ACK→ACK→data→FIN)
    ↓
TUN inbound (zero-proxy)      → tokio::select!{ read packets → feed stack → accept → serve_inbound() }
```

### Network Stack Trait

`zero-traits` defines `TcpStack` / `UdpStack` / `NetworkStack` — the boundary between raw IP packets and connection-oriented I/O. Two implementations:

| Implementation | Strategy | Driver |
|---------------|----------|--------|
| `UserNetworkStack` | User-space TCP state machine (SYN→Established→CloseWait, MSS option, seq/ack tracking) | TUN device required |
| `SystemStack` | OS TCP listener (iptables/pf redirect → accept TcpStream) | None on Linux/macOS |

The stack is pluggable via the trait — switching implementations requires zero changes to the inbound handler.

### TCP State Machine (UserTcpStack)

- **SYN** → SYN-ACK with MSS option → stored in SynReceived state
- **ACK** → transition to Established → available via `TcpStack::accept()`
- **Data** → payload extracted, forwarded to proxy via channel, ACK sent
- **FIN** → ACK sent, transition to CloseWait → proxy shutdown triggers our FIN
- **RST** → immediate teardown

### Platform Support

| Platform | Backend | Dependency | Provided by |
|----------|---------|------------|-------------|
| Linux | `/dev/net/tun` ioctl | Kernel built-in | OS |
| macOS | utun socket | Kernel built-in | OS |
| Windows | Wintun driver | `wintun.dll` | GUI / installer |

On Windows, `wintun.dll` is a platform resource like `/dev/net/tun` on Linux — it must be present on the target system, but the kernel only *declares* the dependency (via the `wintun` crate), it does not manage DLL lifecycle.

### CLI Commands

```bash
zero tun start --addr 10.0.0.1 --tag proxy    # start TUN
zero tun stop                                  # stop TUN
zero tun status                                # check status
```

Commands are routed through IPC (`ProxyHandle` intercepts TUN commands before they reach the engine).
