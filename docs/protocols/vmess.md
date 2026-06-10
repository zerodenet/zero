# VMess

VMess is currently an `experimental` protocol in Zero. The in-tree implementation covers Zero-to-Zero TCP, TCP/UDP MUX, UDP-over-stream, and same-protocol relay-chain baselines. External TCP and UDP baseline interoperability is validated against Xray, sing-box, and Mihomo/Clash-family behavior. External Xray WS/gRPC TCP transport interoperability is validated in both directions. Mainstream `cipher: zero` compatibility is still tracked as incomplete.

## Current Capability

| Capability | Status | Notes |
|------|------|------|
| TCP inbound | `experimental` | TLS ingress; raw TLS, WebSocket over TLS, and gRPC over TLS are supported |
| TCP outbound | `experimental` | Raw TCP, TLS, WebSocket, and gRPC transports; MUX connection pool for outbound TCP multiplexing |
| UDP inbound | `experimental` | Supports VMess `CMD_UDP`; authenticated sessions enter the kernel `UdpPipe`; accepts both VMess UDP packet payloads and mainstream raw datagram payloads |
| UDP outbound | `experimental` | Supports VMess UDP-over-stream single-hop outbound and MUX UDP sub-connections |
| VMess -> VMess UDP relay-chain | `experimental` | Supports same-protocol VMess UDP relay-chain; local SOCKS5/Mixed is only the client entry |
| TCP body relay | `experimental` | `VmessAeadStream` owns body chunk encryption/decryption state; supports authenticated length, chunk masking (SHAKE128), global padding, and periodic rekey (every 2^14 chunks) |
| UDP packet framing | `experimental` | The VMess crate owns UDP packet encode/decode |
| cipher | `experimental` | Supports `auto`, `aes-128-gcm`, `chacha20-poly1305`, `none`, and `zero`; `auto` is normalized to the current AEAD baseline. `none` has Xray TCP interoperability coverage. `zero` is covered only by Zero-to-Zero tests and is not a mainstream external compatibility claim |
| MUX | `experimental` | TCP and UDP Mux.Cool sub-connections are implemented; UDP supports both Zero packet payloads and mainstream raw datagram payloads; outbound MUX uses a connection pool keyed by (server, port, uuid, cipher, transport) |

## Validation Coverage

In-tree validation covers:

- VMess TCP raw TLS, WSS, and gRPC outbound paths.
- Bidirectional AEAD body relay for every explicit supported cipher.
- Shutdown termination chunk delivery for every cipher.
- VMess UDP packet framing (domain, IPv4, IPv6 targets).
- SOCKS5 UDP ASSOCIATE -> VMess TLS outbound -> VMess inbound -> direct UDP echo.
- SOCKS5 entry -> VMess -> VMess -> direct UDP echo as a same-protocol VMess relay-chain path.
- SOCKS5 TCP entry -> VMess TCP MUX outbound -> VMess inbound -> direct TCP echo.
- SOCKS5 UDP ASSOCIATE -> VMess MUX UDP sub-connection -> VMess inbound -> direct UDP echo.

External validation covers:

- Zero VMess outbound -> Xray VMess inbound, TCP, `aes-128-gcm`.
- Zero VMess outbound -> Xray VMess inbound, TCP, `none`.
- Xray VMess outbound -> Zero VMess inbound, TCP, `aes-128-gcm`.
- Xray VMess outbound -> Zero VMess inbound, TCP, `none`.
- Zero VMess outbound -> Xray VMess inbound, WebSocket over TLS, TCP.
- Xray VMess outbound -> Zero VMess inbound, WebSocket over TLS, TCP.
- Zero VMess outbound -> Xray VMess inbound, gRPC over TLS, TCP.
- Xray VMess outbound -> Zero VMess inbound, gRPC over TLS, TCP.
- Zero VMess outbound -> sing-box VMess inbound, TCP, `aes-128-gcm`.
- Zero VMess outbound -> sing-box VMess inbound, UDP.
- Mihomo VMess outbound -> Zero VMess inbound, TCP, `auto`.
- Mihomo VMess outbound -> Zero VMess inbound, UDP over `CMD_UDP` raw datagram payload.
- Zero VMess outbound -> Xray VMess inbound, UDP.
- Xray VMess outbound -> Zero VMess inbound, UDP over Mux.Cool raw datagram payload.
- Xray rejects VMess `zero` security on inbound; Zero therefore does not present `cipher: zero` as mainstream Xray-compatible behavior.

## Relay-chain Boundary

Current VMess chain tracking is same-protocol only: `vmess -> vmess`. Local SOCKS5 or Mixed entry is just the client ingress and does not make the path a general cross-protocol relay-chain. Arbitrary multi-protocol combinations are not part of the VMess completion target.

## Remaining Gaps

| Gap | Impact |
|------|------|
| `cipher: zero` external compatibility | Zero-to-Zero is covered; Xray inbound rejects `zero` security, so GUI integrations should not expose it as a mainstream default |

Zero has validated the listed Xray, sing-box, and Mihomo/Clash-family paths. Do not generalize that evidence to untested transport combinations.

## Inbound Config

```json
{
  "tag": "vmess-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "vmess",
    "users": [
      {
        "id": "11111111-2222-3333-4444-555555555555",
        "cipher": "aes-128-gcm"
      }
    ],
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    }
  }
}
```

`tls` is required for VMess inbound. `users[].cipher` is optional and defaults to `aes-128-gcm`. `ws` and `grpc` are mutually exclusive.

## Outbound Config

```json
{
  "tag": "vmess-out",
  "protocol": {
    "type": "vmess",
    "server": "example.com",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555",
    "cipher": "aes-128-gcm",
    "mux_concurrency": 8
  }
}
```

Optional transport fields are `tls`, `ws`, and `grpc`. `ws` and `grpc` are mutually exclusive. `mux_concurrency` enables VMess TCP MUX for outbound TCP sessions; `mux_idle_timeout_secs` is accepted as a pool policy field.
