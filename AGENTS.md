# Repository Guidelines

## Structure

Root `src/main.rs` builds the `zero` binary. Reusable crates live under `crates/`: `core`, `traits`, `config`, `router`, `engine`, and `platform/tokio`. External protocol implementations live under `protocols/`. Versioned docs and release notes live under `docs/versions/v0.0.1/`. Long-term project notes live under `docs/project/`. Example configs live under `examples/v0.0.1/`.

## Commands

- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- `cargo run -- run examples/v0.0.1/basic.json`
- `cargo run -- status --json examples/v0.0.1/basic.json`

Use workspace commands by default. If you change protocol behavior, config parsing, routing, or runtime wiring, run the full test suite.

## Style

Use `rustfmt` defaults. Keep module and function names in `snake_case`, types in `CamelCase`, package names in `zero-*` form, and directory names short. Prefer ASCII unless the file already uses Unicode. Avoid large source files; around 300 lines is a good point to split code by responsibility.

## Tests

Keep tests in sibling `tests/` directories instead of inline `#[cfg(test)]` blocks unless the test is very local. Name tests by behavior. Update or add tests whenever config shape, protocol handling, routing, or logs change.

## Boundaries

Do not move protocol parsing into the root binary. Keep config ADTs in `crates/config`, routing in `crates/router`, orchestration in `crates/engine`, and concrete protocol code in `protocols/*`. `direct` and `block` stay inside `zero-engine`; they are not standalone protocol crates.

## Docs

When changing config, protocol scope, or release boundaries, update the matching docs in the same change. `docs/project/` is for long-term rules. `docs/versions/v0.0.1/` is for what this version actually ships.
