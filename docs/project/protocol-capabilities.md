# 协议能力

本文档记录内核暴露的当前协议能力范围。它描述实现事实，而非发布历史。

外部对接应优先使用机器可读的 `capabilities` 响应。此处的表格以人类可读的方式解释相同模型。

`zero-api` 定义协议能力的传输模型。具体协议事实由代理运行时协议清单针对当前二进制填充。`zero-engine` 报告通用管控面能力，不维护协议能力矩阵。

`zero-traits::protocol` 定义中性描述符和行为边界：`ProtocolCapabilityDescriptor`、`ProtocolMetadata`、`TcpTunnelProtocol`、`DeferredTcpTunnelProtocol`、`TcpSessionProtocol`、`UdpRelayProtocol`、`UdpPacketTunnelProtocol`、`UdpPacketFraming`、`UdpPacketStreamFraming` 和 `UdpDatagramFraming`。每个协议 crate 暴露其自己的 `ProtocolMetadata` 实现（例如 `socks5::Socks5Protocol`、`trojan::TrojanProtocol`），代理适配器委托给它。协议 crates 实现行为特征，将握手、会话状态、帧封装、流数据包边界、datagram 数据包边界和关联细节隐藏在协议无关特征之后。

## 能力字段

每个协议能力使用 snake_case 字段名称和值：

| 字段 | 含义 |
|------|---------|
| `protocol` | 配置和状态导出中使用的协议或内核动作名称 |
| `feature` | 编译该协议所需的 Cargo feature，或 `core` |
| `compiled` | 该二进制文件是否已编译此 feature |
| `status` | 总体支持级别：`supported`、`partial` 或 `experimental` |
| `compatibility_baseline` | 用作基线的上游协议文档或实现族 |
| `inbound.tcp` / `inbound.udp` | 协议是否可以接受 TCP 或 UDP 流 |
| `outbound.tcp` / `outbound.udp` | 协议是否可以创建 TCP 或 UDP 上游流 |
| `transports` | 协议适配器支持的传输名称 |
| `mux` | MUX 支持状态 |
| `limitations` | 机器可读的限制代码 |

`CapabilityState.level` 值为：

| 级别 | 含义 |
|------|---------|
| `supported` | 作为正常内核能力实现 |
| `partial` | 已实现，但有记录的缺口或不完整的互操作性覆盖 |
| `experimental` | 代码中存在，但不足以用于生产环境的默认假设 |
| `unsupported` | 此方向/网络未实现 |
| `not_applicable` | 协议不定义此方向/网络 |

`status` 与基线可用性不同。一个协议可能已实现其基线 TCP/UDP 路径，但在外部互操作性覆盖、MUX 行为、指纹行为或特殊链传输路径不完整时仍保持 `partial`。GUI 和管控面消费者应使用方向字段和 `limitations` 来获得精确行为，而不仅是顶层 `status`。

## 完备性术语

能力矩阵使用精确术语：

| 术语 | 含义 |
|------|---------|
| 基线完备 | 正常配置的 TCP/UDP 路径已通过路由、会话、统计、事件、运行时分发和能力导出连接 |
| `supported` | 协议或内核动作是正常的稳定内核能力，没有已知的协议级别限制 |
| `partial` | 基线路径可以实现，但互操作性、MUX、指纹或特殊链传输路径仍有记录的缺口 |
| 生产级完备 | 基线路径、上游互操作性、高级协议选项和有记录的链路径全部覆盖 |

当前矩阵不应被解读为"所有协议都完备"。它表示基线代理路径在明确列出的位置存在，而顶层 `status` 和 `limitations` 仍然定义真正的外部契约。

## 当前矩阵

