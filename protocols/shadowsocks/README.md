# Shadowsocks

This crate owns the Shadowsocks protocol semantics used by Zero. The proxy
runtime owns orchestration, sockets, routing, sessions, stats, events, and
response bridging.

## Capability

| Area | Current fact |
|------|--------------|
| TCP inbound | Accepts AEAD stream requests and returns `ShadowsocksAccept` |
| TCP outbound | Writes the initial target request and returns `ShadowsocksOutboundSession` |
| TCP stream | `ShadowsocksAeadStream` owns chunk encryption, decryption, response salt, and download key derivation |
| UDP datagram | `UdpDatagramFraming` encodes and decodes Shadowsocks UDP packets |
| UDP composition | `ShadowsocksDatagramCodec` is used by generic packet-path orchestration; Shadowsocks final-hop UDP chains support SOCKS5 and Shadowsocks packet-path carriers |
| MUX | Not applicable |

Supported cipher names:

- `aes-128-gcm`
- `aes-256-gcm`
- `chacha20-ietf-poly1305`
- `2022-blake3-aes-128-gcm`
- `2022-blake3-aes-256-gcm`
- `2022-blake3-chacha20-poly1305`

## Boundaries

```text
src/lib.rs       - crate root and re-exports
src/inbound.rs   - inbound request parsing and accept state
src/outbound.rs  - outbound TCP session and UDP datagram framing
src/shared.rs    - cipher enum, key derivation, address and target-data helpers
src/stream.rs    - AEAD stream wrapper
src/metadata.rs  - protocol capability descriptor
```

## Known Limits

- External interoperability coverage is incomplete.
