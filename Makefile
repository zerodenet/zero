.DEFAULT_GOAL := help

CARGO ?= cargo
CONFIG ?= examples/v0.0.1/basic.json
STATUS_LISTEN ?= 127.0.0.1:9090

.PHONY: help fmt check test clippy build release clean run run-status status status-json

help:
	@echo Available targets:
	@echo   make fmt          - cargo fmt --all
	@echo   make check        - cargo check --workspace
	@echo   make test         - cargo test --workspace
	@echo   make clippy       - cargo clippy --workspace --all-targets
	@echo   make build        - cargo build
	@echo   make release      - cargo build --release
	@echo   make clean        - cargo clean
	@echo   make run          - run zero with CONFIG=$(CONFIG)
	@echo   make run-status   - run zero with local status endpoint
	@echo   make status       - print text status for CONFIG=$(CONFIG)
	@echo   make status-json  - print JSON status for CONFIG=$(CONFIG)

fmt:
	$(CARGO) fmt --all

check:
	$(CARGO) check --workspace

test:
	$(CARGO) test --workspace

clippy:
	$(CARGO) clippy --workspace --all-targets

build:
	$(CARGO) build

release:
	$(CARGO) build --release

clean:
	$(CARGO) clean

run:
	$(CARGO) run -- run $(CONFIG)

run-status:
	$(CARGO) run -- run --status-listen $(STATUS_LISTEN) $(CONFIG)

status:
	$(CARGO) run -- status $(CONFIG)

status-json:
	$(CARGO) run -- status --json $(CONFIG)
