# Shadowsocks Metadata

对应 `protocols/shadowsocks/src/metadata.rs` — `ShadowsocksProtocol` 实现 `ProtocolMetadata` trait。

## 能力描述符

```rust
impl ProtocolMetadata for ShadowsocksProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ProtocolCapabilityDescriptor {
            protocol: "shadowsocks",
            feature: "shadowsocks",
            compatibility_baseline: "shadowsocks_rust_sip022",
            status: ProtocolCapabilityLevel::Partial,
            inbound: DirectionalCapability {
                tcp: ProtocolCapabilityLevel::Supported,
                udp: ProtocolCapabilityLevel::Supported,
            },
            outbound: DirectionalCapability {
                tcp: ProtocolCapabilityLevel::Supported,
                udp: ProtocolCapabilityLevel::Supported,
            },
            transports: vec!["tcp", "udp"],
            mux: ProtocolCapabilityLevel::NotApplicable,
            limitations: vec![
                "shadowsocks_2022_tcp_header_is_not_implemented",
                "shadowsocks_2022_udp_server_response_context_is_not_implemented",
            ],
        }
    }
}
```

## 状态解释

| 字段 | 值 | 理由 |
|------|-----|------|
| `status` | `partial` | TCP/UDP 基线完成，但 AEAD 2022 TCP 和 server-side UDP response 仍有缺口 |
| `inbound.tcp` | `supported` | 常规 AEAD + AEAD 2022 TCP inbound accept |
| `inbound.udp` | `supported` | UDP datagram framing for inbound |
| `outbound.tcp` | `supported` | 全 cipher TCP outbound |
| `outbound.udp` | `supported` | 全 cipher UDP outbound (已外部验证) |
| `transports` | `["tcp", "udp"]` | Shadowsocks 不使用 transport 层抽象 |
| `mux` | `unsupported` | Shadowsocks 未实现 MUX |

## Limitations

| Limitation 码 | 含义 |
|---------------|------|
| `shadowsocks_2022_tcp_header_is_not_implemented` | AEAD 2022 TCP 仍使用 AEAD stream wrapper，未实现 SIP022 TCP header |
| `shadowsocks_2022_udp_server_response_context_is_not_implemented` | AEAD 2022 UDP server response 需要 session control state |

## 外部互操作状态

- UDP outbound 已完成与 `shadowsocks-rust ssserver -U` 的所有 6 个 cipher 外部互通
- Xray/sing-box/shadowsocks-rust 外部互操作测试文件：`crates/proxy/tests/shadowsocks_xray_interop.rs`（10 个测试，本地手动执行）
- AEAD 2022 TCP 外部互通和 AEAD 2022 UDP server 互通尚未完成
