# VMess Metadata

对应 `protocols/vmess/src/metadata.rs` — `VmessProtocol` 实现 `ProtocolMetadata` trait。

## 能力描述符

```rust
impl ProtocolMetadata for VmessProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ProtocolCapabilityDescriptor {
            protocol: "vmess",
            feature: "vmess",
            compatibility_baseline: "xray_core_vmess_aead",
            status: ProtocolCapabilityLevel::Partial,
            inbound: DirectionalCapability {
                tcp: ProtocolCapabilityLevel::Partial,
                udp: ProtocolCapabilityLevel::Partial,
            },
            outbound: DirectionalCapability {
                tcp: ProtocolCapabilityLevel::Partial,
                udp: ProtocolCapabilityLevel::Partial,
            },
            transports: vec!["tcp", "tls", "ws", "grpc"],
            mux: ProtocolCapabilityLevel::Partial,
            limitations: vec!["external_interop_coverage_is_incomplete", "cipher_zero_mainstream_compatibility_is_incomplete"],
        }
    }
}
```

## 状态解释

| 字段 | 值 | 理由 |
|------|-----|------|
| `status` | `partial` | 基线实现完成，主流传输组合生产可用，外部互通三大家族覆盖；`cipher: zero` 主流兼容缺失，保留 partial |
| `inbound.tcp` | `partial` | Raw TLS + WS + gRPC 入站；外部互操作覆盖不足 (`external_interop_coverage_is_incomplete`) |
| `inbound.udp` | `partial` | CMD_UDP 入站 (packet + raw datagram)；外部互操作覆盖不足 |
| `outbound.tcp` | `partial` | 全 cipher + 全 transport 出站 + MUX 连接池；外部互操作覆盖不足 |
| `outbound.udp` | `partial` | 单跳 + 同协议 relay chain；外部互操作覆盖不足 |
| `transports` | `["tcp", "tls", "ws", "grpc"]` | VMess 主流传输基线 |
| `mux` | `partial` | Mux.Cool TCP + UDP 子连接；外部互操作覆盖不足 |
| `limitations` | `["external_interop_coverage_is_incomplete", "cipher_zero_mainstream_compatibility_is_incomplete"]` | 外部互操作测试未覆盖全部传输组合；`cipher: zero` 主流实现 (Xray inbound) 拒绝该安全类型 |

## 剩余缺口

| 缺口 | 影响 |
|------|------|
| `cipher_zero_mainstream_compatibility_is_incomplete` | `cipher: zero` 仅 Zero-to-Zero 可用；Xray inbound 拒绝 `zero` 安全类型。GUI 不应将 `zero` 作为主流默认选项暴露。这是 VMess 从 `partial` 提升到 `supported` 的唯一缺口。 |
| `external_interop_coverage_is_incomplete` | 针对基线上游实现的端到端测试尚不够完整，无法宣称所有高级路径生产可用。已验证路径（Xray raw TLS/WS/gRPC、sing-box、Mihomo）的证据不应推广到未测试的传输组合（H2、HTTPUpgrade）。 |

## Relay-chain Boundary

当前 VMess chain 仅同协议：`vmess → vmess`。SOCKS5/Mixed 入口作为客户端 ingress，不构成跨协议 relay chain。

## 外部互操作状态

已验证路径（不应推广到未测试的 transport 组合）：
- Xray 双向 TCP (`aes-128-gcm`/`none`) + WS+TLS + gRPC+TLS + UDP
- sing-box Zero-outbound TCP+UDP
- Mihomo outbound TCP (`auto`) + UDP (`CMD_UDP` raw datagram)
- 互操作测试文件：`crates/proxy/tests/vmess_xray_interop.rs`（本地手动执行）
