# Trojan Metadata

对应 `protocols/trojan/src/metadata.rs` — `TrojanProtocol` 实现 `ProtocolMetadata` trait。

## 能力描述符

```rust
impl ProtocolMetadata for TrojanProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ProtocolCapabilityDescriptor {
            protocol: "trojan",
            feature: "trojan",
            compatibility_baseline: "trojan_go",
            status: ProtocolCapabilityLevel::Partial,
            inbound: DirectionalCapability {
                tcp: ProtocolCapabilityLevel::Supported,
                udp: ProtocolCapabilityLevel::Partial,
            },
            outbound: DirectionalCapability {
                tcp: ProtocolCapabilityLevel::Supported,
                udp: ProtocolCapabilityLevel::Partial,
            },
            transports: vec!["tcp", "tls"],
            mux: ProtocolCapabilityLevel::Unsupported,
            limitations: vec![],
        }
    }
}
```

## 状态解释

| 字段 | 值 | 理由 |
|------|-----|------|
| `status` | `partial` | TCP 基线完成，UDP partial，MUX unsupported |
| `inbound.tcp` | `supported` | TLS ingress + Trojan request |
| `inbound.udp` | `partial` | UDP-over-stream 入站 |
| `outbound.tcp` | `supported` | TLS + Trojan request 出站 |
| `outbound.udp` | `partial` | 单跳 + TCP relay-prefix final-hop |
| `transports` | `["tcp", "tls"]` | |
| `mux` | `unsupported` | Trojan MUX 不在范围内 |

## 剩余缺口

- `external_interop_coverage_is_incomplete`
- `relay_stream_tls_client_fingerprint_is_not_supported`
- `mux_udp_is_not_implemented` (Trojan 无 MUX)

## 外部互操作

互操作测试文件：`crates/proxy/tests/trojan_xray_interop.rs`（8 个测试，Xray/sing-box/Mihomo 三大族，本地手动执行，`#[ignore]`）。