| 协议 | 状态 | 入站 TCP | 入站 UDP | 出站 TCP | 出站 UDP | MUX | 基线 |
|---------|--------|-------------|-------------|--------------|--------------|-----|----------|
| `direct` | `supported` | `supported` | `unsupported` | `supported` | `supported` | `not_applicable` | `kernel_builtin` |
| `block` | `supported` | `unsupported` | `unsupported` | `supported` | `supported` | `not_applicable` | `kernel_builtin` |
| `socks5` | `supported` | `supported` | `supported` | `supported` | `supported` | `not_applicable` | `rfc_1928_rfc_1929` |
| `http` | `supported` | `supported` | `not_applicable` | `unsupported` | `not_applicable` | `not_applicable` | `rfc_7231_connect` |
| `mixed` | `supported` | `supported` | `supported` | `unsupported` | `unsupported` | `not_applicable` | `kernel_builtin` |
| `vless` | `partial` | `supported` | `partial` | `supported` | `partial` | `partial` | `xray_core_vless` |
| `hysteria2` | `partial` | `supported` | `partial` | `supported` | `partial` | `unsupported` | `hysteria` |
| `shadowsocks` | `partial` | `supported` | `supported` | `supported` | `supported` | `unsupported` | `shadowsocks_rust_sip022` |
| `trojan` | `partial` | `supported` | `partial` | `supported` | `partial` | `unsupported` | `trojan_go` |
| `vmess` | `partial` | `partial` | `partial` | `partial` | `partial` | `partial` | `xray_core_vmess_aead` |
| `mieru` | `supported` | `supported` | `supported` | `supported` | `supported` | `unsupported` | `mieru` |

## 内核缺口

主要协议缺口：

- `udp_relay_final_hop_not_externally_validated`: VLESS UDP 中继链支持 TCP 中继前缀，最终跳可包装已建立的 TCP 中继流：原始 TCP、TLS、Reality、WebSocket、gRPC、H2、HTTP Upgrade 和 XHTTP。XHTTP `stream-one`（默认 `auto`）单连接模式使 SplitHTTP/XHTTP 可作为 relay-chain 最终跳——此前 SplitHTTP 需双连接（POST+GET）而 relay 前缀仅提供单流，故无法作为最终跳；现经 `stream-one` 在单流上承载上行分块（POST body）与下行分块（response body）即解决。QUIC 因 XTLS 已弃用独立 VLESS QUIC 传输且需非 TCP 载体，不支持作为 UDP 中继链最终跳。该 stream-one 路径已有内部 e2e 覆盖（`relays_udp_through_socks5_to_vless_xhttp_stream_one_relay_chain`，验证 UDP over stream-one relay 最终跳），尚未与上游 Xray 服务器完成外部互通验证。
- `external_interop_coverage_is_incomplete`: 内置数据包处理存在，但针对基线上游实现的端到端测试不足以将每个高级路径称为生产级兼容。对于 VMess，TCP 和 UDP 基线互操作性已覆盖：Xray 双向（原始 TLS `aes-128-gcm`/`none`、WS+TLS、gRPC+TLS）、Zero 出站到 sing-box 入站（TCP+UDP）、Mihomo 出站到 Zero 入站（TCP `auto`+UDP `CMD_UDP` 原始 datagram）。这些路径的证据不得推广到未测试的传输组合（如 H2、HTTPUpgrade、XHTTP `stream-one`）。
- `shadowsocks_2022_hardening_not_externally_validated`: Shadowsocks AEAD 2022 (SIP022) **全部 spec 章节已实现并通过内置测试**（3.1.1 加密/nonce、3.1.2 格式、3.1.3 头部+检测防御、3.1.5 重放保护、3.2 UDP 含 3.2.4 滑动窗口）。SIP022 3.2.4 的按客户端 session id 隔离 UDP 中继流已实现：`UdpFlowKey` 增加了可选 `client_session_id` 维度，SS 2022 inbound 将客户端 SIP022 session id 传入 UDP 调度层，不同客户端 session id 到同一 `(target, port)` 会建立独立的出站流。具体：TCP 请求/响应头部、固定+变量头、30 秒时间戳窗口、请求 salt 回填校验、padding、SIP022 3.1.5 的 60 秒服务端重放 salt 池、SIP022 3.1.3 的单次读取+失败时 drain 检测防御、SIP022 3.2.4 的每会话 UDP 滑动窗口重放过滤、AEAD 2022 UDP 服务端响应（回填客户端 session id）。验证覆盖：TCP 入站方向已通过 `shadowsocks-rust` 参考客户端 (`sslocal`) 端到端互操作性、TCP 出站管线已通过 Zero→Zero、AEAD 2022 UDP 服务端响应已通过手动探针（DNS 往返）。尚未完成外部验证的部分：新的检测防御/drain 与滑动窗口对抗真实主动探测/重放攻击的行为，以及与未损坏的外部 `ssserver` 的直接互操作（此环境下的 `ssserver` 单次读取存在 Windows 环境缺陷，已通过参考对对照测试排除 Zero 自身缺陷）。
- `vless_quic_transport_deprecated_by_xtls`: XTLS 已移除独立 VLESS `quic` 传输，其继任者为 XHTTP `stream-one` over H3。项目保留 `quic` 配置字段以向后兼容，但 metadata `transports` 不再列出 `quic`，且不作最终跳推荐。

