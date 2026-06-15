# Shadowsocks

This crate owns the Shadowsocks protocol semantics used by Zero. The proxy
runtime owns orchestration, sockets, routing, sessions, stats, events, and
response bridging.

## Capability

| Area | Current fact |
|------|--------------|
| TCP inbound | Accepts AEAD stream requests (legacy + 2022 SIP022) and returns `ShadowsocksAccept` |
| TCP outbound | Writes the initial target request (legacy + 2022 SIP022) and returns `ShadowsocksOutboundSession` |
| TCP stream | `ShadowsocksAeadStream` owns chunk encryption, decryption, response salt, and download key derivation |
| UDP datagram | `UdpDatagramFraming` encodes and decodes Shadowsocks UDP packets; AEAD 2022 outbound client packets use the SIP022 UDP header format |
| UDP composition | `ShadowsocksDatagramCodec` is used by generic packet-path orchestration; Shadowsocks final-hop UDP chains support SOCKS5 and Shadowsocks packet-path carriers |
| MUX | Not applicable |

## Validation

In-tree validation covers these Shadowsocks paths:

- TCP outbound through a SOCKS5 inbound to a Shadowsocks inbound for every
  supported cipher listed below, including a large payload that crosses AEAD
  chunk boundaries.
- TCP authentication failure when the outbound password does not match the
  upstream Shadowsocks inbound password; the flow is closed before reaching the
  target service.
- UDP outbound through SOCKS5 UDP ASSOCIATE to a Shadowsocks inbound.
- UDP end-to-end relay for every supported cipher listed below.
- Shadowsocks UDP packet-path relay chains where the carrier is SOCKS5 UDP
  ASSOCIATE or Shadowsocks UDP.
- Local external UDP interoperability against `shadowsocks-rust ssserver -U`
  for every supported cipher listed below.

Supported cipher names:

- `aes-128-gcm`
- `aes-256-gcm`
- `chacha20-ietf-poly1305`
- `2022-blake3-aes-128-gcm`
- `2022-blake3-aes-256-gcm`
- `2022-blake3-chacha20-poly1305`

For AEAD 2022 cipher names, `password` is standard base64 key material. The
decoded length must match the method key length: 16 bytes for
`2022-blake3-aes-128-gcm`, and 32 bytes for
`2022-blake3-aes-256-gcm` and `2022-blake3-chacha20-poly1305`. AES 2022
passwords may include colon-separated identity keys; Zero uses the last segment
as the user PSK and does not emit EIH identity headers.

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

- AEAD 2022 UDP **server-side responses** are implemented and validated (SIP022
  3.2.3 echo of client session id, DNS round-trip probe).
- **SIP022 3.2.4** per-session sliding-window replay filtering and per-client
  session-id flow isolation are implemented and tested.
- The remaining limitation is `shadowsocks_2022_hardening_not_externally_validated`:
  the detection-prevention drain and sliding-window replay filter have not been
  validated against real active probes/replay attacks, and direct interop with
  an external `ssserver` is pending (blocked by a Windows-env single-read bug in
  the available `ssserver` build, exonerated by a reference-pair control test).
