# Initial Release Boundary

This document defines the capability boundary for the first stable external
integration surface. It is a planning boundary, not release history.

The release boundary is gate-driven. Work that cannot be mapped to one of these
gates should not enter the release path.

| Gate | Objective | Exit condition |
|------|-----------|----------------|
| Gate 0 | Runtime architecture boundary | TCP and UDP traffic enter through the common kernel pipe model; protocol code does not bypass the runtime boundary |
| Gate 1 | Protocol baseline closure | Protocol capability facts match implemented TCP/UDP baseline behavior and documented limitations |
| Gate 2 | Control-plane integration closure | GUI and panel integrations can discover, validate, observe, and control the current binary through public APIs |
| Gate 3 | Release freeze | Workspace validation, docs, examples, and capability exports are consistent |

## Principle

The kernel first completes the common proxy core, chain proxy orchestration, and
each protocol's baseline TCP/UDP behavior. Protocol-specific extensions,
optional transports, mux modes, advanced fingerprints, and special transport
paths are added after the core boundary is stable.

Core protocol completeness means:

- ordinary TCP/UDP inbound execution enters through the kernel pipe boundary;
- the protocol's configured inbound and outbound directions are wired through
  routing, sessions, stats, logging, and events;
- TCP and UDP are implemented when they are baseline capabilities of that
  protocol;
- config validation fails early when a protocol or transport is not compiled;
- protocol behavior lives in the protocol crate when it is a protocol handshake,
  session state, packet boundary, or packet framing concern;
- proxy runtime code owns transport setup, socket setup, relay orchestration,
  route execution, fallback, health checks, task lifecycle, stats, and events;
- exported capability facts match the runtime behavior of the current binary.

Protocol completeness does not mean:

- every upstream implementation has full interoperability coverage;
- every optional transport, mux mode, fingerprint mode, or protocol extension is
  available;
- every special UDP transport path is implemented;
- VMess blocks the baseline boundary while its compatibility work remains
  experimental.

## Required Core Surface

The initial boundary requires these kernel capabilities to be coherent:

| Area | Boundary |
|------|----------|
| Runtime pipe | The proxy runtime exposes one common orchestration boundary with TCP and UDP pipe implementations |
| Config | Current config schema parses, validates, and rejects uncompiled protocols early |
| Routing | Direct, global, rule, selector, fallback, url_test, and relay groups execute through engine decisions |
| TCP | Inbound accept, outbound connect, relay chains, rate limits, sessions, stats, logging, and events work through the shared runtime path |
| UDP | SOCKS5 UDP associate, direct UDP, UDP-capable outbound protocols, fallback/group dispatch, sessions, stats, and events work through the shared dispatch path |
| Control plane | Queries, commands, event subscription, capabilities, config snapshots, flow views, policy views, and runtime stats use snake_case wire names |
| Docs | Technical docs describe current facts and project policy, not version history |
| Features | Cargo features expose coarse protocol/control-plane choices; referenced uncompiled protocols fail clearly |

## Protocol Boundary

The capability matrix in `protocol-capabilities.md` is the source of truth for
human-readable protocol status. The machine-readable `capabilities` response is
the source of truth for GUI and control-plane consumers.

Baseline protocol behavior is in scope:

| Protocol | Baseline boundary |
|----------|-------------------|
| `direct` | TCP and UDP outbound execution |
| `block` | TCP and UDP rejection semantics |
| `socks5` | TCP CONNECT, UDP ASSOCIATE, inbound and outbound |
| `http_connect` | TCP CONNECT inbound |
| `mixed` | Same-port inbound multiplexing for SOCKS5 TCP CONNECT, SOCKS5 UDP ASSOCIATE, and HTTP CONNECT TCP |
| `vless` | TCP and UDP-over-stream baseline paths over supported transports |
| `trojan` | TCP and UDP-over-stream baseline paths |
| `shadowsocks` | TCP stream and UDP datagram baseline paths |
| `mieru` | TCP stream session and UDP associate baseline paths |
| `hysteria2` | TCP stream and UDP datagram baseline paths over QUIC |
| `vmess` | TCP/UDP baseline compatibility over raw TLS, WS+TLS, and gRPC+TLS; MUX connection pool for TCP and UDP; Xray/sing-box/Mihomo interop validated; kept experimental until mainstream `cipher: zero` compatibility is resolved |

Advanced protocol behavior is not required for the initial boundary:

- every protocol-specific UDP chain transport path;
- Hysteria2 UDP chaining through QUIC;
- VLESS UDP over SplitHTTP or QUIC relay-chain final hops;
- non-VMess UDP MUX gaps;
- full TLS fingerprint passthrough outside the currently implemented paths;
- VMess `cipher: zero` mainstream external compatibility.

## Chain Proxy Boundary

Chain proxy is a kernel composition capability. It should be treated as a core
orchestration path: the previous hop provides a connected stream or packet path,
and the next hop establishes its protocol layer over that path.

TCP chaining is represented as a stream path. UDP chaining is represented as a
packet path. The runtime may cache sockets, streams, associations, and response
bridges, but protocol packet encoding and decoding remains in protocol crates or
neutral protocol traits.

Supported chain paths should be documented as implemented facts. Missing special
transport paths should be represented as limitation codes instead of blocking
the entire protocol baseline.

New chain work should be accepted when it satisfies one of these:

- it completes a baseline capability already represented by config and
  capabilities;
- it removes a false capability claim;
- it creates a reusable kernel boundary that avoids per-protocol pair special
  cases in `zero-proxy`;
- it is required by a concrete integration path and can be described as a
  protocol-neutral kernel primitive.

Current UDP chain work should converge toward a unified packet-path
orchestration boundary. Implemented paths such as `socks5 -> shadowsocks` should
remain tested, but new work should avoid adding one manager per protocol pair.

## Stop Condition

The implementation is ready for the initial external integration boundary when:

- the Gate 0 architecture boundary is closed and no ordinary inbound protocol
  bypasses the TCP/UDP pipe entry points;
- every capability exported by `capabilities` is backed by implementation and
  tests;
- every known gap has a snake_case limitation code;
- core TCP and UDP flows work through in-tree end-to-end tests;
- protocol special abilities are either implemented or explicitly documented as
  future work;
- no external-facing docs describe obsolete names, version history, or behavior
  that the current binary cannot perform.
