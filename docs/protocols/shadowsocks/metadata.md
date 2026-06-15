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
                "shadowsocks_2022_hardening_not_externally_validated",
                "shadowsocks_2022_udp_relays_target_keyed_not_session_id",
            ],
        }
    }
}
```

## 状态解释

| 字段 | 值 | 理由 |
|------|-----|------|
| `status` | `partial` | SIP022 全部章节已实现；剩余为安全加固外部验证与 UDP 按 session id 路由 |
| `inbound.tcp` | `supported` | 常规 AEAD + AEAD 2022 TCP inbound accept（SIP022 头部+检测防御+重放） |
| `inbound.udp` | `supported` | UDP datagram framing for inbound（含 2022 UDP server response + 滑动窗口） |
| `outbound.tcp` | `supported` | 全 cipher TCP outbound |
| `outbound.udp` | `supported` | 全 cipher UDP outbound (已外部验证) |
| `transports` | `["tcp", "udp"]` | Shadowsocks 不使用 transport 层抽象 |
| `mux` | `unsupported` | Shadowsocks 未实现 MUX |

## Limitations

| Limitation 码 | 含义 |
|---------------|------|
| `shadowsocks_2022_hardening_not_externally_validated` | SIP022 全部 spec 章节已实现并通过内置测试；但新的检测防御/drain 与滑动窗口尚未对抗真实主动探测/重放攻击完成外部验证 |
| `shadowsocks_2022_udp_relays_target_keyed_not_session_id` | SIP022 3.2.4 要求按客户端 session id 路由；Zero 的 UDP 调度按 `(target, port)` 复用流，并发同目标客户端可能交叉路由 |

## 外部互操作状态

- UDP outbound 已完成与 `shadowsocks-rust ssserver -U` 的所有 6 个 cipher 外部互通
- AEAD 2022 TCP **入站**已通过 `shadowsocks-rust` 参考客户端 `sslocal` 端到端互操作验证（HTTP 200）
- AEAD 2022 TCP **出站**管线已通过 Zero→Zero 端到端验证（外部 `ssserver` 在此环境有 Windows 单次读取缺陷，已通过参考对对照测试排除 Zero 自身缺陷）
- AEAD 2022 **UDP server response** 已通过手动探针验证（DNS 往返 + 滑动窗口重放拒绝）：`protocols/shadowsocks/examples/udp_server_probe.rs`
- 新增的检测防御/drain 与滑动窗口对抗真实主动探测/重放攻击的外部验证尚未完成
