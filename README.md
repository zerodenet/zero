# Zero

A network proxy kernel written in Rust.

Run it as a local gateway, an edge node, or a server. Combine the protocols you need — SOCKS5, HTTP CONNECT, VLESS, Hysteria2, Shadowsocks, Trojan, mieru, TUN — drive it over HTTP, IPC, or CLI, and control traffic with rule-based routing and outbound groups.

## Quick start

```shell
cargo build --release
cargo run -- run examples/v0.0.1/basic.json
cargo run -- status --json examples/v0.0.1/basic.json
```

## Documentation

- [Quick start](docs/guides/quickstart.md)
- [Configuration](docs/project/config.md)
- [Architecture](docs/project/architecture.md)
- [Control plane API](docs/control-plane-api/README.md)
- [Examples](examples/)

## License

MPL-2.0 — see [LICENSE](LICENSE).
