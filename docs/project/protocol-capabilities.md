# Protocol Capabilities

This document records the current protocol capability surface exposed by the
kernel. It describes implementation facts, not release history.

External integrations should prefer the machine-readable `capabilities`
response. The tables here explain the same model for humans.

`zero-api` defines the wire model for protocol capabilities. The concrete
protocol facts are populated by the proxy runtime protocol inventory for the
current binary. `zero-engine` reports generic control-plane capabilities and
does not own the protocol capability matrix.

`zero-traits::protocol` defines the neutral descriptor and behavior boundary:
`ProtocolCapabilityDescriptor`, `ProtocolMetadata`, `TcpTunnelProtocol`,
`DeferredTcpTunnelProtocol`, `TcpSessionProtocol`, `UdpRelayProtocol`,
`UdpPacketTunnelProtocol`, `UdpPacketFraming`, `UdpPacketStreamFraming`, and
`UdpDatagramFraming`. Each protocol crate exposes its own
`ProtocolMetadata` implementation (e.g. `socks5::Socks5Protocol`,
`trojan::TrojanProtocol`) and the proxy adapter delegates to it. Protocol
crates implement behavior traits to hide handshake, session-state, framing,
stream packet boundaries, datagram packet boundaries, and association details
behind neutral traits.

## Capability Fields

Each protocol capability uses snake_case field names and values:

| Field | Meaning |
|------|---------|
| `protocol` | Protocol or kernel action name used in config and status exports |
| `feature` | Cargo feature required to compile the protocol, or `core` |
| `compiled` | Whether this binary has the feature compiled in |
| `status` | Overall support level: `supported`, `partial`, or `experimental` |
| `compatibility_baseline` | Upstream protocol document or implementation family used as the baseline |
| `inbound.tcp` / `inbound.udp` | Whether the protocol can accept TCP or UDP flows |
| `outbound.tcp` / `outbound.udp` | Whether the protocol can create TCP or UDP upstream flows |
| `transports` | Transport names supported by the protocol adapter |
| `mux` | MUX support state |
| `limitations` | Machine-readable limitation codes |

`CapabilityState.level` values are:

| Level | Meaning |
|------|---------|
| `supported` | Implemented as a normal kernel capability |
| `partial` | Implemented, with documented gaps or incomplete interoperability coverage |
| `experimental` | Present in code but not production-compatible enough for default assumptions |
| `unsupported` | Not implemented for this direction/network |
| `not_applicable` | The protocol does not define this direction/network |

## Current Matrix

| Protocol | Status | Inbound TCP | Inbound UDP | Outbound TCP | Outbound UDP | MUX | Baseline |
|---------|--------|-------------|-------------|--------------|--------------|-----|----------|
| `direct` | `supported` | `supported` | `unsupported` | `supported` | `supported` | `not_applicable` | `kernel_builtin` |
| `block` | `supported` | `unsupported` | `unsupported` | `supported` | `supported` | `not_applicable` | `kernel_builtin` |
| `socks5` | `supported` | `supported` | `supported` | `supported` | `supported` | `not_applicable` | `rfc_1928_rfc_1929` |
| `http_connect` | `supported` | `supported` | `not_applicable` | `unsupported` | `not_applicable` | `not_applicable` | `rfc_7231_connect` |
| `mixed` | `supported` | `supported` | `supported` | `unsupported` | `unsupported` | `not_applicable` | `kernel_builtin` |
| `vless` | `partial` | `supported` | `partial` | `supported` | `partial` | `partial` | `xray_core_vless` |
| `hysteria2` | `partial` | `supported` | `partial` | `supported` | `partial` | `not_applicable` | `hysteria` |
| `shadowsocks` | `partial` | `supported` | `partial` | `supported` | `partial` | `not_applicable` | `shadowsocks_rust_sip022` |
| `trojan` | `partial` | `supported` | `partial` | `supported` | `partial` | `unsupported` | `trojan_go` |
| `vmess` | `experimental` | `experimental` | `unsupported` | `experimental` | `unsupported` | `unsupported` | `xray_core_vmess_aead` |
| `mieru` | `partial` | `experimental` | `partial` | `partial` | `partial` | `not_applicable` | `mieru` |

## Kernel Gaps

The main protocol gaps are:

- `udp_relay_chain_final_transport_limited`: VLESS UDP relay chains support a TCP relay prefix and VLESS final hops that can wrap an already established TCP relay stream: raw TCP, TLS, Reality, WebSocket, gRPC, H2, and HTTP Upgrade. SplitHTTP needs an additional TCP connection and QUIC needs a non-TCP carrier, so they are not supported as UDP relay-chain final-hop transports. TLS `client_fingerprint` is not supported over relay streams because that path depends on raw socket handshake control.
- `udp_relay_chain_tcp_prefix_is_not_supported`: the protocol's UDP carrier does not fit the current TCP relay-prefix model. Shadowsocks UDP is datagram-over-UDP and Hysteria2 UDP is QUIC/UDP-carried, so they need a UDP-carrier relay-prefix model before they can be represented as UDP relay-chain final hops.
- `external_interop_coverage_is_incomplete`: in-tree packet handling exists, but end-to-end tests against the baseline upstream implementation are not complete enough to call the path production-compatible.
- `relay_stream_tls_client_fingerprint_is_not_supported`: relay-chain final-hop TLS can run over an already established TCP stream, but custom TLS fingerprint handshakes that require raw socket control are not supported on that path.
- `mux_udp_is_not_implemented`: VLESS MUX handles TCP sub-connections; UDP MUX sub-connections are not implemented.
- `non_reality_tls_fingerprint_passthrough_is_incomplete`: the non-Reality VLESS TLS path does not fully pass fingerprint cipher suite and key-exchange preferences into the TLS implementation.
- `vmess_udp_is_not_implemented`: VMess UDP is not implemented.
- `cipher_auto_is_not_supported`: VMess `cipher: auto` exports are rejected.