## 基线完备

基线代理范围已为下列方向实现。这意味着这些已配置的 TCP/UDP 方向已通过路由、会话、统计、事件和运行时分发连接。这不意味着每个高级协议选项或每个外部实现兼容性案例都完备。

| 协议 | 基线状态 | 为何 status 可能保持在 `supported` 以下 |
|----------|----------------|------------------------------------------|
| `direct` | 完备 | 无剩余协议缺口 |
| `block` | 完备 | 无剩余协议缺口 |
| `socks5` | 完备 | 无剩余协议缺口 |
| `http` | 完备 | UDP 不适用 |
| `mixed` | 完备 | Mixed 是内核入站多路复用器：SOCKS5 TCP CONNECT 和 UDP ASSOCIATE 使用 SOCKS5 运行时路径；HTTP CONNECT 使用 HTTP TCP 运行时路径 |
| `vless` | TCP 和 UDP-over-stream 基线路径完备 | UDP MUX outbound 尚未接入 VlessUdpOutboundManager；XHTTP `stream-one` 最终跳路径尚未与上游 Xray 完成外部互通验证；QUIC 传输已被 XTLS 弃用 |
| `trojan` | TCP 和 UDP-over-stream 基线路径完备 | 外部互操作性覆盖不完整（中继流 TLS 指纹已支持，见 `relay_stream_tls_client_fingerprint_is_not_supported`） |
| `shadowsocks` | 普通 AEAD TCP 和 UDP datagram 路径完备，包括 Shadowsocks UDP over SOCKS5、大 TCP 载荷分块、错误密码拒绝、数据包路径中继链以及针对 `shadowsocks-rust` 的本地外部 UDP 出站互操作性（覆盖所有支持的 cipher）；AEAD 2022 (SIP022) **spec 全部章节已实现**——TCP 请求/响应头部协议（固定+变量头、30 秒时间戳窗口、请求 salt 回填校验、padding）、SIP022 3.1.3 检测防御（salt+固定头单次读取 + 失败时 drain）、SIP022 3.1.5 服务端重放 salt 池（60 秒）、SIP022 3.2.4 每会话 UDP 滑动窗口重放过滤 + 按客户端 session id 隔离 UDP 中继流（`client_session_id` 传入 UDP 调度层，不同客户端到同一 target 不复用出站流）、AEAD 2022 UDP 服务端响应（回填客户端 session id），覆盖三个 blake3 cipher 双向基线路径；TCP 入站方向已通过 `shadowsocks-rust` 参考客户端 (`sslocal`) 端到端互操作性验证，AEAD 2022 UDP 服务端响应已通过手动探针（DNS 往返）验证，TCP 出站管线已通过 Zero→Zero 端到端验证 | 新增的检测防御/drain 与滑动窗口尚未对抗真实主动探测/重放攻击完成外部验证；TCP 出站方向尚未在未损坏的外部 `ssserver` 上完成端到端验证（此环境下的 `ssserver` 单次读取有 Windows 环境缺陷，已通过参考对对照测试排除 Zero 自身缺陷） |
| `hysteria2` | QUIC TCP 流和 UDP datagram 基线路径完备 | 外部互操作性覆盖不完整（QUIC UDP 链载体已实现，见 `udp_relay_chain_quic_path_not_supported`） |
| `mieru` | TCP 流和 UDP associate 基线路径完备；出站 TCP/UDP 已与外部 mita 互通验证，入站经 `protocols/mieru/tests/loopback.rs` 对已验证出站配对验证 | 无剩余协议缺口 |
| `vmess` | 基线 TCP 握手、TCP/UDP MUX、UDP-over-stream、同协议 `vmess -> vmess` UDP 中继链和 body relay 已针对内置运行时实现；原始 TLS、WSS、gRPC、`cipher: auto` 规范化、`cipher: none` / `cipher: zero`、本地 TCP MUX、本地 MUX UDP、本地 UDP 单跳中继和本地同协议 UDP 中继链具有内置覆盖；body AEAD 支持认证长度、块掩码（SHAKE128）、全局填充和定期密钥旋转（2^14 块）；外部 TCP 和 UDP 基线互操作性已覆盖：Xray 双向、Zero 出站到 sing-box 入站、Mihomo 出站到 Zero 入站；Xray WS/gRPC TCP 传输互操作性已覆盖双向 | 外部互操作性覆盖和主流 `cipher: zero` 兼容性仍不完整 |

