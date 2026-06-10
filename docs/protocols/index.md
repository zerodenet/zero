# Protocol Overview

This section tracks the protocol implementation surface in the current codebase. It is factual documentation for GUI authors, panel adapters, and kernel integrators. Runtime consumers should still treat `capabilities.protocols` as the machine-readable source of truth.

## Status Terms

| Status | Meaning |
|------|------|
| `supported` | Normal kernel capability with no known protocol-level gap |
| `partial` | Baseline path exists, but interoperability, MUX, special transport, or server/client direction gaps remain |
| `experimental` | Implemented, but not ready for production compatibility assumptions |
| `unsupported` | Not implemented |
| `not_applicable` | The protocol does not define this direction |

## Matrix

| Protocol | Config type | Status | TCP inbound | TCP outbound | UDP inbound | UDP outbound | Tracking |
|------|------|------|-------------|--------------|-------------|--------------|----------|
| SOCKS5 | `socks5` | `supported` | `supported` | `supported` | `supported` | `supported` | [SOCKS5](./socks5.md) |
| HTTP CONNECT | `http_connect` | `supported` | `supported` | `unsupported` | `not_applicable` | `not_applicable` | [HTTP CONNECT](./http-connect.md) |
| Mixed | `mixed` | `supported` | `supported` | `unsupported` | `supported` | `unsupported` | [Mixed](./mixed.md) |
| VLESS | `vless` | `partial` | `supported` | `supported` | `partial` | `partial` | [VLESS](./vless.md) |
| Hysteria2 | `hysteria2` | `partial` | `supported` | `supported` | `partial` | `partial` | [Hysteria2](./hysteria2.md) |
| Shadowsocks | `shadowsocks` | `partial` | `supported` | `supported` | `supported` | `supported` | [Shadowsocks](./shadowsocks.md) |
| Trojan | `trojan` | `partial` | `supported` | `supported` | `partial` | `partial` | [Trojan](./trojan.md) |
| Mieru | `mieru` | `partial` | `experimental` | `partial` | `partial` | `partial` | [Mieru](./mieru.md) |
| VMess | `vmess` | `experimental` | `experimental` | `experimental` | `experimental` | `experimental` | [VMess](./vmess.md) |

## Kernel Actions

| Name | Status | Notes |
|------|------|------|
| `direct` | `supported` | Kernel direct action; TCP and UDP outbound are available |
| `block` | `supported` | Kernel reject action; TCP and UDP routing decisions can use it |
| `tun` | `supported` | Layer-3 virtual interface controlled by CLI/control plane, not static JSON `inbounds` |

## Documentation Layout

Each protocol has its own tracking page under `docs/protocols/`. The per-protocol page owns the implementation facts, configuration notes, validation coverage, and incomplete items for that protocol. Shared pages keep only cross-protocol summaries:

- [Configuration](./configuration.md): common config examples.
- [Incomplete](./incomplete.md): cross-protocol gap index.
- [Protocol Capabilities](../project/protocol-capabilities.md): descriptor and runtime capability model.
