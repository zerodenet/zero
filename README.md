# Zero

A network proxy kernel written in Rust. It can run as a local gateway, an edge node, or a server, and be driven by external control planes over HTTP, IPC, or CLI.

## Features

**Inbound** — SOCKS5, HTTP CONNECT, mixed (auto-detect SOCKS5 / HTTP), VLESS, Hysteria2, Shadowsocks (AEAD + 2022-blake3), Trojan, mieru, TUN (virtual NIC, IPv4/IPv6 dual-stack), direct.

**Outbound** — direct / block, SOCKS5, VLESS (9 transports, MUX, Vision, UDP-over-TCP), Hysteria2, Shadowsocks, Trojan, mieru.

**Routing** — `rule` / `global` / `direct` modes; conditions (`domain`, `domain_keyword`, `domain_regex`, `ip`, `rule_set`, `geoip`, `sni`, `and`, `or`); outbound groups (`selector`, `fallback`, `url_test`, `relay`, `load_balance`) with nesting; local + URL rule sets with caching; hot reload of rules and groups.

**Control plane** — three channels:
- HTTP (`127.0.0.1:9090`, Bearer auth) for remote panels and debugging.
- IPC (Unix socket / Windows Named Pipe) for local GUI / CLI.
- CLI for terminal operations.

Query capabilities, health, config, runtime, stats, flows; issue commands (policy select, probe, flow close, config apply); subscribe to SSE events with resumption.

**Kernel primitives** — unified TCP/UDP pipeline, GCRA rate limiting, outbound circuit breaker, per-connection idle timeout, per-user rate limiting, URL rewrite.

## Quick start

```shell
# Build (default: all protocols + HTTP control plane)
cargo build --release

# Run a config
cargo run -- run examples/v0.0.1/basic.json

# Run with the HTTP control plane
cargo run -- run --status-listen 127.0.0.1:9090 examples/v0.0.1/basic.json

# CLI status (IPC auto-discovery)
cargo run -- status --json examples/v0.0.1/basic.json
```

## Selective build

Trim the binary by enabling only what you need:

```shell
cargo build --features full,status_api      # everything (default)
cargo build --features vless,status_api      # VLESS + control plane only
```

See `[features]` in [Cargo.toml](Cargo.toml) for the full list. The TUN inbound is always compiled (no feature gate).

## Documentation

- [Quick start](docs/guides/quickstart.md)
- [Configuration](docs/project/config.md)
- [Architecture](docs/project/architecture.md)
- [Control plane API](docs/control-plane-api/README.md)
- [GUI integration](docs/guides/gui-integration.md)

Runnable configuration examples live in [`examples/`](examples/).

## Project layout

```
src/                      CLI + IPC + HTTP control endpoint
crates/config             configuration models, validation, rule sets
crates/engine             protocol-agnostic decision/planning/state
crates/router             rule matching
crates/proxy              proxy runtime: listeners, transport, protocol wiring
crates/transport          unified transport (TLS, WS, gRPC, H2, QUIC, ...)
crates/stack              user-space + system TCP/UDP stacks
crates/tun                TUN device (Linux / macOS / Windows)
protocols/*               per-protocol implementations (socks5, vless, ...)
crates/api, crates/ffi    control-plane API types, C-compatible embedding
```

## License

MPL-2.0 — see [LICENSE](LICENSE).
