# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Zero is a network proxy kernel written in Rust at workspace version `0.0.4`.

**Inbound protocols:**
- `SOCKS5` (no-auth, CONNECT + UDP ASSOCIATE)
- `HTTP CONNECT`
- `mixed` (same port auto-detects SOCKS5 / HTTP CONNECT)
- `VLESS` (TCP, TLS, Reality, WebSocket, gRPC, H2, HTTPUpgrade, QUIC, SplitHTTP; MUX + Vision flow + UDP over TCP)
- `Hysteria2` (QUIC, password auth, TCP + UDP)
- `Shadowsocks` (AEAD: aes-128-gcm, aes-256-gcm, chacha20-ietf-poly1305; 2022-blake3; TCP + UDP)
- `Trojan` (TCP)
- `TUN` (virtual network interface, no feature gate, always compiled)
- `direct` (fixed-target forwarder; outbound determined by normal route rules, no feature gate, always compiled)

**Outbound protocols:**
- `direct` / `block`
- `SOCKS5` (TCP CONNECT + UDP relay)
- `VLESS` (all 9 transports, MUX, Vision, UDP over TCP)
- `Hysteria2` (QUIC, TCP + UDP)
- `Shadowsocks` (TCP + UDP)
- `Trojan` (TCP)

**Routing & outbound groups:**
- `mode = rule | global | direct`
- Conditions: `domain`, `domain-keyword`, `ip`, `geoip`, `and`, `or`
- Groups: `selector`, `fallback`, `urltest`, `relay` (chain)
- Group nesting
- Rule sets: local files + URL remote fetch with auto-cache
- Hot reload (rules + groups, no restart)

**Control plane (three channels):**
| Channel | Transport | Auth | Use case |
|---------|-----------|------|----------|
| HTTP | `127.0.0.1:9090` | Bearer Token | Remote debug, web panel |
| IPC | Unix socket / Windows Named Pipe | FS permissions | Local GUI, CLI |
| CLI | IPC auto-discovery | FS permissions | Terminal ops |

- Query: capabilities, health, config, runtime, stats, flows, policies
- Commands: policy select, probe, flow close, config apply
- Events: SSE stream with `Last-Event-ID` resumption; File (JSON-line rotation) / Webhook / Memory / Callback dispatchers
- Hooks: FlowHook trait, IPC external process decision
- Push connector: heartbeat + remote commands for node reporting
- Embedded: `zero-ffi` crate — C-compatible `cdylib` + `staticlib` for Go/Python/mobile

## Common Commands

```bash
# Build
cargo build
cargo build --release
cargo build --features full,status-api          # default feature set

# Test
cargo test --workspace
cargo test -p <crate-name>                     # single crate
cargo test -- <test-name>                      # specific test by name

# Lint
cargo fmt --all
cargo check --workspace
cargo clippy --workspace --all-targets

# Run
cargo run -- run examples/v0.0.1/basic.json
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.0.1/basic.json

# CLI status / control (IPC auto-discovery)
cargo run -- status --json examples/v0.0.1/basic.json
cargo run -- select <policy-tag> <target-tag>
cargo run -- flows
cargo run -- policies
cargo run -- events
cargo run -- reload <config-path>              # hot reload
cargo run -- tun start --addr IP --tag TAG    # start TUN
cargo run -- tun stop                         # stop TUN
cargo run -- tun status                       # TUN status
cargo run -- version

# Runtime selector API (HTTP, when status-listen enabled)
curl -X POST http://127.0.0.1:9090/selectors/<group-name>/<outbound-tag>

# Makefile shortcuts (same commands)
make fmt / check / test / clippy / build / release / run / run-status / status / status-json
```

## Code Architecture

### Layered Structure (top to bottom)

1. **Application Layer** (`src/`)
   - Entry point: `src/main.rs` — CLI parsing, commands (`run`, `status`, `select`, `flows`, `policies`, `events`, `reload`, `tun`, `version`, `help`)
   - `src/cli.rs` — argument parsing
   - `src/ipc/` — IPC client/server, protocol framing, Unix socket + Windows Named Pipe
   - `src/http_adapter.rs` — HTTP status/control endpoint (feature-gated)
   - `src/hooks.rs` — hook system wiring
   - `src/output.rs` — structured status output
   - `src/rule_set_fetch.rs` — remote rule set downloading

