# VMess UDP

对应 `protocols/vmess/src/udp.rs` — `VmessUdpPacket`、`build_udp_packet`、`parse_udp_packet`。

## VmessUdpPacket

```rust
pub struct VmessUdpPacket {
    pub target: VmessUdpPacketTarget,  // Address + Port
    pub payload: Vec<u8>,
}
```

### VmessUdpPacketTarget

```rust
pub enum VmessUdpPacketTarget {
    Domain { domain: String, port: u16 },
    Ipv4 { addr: [u8; 4], port: u16 },
    Ipv6 { addr: [u8; 16], port: u16 },
}
```

### VmessUdpPacketTunnelTarget

用于 UDP over stream (CMD_UDP) 的 tunnel target 表示，包含完整的 outbound 配置。

## Packet 格式

### VMess packet 模式

```
[ATYP][ADDR][PORT][TYPE][PAYLOAD_LEN][PAYLOAD]
```
- TYPE: 0x01 = data
- 符合 VMess CMD_UDP 规范

### Raw datagram 模式

```
[ATYP][ADDR][PORT][PAYLOAD]
```
- 无 TYPE 和 PAYLOAD_LEN 字段
- 主流实现（Mihomo 等）使用此格式

## Payload 模式自动检测

Inbound 自动检测 payload 是 VMess packet 模式还是 raw datagram 模式：

```rust
pub fn parse_udp_packet(data: &[u8]) -> Result<VmessUdpPacket>
```

根据数据长度和结构判断格式，兼容两种客户端行为。

## VmessUdpOutboundManager

UDP outbound session 管理器：

- 缓存上游 VMess UDP stream
- Broadcast channel 响应桥接：多个本地 UDP session 共享同一条 VMess UDP stream 的响应
- Session 生命周期管理（空闲超时回收）

## VMess → VMess UDP Relay Chain

同协议 UDP 中继链路：`vmess → vmess`。SOCKS5/Mixed 入口仅作为客户端入口，不构成跨协议中继。

## 已验证路径

In-tree:
- VMess UDP packet framing (domain, IPv4, IPv6 targets)
- SOCKS5 UDP ASSOCIATE → VMess TLS outbound → VMess inbound → direct UDP echo
- SOCKS5 entry → VMess → VMess → direct UDP echo (同协议 relay chain)
- SOCKS5 UDP ASSOCIATE → VMess MUX UDP sub-connection → VMess inbound → direct UDP echo

External (local only):
- Zero ↔ Xray 双向 UDP
- Xray → Zero UDP over Mux.Cool raw datagram
- Zero → sing-box UDP
- Mihomo → Zero UDP (`CMD_UDP` raw datagram)
