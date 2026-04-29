# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Zero is a network proxy written in Rust. The current version (v0.1.0) delivers a minimal working TCP proxy with layered architecture for configuration, execution core, protocol implementations, routing, and platform adaptation.

**Current capabilities:**
- SOCKS5 inbound (no-auth, CONNECT only)
- HTTP CONNECT inbound
- `mixed` inbound (same port detects both SOCKS5 and HTTP CONNECT)
- `direct`, `block`, and chained SOCKS5 outbound
- Static routing based on domain and CIDR rules
- Structured logging and local read-only status export

**Known limitations (v0.1.0):**
- TCP only, no UDP, TUN, Shadowsocks, Trojan, VLESS, or VMess
- JSON-only configuration, no hot reload, no GeoIP or remote rule sets
- No API, GUI, subscriptions, or installer

## Common Commands

```bash
# Build and run
cargo build
cargo build --release
cargo run -- run examples/v0.1.0/basic.json
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.1.0/basic.json

# Status inspection
cargo run -- status examples/v0.1.0/basic.json
cargo run -- status --json examples/v0.1.0/basic.json

# Development workflow
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo test -p <crate-name>          # Run tests for a single crate
cargo test -- <test-name>           # Run specific test by name
cargo clippy --workspace --all-targets

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
```

## Code Architecture

### Layered Structure (top to bottom)

1. **Application Layer** (`src/`)
   - Root package `zero` - handles CLI args, config file paths, process startup, status output
   - Entry point: `src/main.rs` - parses commands (`run`, `status`, `help`)

2. **Configuration & Execution Layer**
   - `crates/config` (`zero-config`) - configuration models and validation
   - `crates/engine` (`zero-engine`) - execution, orchestration, statistics, built-in actions
   - `crates/router` (`zero-router`) - rule matching

3. **Protocol Layer**
   - `crates/core` (`zero-core`) - common types and interfaces
   - `protocols/*` - specific protocol implementations
     - `protocols/socks5` - SOCKS5 protocol
     - `protocols/http-connect` - HTTP CONNECT protocol

4. **Abstraction Layer**
   - `crates/traits` (`zero-traits`) - runtime-agnostic abstractions for I/O, DNS, etc.

5. **Platform Layer**
   - `crates/platform/tokio` (`zero-platform-tokio`) - Tokio runtime backend

### Dependency Direction

Only depend downward:
- `zero` → `config`, `engine`
- `engine` → `config`, `router`, `protocols/*`, platform layer
- `protocols/*` → `core`
- `core` → `traits`

## Configuration Format

JSON-based with three top-level sections:
```json
{
  "inbounds": [],
  "outbounds": [],
  "route": {
    "rules": [],
    "final": { "type": "direct" }
  }
}
```

**Inbound types:** `socks5`, `http-connect`, `http` (alias), `mixed`, `vless`
**Outbound types:** `direct`, `block`, `socks5`, `vless`
**Route conditions:** `domain`, `ip`, `and`, `or`
**Route actions:** `direct`, `reject`, `block` (alias), `route`

### VLESS TLS Configuration

**Inbound TLS:**
```json
"tls": {
  "cert_path": "/path/to/cert.pem",
  "key_path": "/path/to/key.pem",
  "alpn": ["h2", "http/1.1"]
}
```

**Outbound TLS:**
```json
"tls": {
  "server_name": "example.com",
  "disable_sni": false,
  "ca_cert_path": "/path/to/ca.pem",
  "insecure": false,
  "alpn": ["h2"]
}
```

## Example Configurations

- `examples/v0.1.0/basic.json` - default mixed inbound at 127.0.0.1:7890
- `examples/v0.1.0/mixed.json`
- `examples/v0.1.0/blocked-route.json`
- `examples/v0.1.0/chained-socks5.json`

## Key Documentation

- `docs/project/architecture.md` - architecture layers and dependency rules
- `docs/project/config.md` - configuration specification
- `docs/versions/v0.1.0/release-notes.md` - v0.1.0 features
- `docs/versions/v0.1.0/known-limitations.md` - scope boundaries
- `docs/versions/v0.1.0/release-checklist.md` - pre-release validation items