## 当前收口状态

1. 运行时 `capabilities.protocols` 是 GUI 和管控面消费者的外部权威来源。
2. `zero-api` 仅拥有传输模型。它不拥有协议事实或协议行为。
3. `zero-engine` 暴露通用管控面能力。它不维护协议能力矩阵。
4. 外部协议描述符位于其协议 crates 中：
   - `socks5::Socks5Protocol`
   - `http::HttpConnectProtocol`
   - `vless::VlessProtocol`
   - `hysteria2::Hysteria2Protocol`
   - `shadowsocks::ShadowsocksProtocol`
   - `trojan::TrojanProtocol`
   - `vmess::VmessProtocol`
   - `mieru::MieruProtocol`
5. 内核动作保留在 `zero-proxy` 描述符映射中，因为它们不是外部协议 crates：`direct`、`block` 和 `mixed`。
6. `TcpTunnelProtocol` 针对仅需在已连接流上建立隧道的协议握手实现：
   - SOCKS5 TCP CONNECT
   - Trojan TCP 请求
   - VLESS 非 flow TCP 请求/响应
   - VLESS flow TCP 请求/响应（Vision/Reality，`reality` feature 启用时）
7. `TcpSessionProtocol` 针对返回会话/流状态的协议握手实现：
   - Shadowsocks TCP（返回 `ShadowsocksOutboundSession`，包含 AEAD key/nonce/cipher）
   - VMess TCP（返回 `VmessOutboundSession`，包含上传/下载 AEAD key/nonce/cipher）
   - Mieru TCP（返回 `MieruOutbound`，包含加密状态）
8. `DeferredTcpTunnelProtocol` 针对必须立即写入请求并将响应验证推迟到流封装器的协议握手实现：
   - VLESS flow TCP ???? Reality ?????????????????????????? `DeferredVlessResponseStream` ??? `protocols/vless::outbound` ???
