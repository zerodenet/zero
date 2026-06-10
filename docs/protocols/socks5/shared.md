# SOCKS5 共享模块

对应 `protocols/socks5/src/shared.rs` — 认证协商、地址编码、UDP 包构建/解析。

## 认证方法

```rust
pub const AUTH_NONE: u8 = 0x00;
pub const AUTH_USERNAME_PASSWORD: u8 = 0x02;
pub const AUTH_NO_ACCEPTABLE: u8 = 0xFF;
```

## 地址类型

```rust
pub const ATYP_IPV4: u8 = 0x01;
pub const ATYP_DOMAIN: u8 = 0x03;
pub const ATYP_IPV6: u8 = 0x04;
```

## UDP 包格式

```
[RSV(2)][FRAG(1)][ATYP][ADDR][PORT][PAYLOAD]
```

- RSV: 0x0000
- FRAG: 0x00 (不支持分片)

### build_udp_packet

```rust
pub fn build_udp_packet(target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>>
```

### parse_udp_packet

```rust
pub fn parse_udp_packet(data: &[u8]) -> Result<UdpPacket>
```
