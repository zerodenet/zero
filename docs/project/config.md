# Configuration

`v0.0.4` uses JSON. The current top-level structure is:

```json
{
  "inbounds": [],
  "outbounds": [],
  "outbound_groups": [],
  "runtime": {
    "udp_upstream_idle_timeout_seconds": 30,
    "dns": {
      "servers": [{ "type": "system" }],
      "cache": { "max_entries": 256 },
      "routes": [],
      "fake_ip": null
    }
  },
  "api": {
    "event_sinks": [],
    "control": { "enabled": false },
    "hooks": [],
    "push": {}
  },
  "mode": { "type": "rule" },
  "route": {
    "rule_sets": [],
    "rules": [],
    "url_rewrite": [],
    "final": { "type": "direct" }
  }
}
```

Only currently implemented configuration is documented here. Long-term design for modes and groups is in [modes-and-groups.md](modes-and-groups.md).

## runtime

`runtime.udp_upstream_idle_timeout_seconds` controls the idle timeout for upstream `SOCKS5` UDP associations.

- Default: `30`
- Unit: seconds
- Constraint: must be greater than `0`

### DNS

`runtime.dns` is the optional DNS subsystem configuration. When omitted, the system resolver is used, behavior unchanged.

```json
{
  "dns": {
    "servers": [
      { "type": "system" },
      { "type": "udp", "address": "8.8.8.8", "port": 53 }
    ],
    "cache": { "max_entries": 512, "max_ttl_seconds": 300 },
    "routes": [
      { "domain": "*.internal.local", "server": "system" },
      { "domain": "*.google.com", "server": "1" }
    ],
    "fake_ip": {
      "cidr": "198.18.0.0/15",
      "ttl_seconds": 86400,
      "exclude_domains": ["*.local"]
    }
  }
}
```

**servers** -- ordered DNS server list. All servers queried concurrently on resolution, fastest response wins.

| Type | Fields | Description |
|------|------|------|
| `system` | -- | OS resolver (getaddrinfo) |
| `udp` | `address`, `port` | Plain UDP DNS, default port 53 |
| `doh` | `url`, `server_name` | DNS-over-HTTPS (v2) |
| `dot` | `address`, `port`, `server_name` | DNS-over-TLS (v2), default port 853 |

**cache** -- TTL-based LRU cache.

| Field | Default | Description |
|------|------|------|
| `max_entries` | `256` | Maximum cache entries |
| `max_ttl_seconds` | -- | TTL ceiling; omitting uses DNS record TTL |

**routes** -- domain-to-server routing. `domain` supports exact (`example.com`) and wildcard (`*.example.com`). `server` is `"system"` or a servers array index (`"0"`, `"1"`).

**fake_ip** -- core of transparent proxying. Returns fake IPs for matching domains, maintains domain-to-fake-IP mapping, reverse-resolves to real domain for routing on connection.

| Field | Default | Description |
|------|------|------|
| `cidr` | -- | Fake IP pool CIDR, recommended `198.18.0.0/15` |
| `ttl_seconds` | `86400` | Fake IP allocation lifetime |
| `exclude_domains` | `[]` | Excluded domains, use real DNS |

## api

`api` is the optional control plane and observability configuration. Related runtime capabilities are controlled by Cargo features; the presence of config does not guarantee default build support.

### event_sinks

`api.event_sinks` describes delivery targets for normalized events. Event types must come from the event catalog in [api.md](api.md).

Local JSON Lines:

```json
{
  "tag": "local-events",
  "type": "jsonl",
  "path": "zero-events.jsonl",
  "events": ["flow.completed"],
  "source_id": "edge-local"
}
```

Panel webhook:

```json
{
  "tag": "panel",
  "type": "webhook",
  "url": "https://panel.example.com/api/zero/events",
  "events": ["flow.completed", "engine.warning"],
  "source_id": "edge-shanghai-01",
  "api_key_env": "ZERO_PANEL_API_KEY"
}
```