9. `UdpRelayProtocol` 针对 SOCKS5 UDP ASSOCIATE 实现。协议 crate 拥有认证协商和关联响应解析；代理拥有控制流拨号、UDP socket 绑定、中继端点解析、关联缓存、空闲超时、统计、事件和 fallback 行为。
10. `UdpPacketTunnelProtocol` 和 `UdpPacketFraming` 针对 VLESS UDP over 已建立流实现。VLESS crate 拥有 UDP 隧道请求/响应握手和 VLESS UDP 数据包编码/解码；代理拥有传输设置、中继前缀设置、路由、fallback、会话生命周期、统计、事件和通用响应任务调度。UDP 中继链针对 TCP 中继前缀和能够在已建立 TCP 流上操作的 VLESS 最终跳传输实现。
11. `UdpPacketTunnelProtocol` 和 `UdpPacketFraming` 针对 VMess UDP over 已建立流实现。VMess crate 拥有 `CMD_UDP` 请求、AEAD 流状态、UDP 数据包编码/解码和 payload 模式选择；代理拥有传输设置、路由、会话生命周期、统计、事件、上游缓存和通用响应任务调度。已实现的 VMess UDP 中继链目标是同协议 `vmess -> vmess` 路径。本地 SOCKS5/Mixed 仅提供客户端入口数据包，不计为跨协议链支持。
12. `UdpPacketTunnelProtocol` 和 `UdpPacketStreamFraming` 针对 Trojan UDP over 已建立 TLS 流实现。Trojan crate 拥有 `CMD_UDP` 请求和长度前缀 UDP 数据包读写行为；代理拥有 TLS 设置、中继前缀设置、上游缓存、任务调度、路由、fallback、会话生命周期、统计和事件。UDP 中继链针对 TCP 中继前缀和 Trojan TLS 最终跳实现。
13. `UdpDatagramFraming` 针对 Shadowsocks UDP datagram 实现。Shadowsocks crate 拥有目标数据编码、盐生成、AEAD/2022 KDF 选择、UDP 加密、UDP 解密、AEAD 2022 客户端数据包头部处理和目标数据解析。代理拥有 UDP socket、上游缓存、响应匹配、路由、fallback、会话生命周期、统计和事件。UDP 中继链使用通用 datagram-over-packet-path 模型：`UdpPacketPath` 载体承载 `DatagramCodec` 编码的 datagram。已实现的 Shadowsocks 最终跳载体为 SOCKS5 UDP ASSOCIATE 和 Shadowsocks UDP。添加新组合需要实现这两个特征，而不是创建按协议对特定的模块。
    Shadowsocks TCP 入站 accept 返回 `ShadowsocksAccept`，协议 crate 拥有 AEAD 流封装器、服务器到客户端响应盐生成、下载密钥派生、块加密和块解密。代理拥有监听器生命周期、认证归因、TCP 管道入口、路由、计量、会话生命周期、统计和事件。
    内置验证覆盖所有支持的 Shadowsocks TCP cipher、大 TCP 载荷分块、错误密码 TCP 拒绝、SOCKS5-to-Shadowsocks UDP 中继、所有支持的 Shadowsocks UDP cipher 和基于已实现数据包路径载体的 Shadowsocks UDP 中继链。本地外部验证覆盖 SOCKS5 UDP ASSOCIATE 通过 Shadowsocks 出站到 `shadowsocks-rust ssserver -U`，覆盖所有支持的 cipher，包括 AEAD 2022 AES-GCM 和 AEAD 2022 XChaCha20Poly1305 UDP 数据包格式。
    AEAD 2022 TCP 仍缺少 SIP022 TCP 请求/响应头部协议。AEAD 2022 UDP 服务器端响应仍需要状态化响应上下文，Zero 才能作为完全兼容的外部 AEAD 2022 UDP 服务器运行。
14. `UdpDatagramFraming` 针对 Hysteria2 UDP datagram 载荷实现。Hysteria2 crate 拥有 UDP datagram 目标编码/解码；代理拥有 QUIC 连接设置、认证、UDP datagram 发送/接收、路由、fallback、会话生命周期、统计和事件。Hysteria2 还提供 `UdpPacketPath` 载体，用于 packet-path relay chain；当前落点是 `[Hysteria2, Shadowsocks]`。Hysteria2 使用代理运行时中的传输特定 connector，因为 QUIC 连接设置与协议协商集成，不分解为流级握手。
15. Mieru TCP 使用 `TcpSessionProtocol` 进行加密流会话设置。在 TCP 中继链中，Mieru 可以作为中间跳，因为代理运行 Mieru 会话握手并在应用下一跳之前用 `MieruTcpStream` 包装中继流。`UdpPacketFraming` 针对 Mieru UDP associate 封装实现。Mieru UDP 通过加密 Mieru 流集成在代理 UDP 分发路径中；协议 crate 拥有 Mieru 段加密/解密状态和 UDP associate 帧封装，而代理拥有路由、中继前缀设置、上游缓存、任务调度、统计和事件。UDP 中继链针对 TCP 中继前缀和 Mieru 作为最终跳实现。

## 剩余工作

1. 保持运行时分发聚焦于路由、生命周期、统计、事件导出、fallback、健康检查和背压。
2. 通过为新载体实现 `UdpPacketPath` 和为新的内部协议实现 `DatagramCodec` 来扩展 UDP 链数据包路径支持。Hysteria2 QUIC UDP 路径载体已实现（`Hysteria2PacketPath`）。
3. 在将任何 `partial` 协议提升到 `supported` 之前，添加兼容性基线的上游互操作性测试。
4. VMess 现在为 `partial`：基线 TCP/UDP/MUX 路径已实现，在主流传输组合（原始 TLS、WS+TLS、gRPC+TLS）上具有生产级能力。`cipher: zero` 仍为已记录的限制，除非有基线实现接受它，否则应保持在主流 GUI 默认值之外。兼容性声明不得扩展到实际测试的外部路径之外。