## Current Landing State

1. Runtime `capabilities.protocols` is the external source of truth for GUI and control-plane consumers.
2. `zero-api` owns only the wire model. It does not own protocol facts or protocol behavior.
3. `zero-engine` exposes generic control-plane capabilities. It does not maintain the protocol capability matrix.
4. External protocol descriptors live in their protocol crates:
   - `socks5::Socks5Protocol`
   - `http_connect::HttpConnectProtocol`
   - `vless::VlessProtocol`
   - `hysteria2::Hysteria2Protocol`
   - `shadowsocks::ShadowsocksProtocol`
   - `trojan::TrojanProtocol`
   - `vmess::VmessProtocol`
   - `mieru::MieruProtocol`
5. Kernel actions remain in `zero-proxy` descriptor mapping because they are not external protocol crates: `direct`, `block`, and `mixed`.
6. `TcpTunnelProtocol` is implemented for protocol handshakes that only need to establish a tunnel over an already connected stream:
   - SOCKS5 TCP CONNECT
   - Trojan TCP request
   - VMess TCP request/response
   - VLESS non-flow TCP request/response
   - VLESS flow TCP request/response (Vision/Reality, when `reality` feature enabled)
7. `TcpSessionProtocol` is implemented for protocol handshakes that return session/stream state:
   - Shadowsocks TCP (returns `ShadowsocksOutboundSession` with AEAD key/nonce/cipher)
   - Mieru TCP (returns `MieruOutbound` with encryption state)
8. `DeferredTcpTunnelProtocol` is implemented for protocol handshakes that must write a request now and defer response validation to a stream wrapper:
   - VLESS flow TCP request for the Reality single-hop path. The proxy still owns transport setup, metering, and wrapping the connected stream with `DeferredVlessResponseStream`.
9. `UdpRelayProtocol` is implemented for SOCKS5 UDP ASSOCIATE. The protocol crate owns authentication negotiation and association response parsing; the proxy owns control-stream dialing, UDP socket binding, relay endpoint resolution, association cache, idle timeout, stats, events, and fallback behavior.
10. `UdpPacketTunnelProtocol` and `UdpPacketFraming` are implemented for VLESS UDP over an established stream. The VLESS crate owns the UDP tunnel request/response handshake and VLESS UDP packet encoding/decoding; the proxy owns transport setup, relay-prefix setup, routing, fallback, session lifecycle, stats, events, and response bridging. UDP relay chains are implemented for a TCP relay prefix with VLESS final-hop transports that can operate over an already established TCP stream.
11. `UdpPacketTunnelProtocol` and `UdpPacketStreamFraming` are implemented for Trojan UDP over an established TLS stream. The Trojan crate owns the `CMD_UDP` request and length-prefixed UDP packet read/write behavior; the proxy owns TLS setup, relay-prefix setup, upstream caching, task scheduling, routing, fallback, session lifecycle, stats, events, and response bridging. UDP relay chains are implemented for a TCP relay prefix with a Trojan TLS final hop.
12. `UdpDatagramFraming` is implemented for Shadowsocks UDP datagrams. The Shadowsocks crate owns target-data encoding, salt generation, AEAD/2022 KDF selection, UDP encryption, UDP decryption, and target-data parsing. The proxy owns UDP sockets, upstream cache, response matching, routing, fallback, session lifecycle, stats, events, and response bridging.
13. Hysteria2 uses transport-specific connectors from the proxy runtime because QUIC connection setup is integrated with protocol negotiation and does not decompose into a stream-level handshake. TCP streams and UDP datagrams are both integrated with the common proxy routing, fallback, session lifecycle, stats, events, and response-bridging path.
14. Mieru TCP uses `TcpSessionProtocol` for encrypted stream session setup. In TCP relay chains, Mieru can be an intermediate hop because the proxy runs the Mieru session handshake and wraps the relay stream in `MieruTcpStream` before applying the next hop. Mieru UDP is integrated in the proxy UDP dispatch path over the encrypted Mieru stream; the protocol crate owns Mieru segment encryption/decryption state and UDP associate wrapping, while the proxy owns routing, relay-prefix setup, upstream caching, task scheduling, stats, events, and response bridging. UDP relay chains are implemented for a TCP relay prefix with Mieru as the final hop.

## Remaining Work

1. Keep runtime dispatch focused on routing, lifecycle, stats, event export, fallback, health checks, and backpressure.
2. Define a UDP-carrier relay-prefix model before representing Shadowsocks UDP or Hysteria2 UDP as relay-chain final hops.
3. Move Mieru and Hysteria2 UDP packet/association behavior behind neutral protocol traits where the protocol shape fits without moving transport setup into protocol crates.
4. Add upstream interoperability tests for the compatibility baselines before raising any `partial` protocol to `supported`.
5. Treat VMess as a separate compatibility project: complete AEAD TCP validation, `cipher_auto`, UDP, MUX, and upstream interoperability before changing its status.