`webhook` uses `Authorization: Bearer <api-key>`. Prefer `api_key_env`; `api_key` is also supported for testing. `http://` webhooks require explicit `allow_insecure: true`.

### control

`api.control` enables the panel to actively query nodes and issue commands. It is off by default and requires an API key when enabled:

```json
{
  "enabled": true,
  "listen": { "address": "127.0.0.1", "port": 9090 },
  "api_key_env": "ZERO_NODE_API_KEY"
}
```

The current control plane uses `Authorization: Bearer <api-key>` or `X-Zero-Api-Key: <api-key>`. It is recommended to listen only on localhost, internal networks, or firewall-protected addresses.

Current HTTP control plane supports:

```text
GET  /api/v1/status
GET  /api/v1/config
GET  /api/v1/runtime
GET  /api/v1/events
POST /api/v1/commands
POST /api/v1/selectors/{group}/{target}
```

`POST /api/v1/commands` uses a unified command JSON, e.g.:

```json
{
  "method": "policies.select",
  "params": {
    "policy_tag": "proxy",
    "target_tag": "direct"
  }
}
```

## Inbounds

Each inbound must have `tag`, `listen`, and `protocol`. An optional `idle_timeout_secs` field controls TCP idle timeout.

```json
{
  "tag": "mixed-in",
  "listen": { "address": "127.0.0.1", "port": 7890 },
  "protocol": { "type": "mixed" },
  "idle_timeout_secs": 300
}
```

| Field | Type | Default | Description |
|------|------|------|------|
| `tag` | string | (required) | Unique inbound identifier |
| `listen.address` | string | (required) | Bind address |
| `listen.port` | u16 | (required) | Bind port |
| `protocol` | object | (required) | Protocol-specific configuration |
| `idle_timeout_secs` | u64 | `300` | TCP relay idle timeout in seconds |

### idle_timeout_secs

The kernel wraps every TCP relay in `tokio::time::timeout`. If no bytes are transferred in either direction for `idle_timeout_secs`, the session is cleanly terminated. This is per-inbound; different listeners can have different timeouts. Omitting the field uses the kernel default of 300 seconds (5 minutes).

### Currently supported protocols

- `socks5`
- `http-connect`
- `http` (alias)
- `mixed` -- same port auto-detects `socks5` and `http-connect`
- `vless` -- TCP/TLS/WS/WSS, Reality, gRPC, H2, HTTPUpgrade, QUIC, SplitHTTP; MUX + Vision flow + UDP over TCP
- `hysteria2` -- QUIC, TCP streams and UDP datagram forwarding
- `shadowsocks` -- AEAD cipher (chacha20-ietf-poly1305, aes-128-gcm, aes-256-gcm); 2022-blake3
- `trojan` -- TLS + SHA224 password auth, TCP streams
- `direct` -- fixed-target TCP forwarder; accepts raw TCP with no handshake, outbound determined by normal route rules
- `tun` -- virtual network interface; started at runtime via CLI/API commands, routes traffic through normal rule matching

`mixed` is not an external protocol, but a config entry for "same port multi-protocol inbound".

### Direct inbound

`direct` inbound listens on a port, accepts raw TCP connections with no protocol handshake, and forwards all traffic through the normal route rules. The target address comes from the inbound config rather than the client. Outbound selection follows the standard routing pipeline -- `mode`, `rules`, `rule_sets`, and `final`.

```json
{
  "tag": "direct-in",
  "listen": { "address": "127.0.0.1", "port": 8080 },
  "protocol": {
    "type": "direct",
    "target": "example.com",
    "port": 443
  }
}
```

Direct inbound config fields:
- `target` -- optional, target address (IP or domain) for forwarded connections; must be present at runtime (defaults to nothing)
- `port` -- optional, target port, default `443`

### TUN inbound

