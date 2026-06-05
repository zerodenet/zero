# Repository Guidelines

## Structure

Root `src/main.rs` builds the `zero` binary. Reusable crates live under `crates/`: `api`, `core`, `traits`, `config`, `router`, `engine`, `proxy`, and `platform/tokio`. External protocol implementations live under `protocols/`: `socks5`, `http-connect`, `vless`. Long-term project notes live under `docs/project/`. Example configs live under `examples/`.

## Features & Build

Default build is `--features full,status_api`. Optional features:
- `socks5`, `http_connect`, `mixed`, `vless`
- `hysteria2`, `shadowsocks`, `trojan`, `vmess`, `mieru`, `dns`
- `status_api` enables runtime control endpoint and selector switching
- `event_dispatcher`, `sink_jsonl`, `panel_connector` enable event delivery and panel connector support
- `grpc_api` enables the gRPC control plane adapter

If a config references an uncompiled protocol, it fails early with a clear error.

## Commands

Always use workspace-wide commands by default:
- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test --workspace` (full test suite)
- `cargo clippy --workspace --all-targets`
- `cargo build --release`
- `cargo run -- run <config>` - run proxy with given config
- `cargo run -- status [--json] <config>` - show current status

Run a single test: `cargo test <test_name>`

If you change protocol behavior, config parsing, routing, or runtime wiring, run the full test suite.

## Style

- Use `rustfmt` defaults
- Keep module/function names `snake_case`, types `CamelCase`, packages `zero-*`
- Prefer ASCII, keep directory names short
- Split files around 300 lines

## Tests

- Tests live in sibling `tests/` directories, not inline in logic files
- Name tests after the behavior they test
- Always add/update tests when changing config shape, protocol handling, routing, runtime behavior, or logging

## Boundaries

- Do not move protocol parsing into root binary
- Keep:
  - config ADTs -> `crates/config`
  - routing -> `crates/router`
  - decisions, plan/state, groups, sessions, stats, events -> `crates/engine`
  - proxy orchestration, listeners, transports, protocol wiring -> `crates/proxy`
  - platform abstraction -> `crates/traits` + `crates/platform/tokio`
  - concrete protocol implementations -> `protocols/*`
- `direct` and `block` target semantics stay inside `zero-engine`; socket-level direct execution stays in `zero-proxy`
- `mixed` is an inbound multiplexor, not an external protocol

## Docs

When changing config, protocol scope, or control surface, update matching docs in the same change:
- `docs/project/` for long-term rules