2. **Configuration & Execution Layer**
   - `crates/config` (`zero-config`) — configuration models, validation, rule set loading (local + remote)
   - `crates/engine` (`zero-engine`) — decision making, planning, state, sessions, statistics, events (protocol-agnostic)
   - `crates/router` (`zero-router`) — rule matching (domain, domain-keyword, ip, geoip, and, or, rule sets)
   - `crates/proxy` (`zero-proxy`) — proxy runtime, listeners, transport, protocol wiring

3. **Protocol Layer**
   - `crates/core` (`zero-core`) — common types and domain models
   - `protocols/*` — protocol implementations:
     - `protocols/socks5` — SOCKS5 (inbound + outbound + UDP)
     - `protocols/http-connect` — HTTP CONNECT
     - `protocols/vless` — VLESS (Reality TLS 1.3, Vision flow, MUX, 9 transports)
     - `protocols/hysteria2` — Hysteria2 (QUIC, password auth)
     - `protocols/shadowsocks` — Shadowsocks (AEAD, 2022-blake3)
     - `protocols/trojan` — Trojan (TCP)
     - `protocols/vmess` — stub

4. **Transport Layer** (`crates/transport/` — `zero-transport`)
   - Unified transport abstraction: `tls`, `ws`, `grpc`, `h2`, `http_upgrade`, `quic`, `split_http`, `hysteria2_quic`, `vless_transport`

5. **Support Crates**
   - `crates/api` (`zero-api`) — control plane API types
   - `crates/connector` (`zero-connector`) — event dispatcher connectors (jsonl sink, webhook, push)
   - `crates/crypto` (`zero-crypto`) — crypto utilities (Reality, TLS)
   - `crates/logging` (`zero-logging`) — structured logging
   - `crates/web` (`zero-web`) — web utilities (WebSocket)
   - `crates/ffi` (`zero-ffi`) — C-compatible embedded interface

6. **Abstraction Layer**
   - `crates/traits` (`zero-traits`) — runtime-agnostic abstractions for I/O, DNS, etc.

7. **Platform Layer**
   - `crates/platform/tokio` (`zero-platform-tokio`) — Tokio runtime backend

### Dependency Direction (top-down only)

```
zero → config, engine, proxy, api, connector (optional)
engine → config, router, core, platform
proxy → protocols/*, transport, core, platform
transport → core, crypto, web
protocols/* → core
core → traits
```

### Proxy Crate Structure (`crates/proxy/src/`)

```
inbound/          # Protocol handler structs implementing InboundProtocol trait
                  #   socks5, vless, http_connect, mixed, hysteria2, shadowsocks, trojan, direct, tun
                  #   Each provides handshake (accept), client responses (send_ok/send_blocked/send_upstream_failure), and relay
outbound/         # Outbound implementations: direct, socks5, vless, hysteria2, shadowsocks, trojan
runtime/          # Protocol-agnostic runtime
                  #   inbound_protocol.rs — InboundProtocol trait + serve_inbound() unified pipeline entry point
                  #   tcp_outbound.rs — route_and_establish_tcp, establish_tcp_outbound (with circuit breaker)
                  #   udp_helpers.rs / vless_udp.rs — shared UDP types moved from outbound/
                  #   engine_facade.rs, udp_associate.rs / udp_associate/, mux_pool.rs, upstream.rs
transport/        # Low-level I/O
                  #   tcp_relay.rs — RateLimiter (GCRA), RateLimitedWriter, relay_bidirectional_metered_throttled, copy_one_way
                  #   tcp_outbound.rs — data types only: TcpRouteResult, EstablishedTcpOutbound, extract_tcp_stream
                  #   tcp_flow.rs — only is_block_error remains
                  #   metered.rs, stream.rs, direct.rs, tls_hello.rs
```

### v0.0.4 Kernel Primitives