`tun` is a virtual network interface inbound. Unlike other inbounds, it is not declared in the static JSON configuration. Instead, it is started and stopped at runtime via CLI, IPC, or HTTP control plane commands.

```bash
# Start a TUN device
zero tun start --addr 10.0.0.1 --mask 255.255.255.0 --tag my-tun --name tun0

# Check TUN status
zero tun status

# Stop the TUN device
zero tun stop
```

HTTP control plane equivalent (via `POST /api/v1/commands`):

```json
{ "method": "tun.start", "params": { "addr": "10.0.0.1", "mask": "255.255.255.0", "tag": "my-tun", "name": "tun0", "mtu": 1500 } }
{ "method": "tun.stop" }
```

TUN start parameters:
- `addr` -- required, IP address assigned to the virtual interface
- `mask` -- netmask, default `255.255.255.0`
- `tag` -- required, inbound tag used for routing decisions; TUN traffic matches route rules by this tag
- `name` -- optional, OS-level device name (e.g. `tun0`, `utun8`); auto-assigned if omitted
- `mtu` -- optional, MTU in bytes, default `1500`

Internally, TUN reads raw IP packets from the virtual interface, parses TCP headers (IPv4 currently), maintains a minimal TCP state machine, and dispatches each TCP connection through `serve_inbound()` for unified routing and relay. The implementation is in `crates/proxy/src/inbound/tun.rs` with platform backends in `crates/tun/` (Linux ioctl, macOS utun, Windows Wintun).

SOCKS5 inbound defaults to no-auth. Configuring `users` enables RFC 1929 username/password:

```json
{
  "tag": "socks-in",
  "listen": { "address": "127.0.0.1", "port": 7890 },
  "protocol": {
    "type": "socks5",
    "users": [
      { "username": "alice", "password": "secret" }
    ]
  }
}
```

`mixed` inbound can also configure auth for the SOCKS5 branch:

```json
{
  "tag": "mixed-in",
  "listen": { "address": "127.0.0.1", "port": 7890 },
  "protocol": {
    "type": "mixed",
    "socks5_users": [
      { "username": "alice", "password": "secret" }
    ]
  }
}
```

VLESS inbound must configure user UUIDs. `credential_id` and `principal_key` are observability attribution fields that appear in `flow.completed`'s `auth` and the event top-level `principal_key`; UUIDs themselves are not sent back to the panel by default:

```json
{
  "tag": "vless-in",
  "listen": { "address": "127.0.0.1", "port": 8443 },
  "protocol": {
    "type": "vless",
    "users": [
      {
        "id": "11111111-2222-3333-4444-555555555555",
        "credential_id": "node-user-1",
        "principal_key": "user:10001"
      }
    ]
  }
}
```

VLESS inbound with TLS, add `tls` inside the protocol:

```json
{
  "tag": "vless-tls-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "vless",
    "users": [
      { "id": "11111111-2222-3333-4444-555555555555" }
    ],
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    }
  }
}
```

VLESS inbound supports WebSocket transport, enable with `ws`:

```json
{
  "tag": "vless-ws-in",
  "listen": { "address": "0.0.0.0", "port": 80 },
  "protocol": {
    "type": "vless",
    "users": [
      { "id": "11111111-2222-3333-4444-555555555555" }
    ],
    "ws": {
      "path": "/vless"
    }
  }
}
```

WebSocket can be combined with TLS (WSS):

```json
{
  "tag": "vless-wss-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "vless",
    "users": [
      { "id": "11111111-2222-3333-4444-555555555555" }
    ],
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    },
    "ws": {
      "path": "/vless"
    }
  }
}
```

### Hysteria2 inbound

Hysteria2 inbound carries TCP streams and UDP datagrams over QUIC. The server requires a certificate:

```json
{
  "tag": "hysteria2-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "hysteria2",
    "password": "your-secret-password",
    "cert_path": "certs/fullchain.pem",
    "key_path": "certs/privkey.pem"
  }
}
```

