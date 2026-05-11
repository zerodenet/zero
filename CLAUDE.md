# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Zero is a network proxy written in Rust. The current version (v0.0.2 pre-release) delivers a minimal working TCP proxy with layered architecture, outbound groups, runtime selector switching, VLESS TLS support, and SOCKS5 UDP.

**Current capabilities:**
- `SOCKS5` inbound (no-auth, CONNECT + UDP ASSOCIATE)
- `HTTP CONNECT` inbound
- `mixed` inbound (same port detects both SOCKS5 and HTTP CONNECT)
- `VLESS` TCP / TLS inbound (with WebSocket transport support)
- `direct`, `block`, chained `SOCKS5`, and chained `VLESS` TCP / TLS outbound
- SOCKS5 UDP support (direct, block, and upstream SOCKS5)
- `mode = rule | global | direct` for routing behavior
- Outbound groups: `selector`, `fallback`, `urltest`
- Runtime selector switching via HTTP API
- `group -> group` nesting support
- Static routing based on domain, CIDR, and external `rule_sets` files
- Structured logging and local read-only status export (active sessions, recent completed sessions)

**Known limitations (v0.0.2):**
- No TUN, Shadowsocks, Trojan, or VMess protocols (stubs exist but not implemented)
- JSON-only configuration, no hot reload
- No GeoIP or remote rule sets
- No GUI, subscriptions, or installer

## Version Status

- `v0.0.1`: Minimum proxy foundation (shipped)
- `v0.0.2`: Current pre-release - adding outbound groups (selector, fallback, urltest, group nesting) + VLESS TLS + UDP
- `v0.1.0`: Reserved for first official release (not yet)

## Common Commands

```bash
# Build and run
cargo build
cargo build --release
cargo run -- run examples/v0.0.1/basic.json
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.0.1/basic.json

# Status inspection
cargo run -- status examples/v0.0.1/basic.json
cargo run -- status --json examples/v0.0.1/basic.json

# Development workflow
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo test -p <crate-name>          # Run tests for a single crate
cargo test -- <test-name>           # Run specific test by name
cargo clippy --workspace --all-targets

# Selective compilation (features)
cargo build --no-default-features           # Minimum core only
cargo build --no-default-features --features inbound-socks5,outbound-socks5

# Using Makefile shortcuts
make fmt
make check
make test
make clippy
make build
make release
make run
make run-status
make status
make status-json

# Runtime selector API (when status-listen is enabled)
curl -X POST http://127.0.0.1:9090/selectors/<group-name>/<outbound-tag>
```

## Code Architecture

### Layered Structure (top to bottom)

1. **Application Layer** (`src/`)
   - Root package `zero` - CLI args, config paths, process startup, status output
   - Entry point: `src/main.rs` - parses commands (`run`, `status`, `help`)

2. **Configuration & Execution Layer**
   - `crates/config` (`zero-config`) - configuration models, validation, rule set loading
   - `crates/engine` (`zero-engine`) - decision making, planning, state, sessions, statistics, events (protocol-agnostic)
   - `crates/router` (`zero-router`) - rule matching (domain, ip, and, or conditions)
   - `crates/proxy` (`zero-proxy`) - proxy runtime, listeners, transport, protocol wiring

### Proxy Crate Architecture (`crates/proxy/`)

The proxy crate follows a clean layered architecture with strict separation of concerns:

```
crates/proxy/src/
├── inbound/          # Inbound protocol handlers
│   ├── socks5.rs     # SOCKS5 (CONNECT + UDP ASSOCIATE)
│   ├── vless.rs      # VLESS (TCP, TLS, WebSocket, Reality + UDP tunneling)
│   ├── http_connect.rs
│   ├── mixed.rs
│   └── mod.rs
├── outbound/         # Outbound protocol implementations
│   ├── direct.rs     # Direct outbound (TCP + UDP)
│   ├── socks5.rs     # SOCKS5 outbound (TCP CONNECT + UDP relay)
│   └── vless.rs      # VLESS outbound (TCP + UDP)
├── runtime/           # Core runtime (protocol-agnostic)
│   ├── udp_associate/  # Shared UDP flow management
│   │   ├── sessions.rs  # UDP flow tracking and session management
│   │   ├── context.rs   # Context types for UDP handling
│   │   └── helpers.rs   # Utility functions
│   └── ...
└── transport/         # Low-level I/O abstractions
    ├── meter.rs      # Traffic metering
    ├── tcp_flow.rs   # TCP flow management
    └── ...
```