- **Unified TCP pipeline:** All TCP protocols share a single `serve_inbound()` entry point in `runtime/inbound_protocol.rs`. Protocol handlers in `inbound/*.rs` implement the `InboundProtocol` trait:
  - `accept()` — protocol handshake, returns `(Session, ClientStream)`
  - `send_ok()` / `send_blocked()` / `send_upstream_failure()` — client-facing responses
  - `relay()` — bidirectional data forwarding (default: raw TCP relay with rate limiting; overridable for AEAD/QUIC relays)
- **Rate limiting:** GCRA-based `RateLimiter` and `RateLimitedWriter` in `transport/tcp_relay.rs`. `relay_bidirectional_metered_throttled` and `copy_one_way` integrate metering plus throttling without blocking sleeps.
- **Circuit breaker:** `engine/src/outbound_health.rs` — tracks connection failures per outbound tag; quarantines unhealthy outbounds (5 failures in 30s window, 60s cooldown) with probe-based recovery.
- **Idle timeout:** Per-inbound `idle_timeout_secs` enforced in `serve_inbound()` via `tokio::time::timeout` wrapping the relay phase (kernel default: 300s).
- **URL rewrite:** Domain rewriting (`from` exact / `from_regex` pattern → `to` substitution) applied in `apply_url_rewrite()` before routing.
- **Per-user rate limiting:** `Session::apply_auth()` is the single injection point; `SessionAuth` carries per-user `up_bps`/`down_bps` applied during protocol accept.

### Deleted in v0.0.4

- `handle_tcp_session` — replaced by the generic `serve_inbound()`
- `TcpInboundProtocol` enum — no longer needed; protocols implement `InboundProtocol` trait directly
- `runtime/tcp_flow.rs` — deleted; only `is_block_error` moved to `transport/tcp_flow.rs`

### Key Architecture Principles
- **Kernel separation** — `zero-engine` is completely protocol-agnostic
- **Inbound/Outbound split** — clear separation between accepting and making connections
- **Protocol-specific UDP** — each protocol owns its UDP handling (SOCKS5 ASSOCIATE, VLESS UDP-over-TCP, Hysteria2 QUIC datagrams)
- **Shared runtime** — common flow tracking, session management, statistics in `runtime/`

## Cargo Features

Always included: config parsing, routing, `EnginePlan`/`EngineState`, `direct`/`block`, status export.

Optional protocol features:
- `inbound-socks5`, `inbound-http-connect`, `inbound-mixed`, `inbound-vless`, `inbound-hysteria2`, `inbound-shadowsocks`, `inbound-trojan`
- `outbound-socks5`, `outbound-vless`, `outbound-hysteria2`, `outbound-shadowsocks`, `outbound-trojan`
- `status-api` — HTTP status endpoint
- `event-dispatcher`, `sink-jsonl`, `panel-connector` — event connectors

Default: `full,status-api` (all protocols + status API)

## Configuration Format

JSON with three top-level sections: `inbounds`, `outbounds`, `route`.
Route supports `mode` (`rule`/`global`/`direct`), `rules` array, and `final` action.

**Inbound types:** `socks5`, `http-connect`, `http`, `mixed`, `vless`, `hysteria2`, `shadowsocks`, `trojan`, `direct`, `tun`
**Outbound types:** `direct`, `block`, `socks5`, `vless`, `hysteria2`, `shadowsocks`, `trojan`, `selector`, `fallback`, `urltest`, `relay`
**Route conditions:** `domain`, `domain-keyword`, `ip`, `geoip`, `and`, `or`
**Route actions:** `direct`, `reject`, `block` (alias), `route`

## Key Documentation

- `docs/project/config.md` — configuration specification
- `docs/project/modes-and-groups.md` — routing modes and outbound groups
- `docs/project/architecture.md` — architecture layers and dependency rules
- `docs/project/logging.md` — structured logging
- `docs/guides/quickstart.md` — quick start guide
- `docs/guides/gui-integration.md` — GUI/embedding integration guide
- `docs/control-plane-api/README.md` — control plane API reference
- `docs/control-plane/README.md` — control plane design docs
- `docs/versions/README.md` — version index