Hysteria2 config fields:
- `password` -- required, client authentication password
- `cert_path` -- optional, TLS certificate path
- `key_path` -- optional, TLS private key path
- `up_bps` -- optional, upload rate limit in bytes/sec (kernel GCRA)
- `down_bps` -- optional, download rate limit in bytes/sec (kernel GCRA)

### Shadowsocks inbound

Shadowsocks inbound uses AEAD cipher for encrypted transport:

```json
{
  "tag": "ss-in",
  "listen": { "address": "127.0.0.1", "port": 8388 },
  "protocol": {
    "type": "shadowsocks",
    "password": "your-secret-password",
    "cipher": "chacha20-ietf-poly1305"
  }
}
```

Shadowsocks config fields:
- `password` -- required, encryption password
- `cipher` -- optional, encryption algorithm, default `chacha20-ietf-poly1305`
- `up_bps` -- optional, upload rate limit in bytes/sec (kernel GCRA)
- `down_bps` -- optional, download rate limit in bytes/sec (kernel GCRA)

### Trojan inbound

Trojan inbound requires TLS, performs password authentication inside the TLS tunnel then forwards the target address:

```json
{
  "tag": "trojan-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "trojan",
    "password": "your-secret-password",
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    }
  }
}
```

Trojan inbound config fields:
- `password` -- required, authentication password (SHA224 hashed for comparison)
- `sni` -- optional, TLS SNI value
- `tls` -- required, TLS certificate config
  - `cert_path` -- certificate file path
  - `key_path` -- private key file path
- `up_bps` -- optional, upload rate limit in bytes/sec (kernel GCRA)
- `down_bps` -- optional, download rate limit in bytes/sec (kernel GCRA)

### Per-inbound rate limits (rate_limits)

Hysteria2, Shadowsocks, and Trojan inbound protocol configs support `up_bps` and `down_bps` fields for per-inbound GCRA rate limiting. These are the values returned by `InboundProtocolConfig::rate_limits()`.

The kernel applies these as defaults in `serve_inbound()` via `apply_kernel_rate_limits()`. If a protocol's accept handler already set per-user limits (e.g. SOCKS5 `AuthHandler::rate_limit_for()`), the per-inbound defaults are not applied -- per-user limits always take priority.

SOCKS5, HTTP CONNECT, Mixed, and VLESS inbounds do not currently support per-inbound rate limits in their protocol config (they return `(None, None)` from `rate_limits()`).

## Outbounds

```json
{
  "tag": "chain",
  "protocol": {
    "type": "socks5",
    "server": "127.0.0.1",
    "port": 2080
  }
}
```

Currently supported:

- `direct`
- `block`
- `socks5`
- `vless`
- `hysteria2`
- `shadowsocks`
- `trojan`

SOCKS5 outbound defaults to no-auth. Configure `username` and `password` when connecting to an authenticated upstream:

```json
{
  "tag": "chain",
  "protocol": {
    "type": "socks5",
    "server": "127.0.0.1",
    "port": 2080,
    "username": "upstream",
    "password": "secret"
  }
}
```

VLESS outbound for connecting to upstream VLESS TCP nodes:

```json
{
  "tag": "vless-chain",
  "protocol": {
    "type": "vless",
    "server": "203.0.113.10",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555"
  }
}
```

Connecting to a TLS VLESS upstream, configure `tls`. `server_name` defaults to `server`. Self-signed or private CA can use `ca_cert_path`. When the upstream does not depend on SNI or the target domain should be hidden, set `disable_sni: true`:

```json
{
  "tag": "vless-tls-chain",
  "protocol": {
    "type": "vless",
    "server": "edge.example.com",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555",
    "tls": {
      "server_name": "edge.example.com",
      "ca_cert_path": "certs/ca.pem",
      "disable_sni": false,
      "insecure": false
    }
  }
}
```