**Architecture Principles:**
1. **Kernel Separation** - `zero-engine` is completely protocol-agnostic
2. **Inbound/Outbound Split** - Clear separation between accepting connections (inbound) and making outbound connections (outbound)
3. **Protocol-Specific Logic** - Each protocol owns its own UDP handling logic where appropriate
   - SOCKS5: Has its own UDP ASSOCIATE flow
   - VLESS: Has its own UDP tunneling over TCP model
4. **Shared Runtime**: Common UDP flow tracking, session management, and statistics are in `runtime/`

3. **Protocol Layer**
   - `crates/core` (`zero-core`) - common types and domain models
   - `protocols/*` - specific protocol implementations
     - `protocols/socks5` - SOCKS5 protocol (inbound + outbound + UDP)
     - `protocols/http-connect` - HTTP CONNECT protocol
     - `protocols/vless` - VLESS protocol (TCP, TLS, WebSocket, Reality)
     - `protocols/shadowsocks` - stub (not implemented)
     - `protocols/trojan` - stub (not implemented)
     - `protocols/vmess` - stub (not implemented)

4. **Support Crates**
   - `crates/api` (`zero-api`) - HTTP status API types
   - `crates/connector` (`zero-connector`) - event dispatcher connectors (jsonl sink, panel connector)
   - `crates/crypto` (`zero-crypto`) - crypto utilities (Reality, TLS)
   - `crates/web` (`zero-web`) - web utilities (WebSocket)

5. **Abstraction Layer**
   - `crates/traits` (`zero-traits`) - runtime-agnostic abstractions for I/O, DNS, etc.

6. **Platform Layer**
   - `crates/platform/tokio` (`zero-platform-tokio`) - Tokio runtime backend

### Dependency Direction

Only depend downward:
- `zero` → `config`, `engine`, `proxy`, `api`, `connector` (optional)
- `engine` → `config`, `router`, `core`, platform layer
- `proxy` → `protocols/*`, `core`, platform layer
- `protocols/*` → `core`
- `core` → `traits`

### Cargo Features

Core capabilities (always included):
- Config parsing and validation
- Routing
- `EnginePlan` / `EngineState`
- `direct` / `block` outbound semantics
- Status export

Optional protocol features:
- `inbound-socks5`, `inbound-http-connect`, `inbound-mixed`, `inbound-vless`
- `outbound-socks5`, `outbound-vless`
- `status-api` - enable HTTP status endpoint
- `event-dispatcher`, `sink-jsonl`, `panel-connector` - event connectors

Default: `full,status-api` = all protocols + status API

## Configuration Format

JSON-based with three top-level sections:
```json
{
  "inbounds": [],
  "outbounds": [],
  "route": {
    "mode": "rule",
    "rules": [],
    "final": { "type": "direct" }
  }
}
```

**Inbound types:** `socks5`, `http-connect`, `http` (alias), `mixed`, `vless`
**Outbound types:** `direct`, `block`, `socks5`, `vless`, `selector`, `fallback`, `urltest`
**Route modes:** `rule`, `global`, `direct`
**Route conditions:** `domain`, `ip`, `and`, `or`
**Route actions:** `direct`, `reject`, `block` (alias), `route`

## Example Configurations

- `examples/v0.0.1/basic.json` - default mixed inbound at 127.0.0.1:7890
- `examples/v0.0.1/global-selector.json` - selector outbound group
- `examples/v0.0.1/rule-set-files.json` - external rule sets
- `examples/v0.0.1/udp-socks5.json` - SOCKS5 UDP
- `examples/v0.0.2/fallback.json` - fallback group
- `examples/v0.0.2/nested-groups.json` - group->group nesting
- `examples/v0.0.2/urltest.json` - urltest latency-based group
- `examples/v0.0.2/vless.json` - VLESS TCP
- `examples/v0.0.2/vless-tls.json` - VLESS TLS
- `examples/v0.0.2/chained-vless-tls.json` - chained VLESS TLS

## Key Documentation

- `docs/project/config.md` - configuration specification
- `docs/project/modes-and-groups.md` - routing modes and outbound groups
- `docs/project/logging.md` - structured logging
- `docs/project/architecture.md` - architecture layers and dependency rules
- `docs/versions/v0.0.2/README.md` - v0.0.2 release details