TLS config fields:
- `server_name` -- optional, SNI and certificate verification domain, defaults to `server`
- `ca_cert_path` -- optional, custom CA certificate path
- `disable_sni` -- optional, do not send SNI extension, default `false`
- `insecure` -- optional, skip certificate verification, default `false`
- `alpn` -- optional, ALPN protocol list

Connecting to a VLESS Reality upstream, configure `reality`. Reality is a VLESS TLS-like security layer and cannot be combined with `tls` or `ws`; current support is raw TCP outbound Reality:

```json
{
  "tag": "vless-reality-chain",
  "protocol": {
    "type": "vless",
    "server": "edge.example.com",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555",
    "reality": {
      "public_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
      "short_id": "0123456789abcdef",
      "server_name": "www.cloudflare.com"
    }
  }
}
```

Reality config fields:
- `public_key` -- required, upstream Reality X25519 public key, base64url no padding encoding, must decode to 32 bytes
- `short_id` -- optional, 0 to 16 hex characters, default empty
- `server_name` -- optional, SNI used in Reality ClientHello, defaults to `server`
- `cipher_suites` -- optional, TLS 1.3 cipher suite name list; supports `TLS_AES_128_GCM_SHA256`, `TLS_AES_256_GCM_SHA384`, `TLS_CHACHA20_POLY1305_SHA256`

VLESS outbound supports WebSocket transport, enable with `ws`:

```json
{
  "tag": "vless-ws-chain",
  "protocol": {
    "type": "vless",
    "server": "edge.example.com",
    "port": 80,
    "id": "11111111-2222-3333-4444-555555555555",
    "ws": {
      "path": "/vless",
      "headers": {
        "User-Agent": "zero-proxy"
      }
    }
  }
}
```

WebSocket can be combined with TLS (WSS):

```json
{
  "tag": "vless-wss-chain",
  "protocol": {
    "type": "vless",
    "server": "edge.example.com",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555",
    "tls": {
      "server_name": "edge.example.com"
    },
    "ws": {
      "path": "/vless"
    }
  }
}
```

WebSocket config fields:
- `path` -- WebSocket handshake path, must not be empty
- `headers` -- optional, custom HTTP headers; must not include `Host`, `Connection`, `Upgrade`, `Sec-WebSocket-*` and other required handshake headers

### Hysteria2 outbound

Connect to upstream Hysteria2 node, carrying TCP and UDP over QUIC:

```json
{
  "tag": "hysteria2-chain",
  "protocol": {
    "type": "hysteria2",
    "server": "example.com",
    "port": 443,
    "password": "your-secret-password",
    "insecure": true
  }
}
```

Hysteria2 outbound config fields:
- `server` -- required, upstream server address
- `port` -- required, upstream port, must be greater than 0
- `password` -- required, authentication password
- `insecure` -- optional, skip certificate verification, default `false`

### Shadowsocks outbound

Connect to upstream Shadowsocks node:

```json
{
  "tag": "ss-chain",
  "protocol": {
    "type": "shadowsocks",
    "server": "example.com",
    "port": 8388,
    "password": "your-secret-password",
    "cipher": "chacha20-ietf-poly1305"
  }
}
```

Shadowsocks outbound config fields:
- `server` -- required, upstream server address
- `port` -- required, upstream port, must be greater than 0
- `password` -- required, encryption password
- `cipher` -- optional, encryption algorithm, default `chacha20-ietf-poly1305`

### Trojan outbound

Connect to upstream Trojan node, authenticate via password inside a TLS tunnel then forward:

```json
{
  "tag": "trojan-chain",
  "protocol": {
    "type": "trojan",
    "server": "example.com",
    "port": 443,
    "password": "your-secret-password",
    "sni": "example.com",
    "insecure": false
  }
}
```

Trojan outbound config fields:
- `server` -- required, upstream server address
- `port` -- required, upstream port, must be greater than 0
- `password` -- required, authentication password (SHA224 hashed before sending)
- `sni` -- optional, TLS SNI, defaults to `server`
- `insecure` -- optional, skip certificate verification, default `false`

UDP currently supports only `direct`, `block`, and upstream `socks5`; upstream `vless` is not yet supported.

### Outbound circuit breaker

`zero-engine` maintains health state for every chained outbound tag. Before each connection attempt, `establish_tcp_candidate` calls `check_outbound_health()`. If 5 failures accumulate within a 30-second window, the outbound is quarantined for 60 seconds. After quarantine, a single probe connection is allowed; success clears the unhealthy state, failure resets the cooldown.

This is a kernel primitive -- no configuration required. It applies automatically to all outbound connection paths except `direct` and `block`.

## Outbound Groups

Three outbound group types are currently implemented:

- `selector`
- `fallback`
- `urltest`

Group members may be either concrete outbounds or other outbound groups. Circular references are rejected at config validation.

### selector

```json
{
  "tag": "proxy",
  "type": "selector",
  "outbounds": ["node-a", "node-b"],
  "selected": "node-a"
}
```

`selector` supports runtime switching. When launched with `--status-listen`, use the local endpoint:

```text
POST /selectors/{group_tag}/{target_tag}
```

For example:

```text
POST /selectors/proxy/direct
```

After a successful switch, `outbound_groups[*].selected` in `/config` and `/status` immediately reflects the new selection.

### fallback

```json
{
  "tag": "proxy",
  "type": "fallback",
  "outbounds": ["node-a", "direct"]
}
```

Semantics:

- Try members in configured order
- On connection failure, automatically fall through to the next member
- Once a connection succeeds, fix on that member for the session
- Circuit breaker quarantines unhealthy members before the connection attempt, causing automatic fall-through

### urltest

```json
{
  "tag": "proxy",
  "type": "urltest",
  "outbounds": ["node-a", "node-b", "direct"],
  "url": "http://example.com/",
  "interval_seconds": 300
}
```

Semantics:

- Probe on `interval_seconds` interval
- Currently only `http://` probe URLs are supported
- Select the member with successful probe and lowest latency
- If all probes fail this round, keep the current selection; before the first probe, default to the first member

## Mode

Currently supported:

- `rule`
- `global`
- `direct`

`global` requires referencing an outbound or outbound group:

```json
{
  "type": "global",
  "outbound": "proxy"
}
```

## Route

Rules are `condition + action`:

```json
{
  "condition": { "type": "domain", "values": ["blocked.example"] },
  "action": { "type": "route", "outbound": "block" }
}
```

Currently supported conditions:

- `domain` -- domain matching, supports `example.com` exact and `*.example.com` wildcard
- `domain-keyword` -- match if domain contains keyword
- `domain-regex` -- match domain against one or more regex patterns
- `ip` -- CIDR matching
- `rule-set` -- reference external rule set files
- `geoip` -- MaxMind GeoLite2-Country mmdb country code matching
- `sni` -- TLS ClientHello SNI domain matching (same syntax as domain)
- `and` -- all sub-conditions must match
- `or` -- any sub-condition must match

Currently supported actions:

- `direct`
- `reject`
- `block` (alias)
- `route`

### domain-regex condition

The `domain-regex` condition matches the target domain against one or more regex patterns. Patterns are compiled at startup. Matches against the target domain extracted from the session. Supports composition with `and` / `or`.

```json
{
  "condition": { "type": "domain-regex", "values": ["^.*\\.google\\..*$", "^.*\\.youtube\\..*$"] },
  "action": { "type": "route", "outbound": "proxy" }
}
```

Note: capture groups in `domain-regex` patterns are not used for routing context. For capture-based domain substitution, use `url_rewrite.from_regex` instead.

### url_rewrite

`route.url_rewrite` is an array of domain rewrite rules applied before routing. Rules are matched first-match-wins: the first rule whose `from` or `from_regex` matches the target domain wins, and no further rules are evaluated.

Each `UrlRewriteRule`:

| Field | Type | Default | Description |
|------|------|------|------|
| `from` | string | -- | Exact domain to match |
| `from_regex` | string | -- | Regex pattern to match against the domain |
| `to` | string | (required) | Replacement domain; supports `$1`, `$2`, etc. for regex captures |
| `status_code` | u16 | -- | If set, return an HTTP redirect (e.g. `302`); HTTP-based protocols only |

At least one of `from` or `from_regex` must be set.

`status_code` triggers a protocol-level HTTP redirect (for HTTP CONNECT). Non-HTTP protocols (SOCKS5, Shadowsocks, etc.) silently ignore `status_code` and always rewrite the target domain.

```json
{
  "route": {
    "url_rewrite": [
      { "from": "old.example.com", "to": "new.example.com" },
      { "from_regex": "^(.+)\\.mirror\\.example\\.com$", "to": "$1.example.com" },
      { "from": "temp.example.com", "to": "permanent.example.com", "status_code": 301 }
    ],
    "rules": [],
    "final": { "type": "direct" }
  }
}
```

## External Rule Sets

Match data can be placed in external files and referenced via `tag` in the main config.

Currently supported:

- `type = file`
- `type = url` (remote fetch with local cache)
- `format = domain-list`
- `format = cidr-list`

```json
{
  "route": {
    "rule_sets": [
      {
        "tag": "ads",
        "type": "file",
        "path": "rules/ads.txt",
        "format": "domain-list"
      },
      {
        "tag": "lan",
        "type": "file",
        "path": "rules/lan.txt",
        "format": "cidr-list"
      }
    ],
    "rules": [
      {
        "condition": { "type": "rule-set", "tag": "ads" },
        "action": { "type": "reject" }
      },
      {
        "condition": { "type": "rule-set", "tag": "lan" },
        "action": { "type": "route", "outbound": "direct" }
      }
    ],
    "final": { "type": "route", "outbound": "proxy" }
  }
}
```

Notes:

- `path` supports relative paths, resolved against the config file directory by default
- `domain-list` loads as a domain list
- `cidr-list` loads as a CIDR list
- Blank lines are ignored
- Lines starting with `#` or `//` are ignored
- Rule files only contain match data, not actions
- `type = url` additionally requires a `url` field; the file at `path` serves as the local cache

## Status Field Semantics

`status --json` current field semantics related to sessions:

- `bytes_up` / `bytes_down`
  - Cumulative application-layer link bytes from the flow perspective
  - Includes SOCKS5 / HTTP CONNECT handshake, SOCKS5 UDP packet headers, and forwarded payload
  - Excludes TCP/IP headers, TCP three-way handshake, and other kernel network stack overhead
  - TCP stats per connection, SOCKS5 UDP stats per target flow
- `inbound_rx_bytes` / `inbound_tx_bytes`
  - Application-layer bytes actually read/written on the inbound side
- `outbound_rx_bytes` / `outbound_tx_bytes`
  - Application-layer bytes actually read/written on the outbound side
- `throughput_up_bps` / `throughput_down_bps`
  - 1-second sampled throughput
- `recent_completed_sessions`
  - Settlement records for recently completed sessions
  - TCP connections and SOCKS5 UDP flows use the same field structure
- `outbound_groups[*].selected`
  - Currently selected member for the group
- `outbound_groups[*].latency_ms`
  - `urltest` most recent successful probe latency
- `outbound_groups[*].last_checked_unix_ms`
  - `urltest` most recent probe completion time

## Constraints

- `tag` must not be empty
- SOCKS5 username/password must not be empty, max 255 bytes each
- SOCKS5 outbound auth must configure both `username` and `password`, cannot configure only one
- VLESS inbound must have at least one user, `id` must be a UUID; when TLS is enabled, `cert_path` and `key_path` must not be empty; when WebSocket is enabled, `ws.path` must not be empty
- VLESS outbound `server` must not be empty, `port` must be greater than `0`, `id` must be a UUID; `tls.server_name`, `tls.ca_cert_path`, and `reality.server_name` must not be empty if configured
- VLESS outbound `reality.public_key` must be a 32-byte base64url no padding value; `reality.short_id` max 16 hex characters; `reality` cannot be combined with `tls` or `ws`
- Tags within the same object type must not be duplicated
- The same `address:port` can only have one inbound
- Use `mixed` when the same port needs both `socks5` and `http-connect`
- Targets referenced by `route` and `global mode` must exist
- Members in outbound groups must be defined outbounds or defined groups
- Outbound groups must not have circular references
- `runtime.udp_upstream_idle_timeout_seconds` must be greater than `0`
- `rule_sets[*].tag` must not be empty and must not duplicate
- `rule-set` condition referenced `tag` must exist
- `urltest.url` must currently be `http://`
- `urltest.interval_seconds` must be greater than `0`
- Hysteria2 inbound `password` must not be empty; outbound `server` must not be empty, `port` must be greater than `0`
- Shadowsocks inbound and outbound `password` must not be empty
- Trojan inbound must configure `tls` with non-empty `cert_path` and `key_path`, `password` must not be empty; outbound `server` must not be empty, `port` must be greater than `0`, `password` must not be empty
- `domain-regex` condition requires at least one pattern in `values`
- `url_rewrite` rules require at least one of `from` or `from_regex`, and `to` must not be empty
- `idle_timeout_secs` must be greater than `0` if set

## Runtime Management

### Mode Switching

Post-startup mode can be hot-switched via CLI, IPC, or HTTP API with no restart:

```bash
zero mode rule              # Switch back to rule matching
zero mode direct            # All direct
zero mode global proxy      # Global via specified outbound
```

### Hot Reload

`zero reload <config>` reloads the configuration file. The following changes take effect immediately:

- route rules, mode, DNS config -- hot swap
- outbound_groups adjustments -- hot swap
- inbounds/outbounds additions/removals/changes -- require restart

### Selector Switching

```bash
zero select <group-tag> <target-tag>
```

Equivalent HTTP API: `POST /api/v1/selectors/{group}/{target}`

## Examples

- [basic.json](../../examples/v0.0.1/basic.json)
- [mixed.json](../../examples/v0.0.1/mixed.json)
- [blocked-route.json](../../examples/v0.0.1/blocked-route.json)
- [chained-socks5.json](../../examples/v0.0.1/chained-socks5.json)
- [global-selector.json](../../examples/v0.0.1/global-selector.json)
- [rule-set-files.json](../../examples/v0.0.1/rule-set-files.json)
- [server-socks5.json](../../examples/v0.0.1/server-socks5.json)
- [udp-socks5.json](../../examples/v0.0.1/udp-socks5.json)
- [fallback.json](../../examples/v0.0.2/fallback.json)
- [nested-groups.json](../../examples/v0.0.2/nested-groups.json)
- [urltest.json](../../examples/v0.0.2/urltest.json)
- [vless.json](../../examples/v0.0.2/vless.json)
- [vless-tls.json](../../examples/v0.0.2/vless-tls.json)
- [vless-ws.json](../../examples/v0.0.2/vless-ws.json)
- [chained-vless-tls.json](../../examples/v0.0.2/chained-vless-tls.json)
- [chained-vless-reality.json](../../examples/v0.0.2/chained-vless-reality.json)
- [hysteria2.json](../../examples/v0.1.0/hysteria2.json)
- [shadowsocks.json](../../examples/v0.1.0/shadowsocks.json)
- [trojan.json](../../examples/v0.1.0/trojan.json)
