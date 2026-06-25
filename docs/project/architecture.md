# 架构

仓库可以从以下分层来理解。

## 应用层

- 根 crate `zero`
- `zero-api`

负责 CLI 参数、配置文件路径、进程启动和状态输出。

管控面和可观测性模型遵循 Zero 自身约定。Clash、sing-box 和 Xray 等外部生态系统仅作为设计参考对待；兼容性适配层应存在于适配器、网关或外部工具中，不应约束内核或长期 API。

`zero-api` 定义外部控制、观测和事件导出能力。它不等同于 HTTP 服务，能力也不按传输线划分。HTTP/HTTPS、本地 IPC、文件、gRPC、二进制帧封装、Rust API 和 FFI 都作为特征实现或 feature-gated 适配器/sink 依附于同一核心能力集。

## 配置、决策和状态层

- `zero-config`
- `zero-engine`
- `zero-router`

`zero-config` 拥有配置模型和解析。`zero-router` 拥有规则匹配。`zero-engine` 拥有将配置编译为可执行计划、路由决策、目标解析、模式和组状态、会话、统计、事件和状态导出。

模式语义（`direct / global / rule`）和出站组语义（`selector / url_test / fallback`）也属于此层，不属于客户端。

`zero-engine` 不绑定 Tokio，不启动监听器，不持有协议实现，也不直接建立 socket 连接。`direct` / `block` 是此层内内置的目标语义；实际的网络执行由代理运行时层执行。

`zero-engine` 当前跨越多个决策边界运行：

- `RuntimeConfig`
  - 来自 `zero-config` 的输入
- `EnginePlan`
  - 不可变执行结构
- `EngineState`
  - 可变运行时状态，包括 `OutboundHealth`（熔断器）
- `view`
  - 用于 `status` / 导出 / 日志的标签渲染

热路径优先读取 plan/state 并通过借用边界传递引用；仅管控面和展示面回到字符串标签。

## 代理运行时层

- `zero-proxy`

`zero-proxy` 将 `zero-engine` 决策转化为实际代理执行：

- 启动入站监听器
- 调用协议实现进行握手和帧封装
- 建立直连或上游出站连接
- 运行 TCP 中继、UDP 关联、TLS、url_test 探测和熔断器健康检查
- 验证当前构建已编译配置引用的协议 features

此层可以依赖 Tokio 后端、协议 crates 和 `zero-engine`。它不重新解释配置语义，也不维护独立的模式、组或路由状态集。

### Adapter dispatch boundary

`ProtocolAdapter` is the compatibility runtime dispatch boundary for UDP establishment.
Inbound bind/spawn dispatch is split out into explicit `InboundListenerCapability` implementations on each registered adapter.
TCP outbound dispatch is split out into explicit `TcpOutboundCapability` implementations on each registered adapter.
Focused capability traits (`ProtocolSupportCapability`, `InboundListenerCapability`, `TcpOutboundCapability`, `UdpFlowCapability`, and `UdpPacketPathCapability`) sit in front of the remaining compatibility trait.
Metadata and feature/support checks live in explicit `ProtocolSupportCapability` implementations, not on the monolithic adapter trait.
These capability entrypoints receive narrow adapter context values (`InboundAdapterContext`, `OutboundAdapterContext`, `UdpAdapterContext`) instead of exposing the full `Proxy` parameter in the trait surface; protocol implementations can still use the context as a migration bridge while runtime dependencies are reduced.
`ProtocolRegistry` stores registered capability objects; the monolithic adapter trait is only the registration compatibility source.
`zero-proxy` runtime orchestration does not match on `InboundProtocolConfig` or `ResolvedLeafOutbound` to select a protocol path.
Adding a protocol means registering an adapter and adding protocol-local inbound/outbound code.

Inbound listener entrypoints live as module functions under `crates/proxy/src/inbound/`.
Adapters call `crate::inbound::run_<protocol>_listener_with_bound`; `Proxy` does not own `run_*_listener_with_bound` methods.
`mixed` remains an inbound multiplexor rather than an external protocol, but it is registered through `MixedAdapter` so reload and spawn use the same adapter path as other inbounds.

`zero-proxy` keeps facade roots thin:

- `adapters/mod.rs` only declares concrete adapter modules and re-exports adapter types. Registry construction and protocol dispatch stay outside this facade.
- `inbound/mod.rs` only declares inbound listener modules and re-exports `run_<protocol>_listener_with_bound` entrypoints. Request models and listener/session logic stay in protocol-local inbound modules.
- `outbound/mod.rs` only declares crate-private per-protocol outbound helper modules. Helper logic lives in `outbound/<protocol>.rs` and is called only by adapter TCP modules.
- `protocol_adapter.rs` only re-exports the crate-private adapter trait, focused capability traits, adapter contexts, adapter models, and registry.
- `protocol_adapter/defaults.rs` only wires adapter default helper modules. TCP bind defaults live in `defaults/bind.rs`; unsupported error construction lives in `defaults/errors.rs`.
- `protocol_adapter/model.rs` only wires adapter model modules. Inbound bind/spawn models live in `model/inbound.rs`; outbound runtime facts live in `model/outbound.rs`.
- `protocol_adapter/registry.rs` only owns the registry struct and submodule wiring. The low-level register helper lives in `registry/build.rs`; compiled protocol collection lives in `src/register.rs`; inbound dispatch, outbound dispatch, metadata, support lookup, and validation live in `registry/{inbound,outbound,metadata,support,validation}.rs`.
- `protocol_adapter/registry/tests.rs` only wires registry test modules. Shared fixtures live in `registry/tests/fixtures.rs`; inbound coverage lives in `registry/tests/inbound.rs`; outbound and `block` kernel fact coverage live in `registry/tests/outbound.rs`.
- `inventory.rs` only owns the runtime-facing `ProtocolInventory` shell. Inbound, TCP, UDP, metadata, direct connector access, and runtime-fact lookups live in sibling modules.
- `inventory/protocols.rs` exposes only neutral proxy-owned protocol helpers such as the direct connector; concrete protocol object construction stays in protocol-local integration code or the compiled registration surface.
- `inventory/udp.rs` only wires UDP inventory submodules. Single-hop leaf dispatch, relay final-hop dispatch, and packet-path adapter probing live in `inventory/udp/{leaf,relay,packet_path}.rs`.

Runtime modules depend on `ProtocolInventory` operations, not adapter trait object lookup. Adapter resolution stays behind `ProtocolInventory` and `ProtocolRegistry`; protocol-private fields stay with each adapter. Protocol identity and cipher config parsing is adapter-owned: runtime modules receive validated protocol values, or opaque adapter-built keys when they need stable cache identity.

UDP runtime flow snapshots use path categories rather than protocol variants. `runtime::udp_flow` records `Direct`, `Relay`, `Datagram`, and `StreamPacket` flow shape plus endpoint/session metadata. Protocol-specific snapshot data, packet-path carriers, parsed cipher values, and cache keys live under `protocol_runtime::udp`; `runtime::udp_dispatch` tracks externally managed protocol flows through neutral managed-flow state.

`runtime.rs` owns the `Proxy` shell and run loop. Control-plane handle details live in `runtime/handle.rs`, the spawned runtime handle lives in `runtime/running.rs`, and reload channel bridging lives in `runtime/reload.rs`.

Per-protocol outbound TCP helpers under `src/outbound/<protocol>.rs` are adapter implementation details. Only the owning `src/adapters/<protocol>/tcp.rs` module calls them; generic runtime and protocol-runtime modules dispatch through `ProtocolInventory` and `TcpOutboundCapability`.

UDP packet-path cache identity is also adapter-owned. Packet-path runtime may store carrier `cache_key`, datagram `datagram_cache_key`, and parsed protocol values such as `CipherKind`; it must not reconstruct cache identity from raw protocol-private config strings such as Shadowsocks cipher names.

Protocol stream/datagram codecs own protocol crypto/framing state. For example, Mieru inbound data-phase encryption/decryption lives in `protocols/mieru::MieruInboundDataCodec`, and Shadowsocks inbound UDP decode/replay/response encoding lives in `protocols/shadowsocks::ShadowsocksInboundUdpCodec`; `zero-proxy` only wraps those codecs as Tokio stream/socket adapters and must not hold their cipher/session primitives or build/parse protocol frames directly.

### 内核管道和入站协议管道

代理运行时围绕一个通用内核管道边界组织：

```text
KernelPipe -> TcpPipe / UdpPipe -> 协议特征 -> 协议 crates
```

`KernelPipe` 是运行时编排边界，不是协议特征。它依赖于运行时状态，如路由、会话、统计、事件、传输设置和任务生命周期，因此它属于 `zero-proxy`。

`TcpPipe` 拥有常规 TCP 流的 TCP 路由执行和出站建立。`UdpPipe` 拥有将 UDP 数据包提交到 UDP 运行时状态机的功能。`UdpDispatch` 仍然是 UDP 管道的内部状态持有者，用于 direct socket、上游关联、协议运行时状态聚合、响应任务、会话核算和 fallback 处理。

协议 crates 不实现 `KernelPipe`。它们实现协议无关的行为特征，用于握手、会话状态、流数据包帧封装或 datagram 帧封装。

### InboundProtocol 特征和 `serve_inbound()` TCP 生命周期

所有 TCP 协议入站处理程序通过单一特征和单一内核入口点统一。

**`InboundProtocol` 特征** -- 协议-服务器边界：

```rust
#[async_trait]
pub trait InboundProtocol: Send + Sync {
    type ClientStream: AsyncRead + AsyncWrite + Unpin + Send;

    async fn accept(&self, stream: TcpRelayStream) -> Result<(Session, Self::ClientStream), EngineError>;

    async fn send_ok(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn send_blocked(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn send_upstream_failure(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn relay(&self, client: Self::ClientStream, upstream: TcpRelayStream,
                   up_bps: Option<u64>, down_bps: Option<u64>) -> Result<(), EngineError>;
}
```

**协议实现者**（SOCKS5、HTTP CONNECT、VLESS、Hysteria2、Shadowsocks、Trojan、VMess、Mieru）仅实现此特征。每个处理程序提供：

- `accept` -- 认证并提取目标地址到 `Session`
- `send_ok` -- 通知客户端隧道已建立（协议特定响应）
- `send_blocked` -- 通知客户端请求被阻止（协议特定错误）
- `send_upstream_failure` -- 通知客户端上游不可达
- `relay` -- 双向中继；默认为原始 TCP `io::copy`，支持可选速率限制；可覆盖用于 AEAD 帧封装（Shadowsocks）或 QUIC 流（Hysteria2）中继

**`serve_inbound()`** 是常规 TCP 协议的单一 TCP 入站生命周期入口点。协议处理程序从不直接接触 engine、config 或 resolver。该函数拥有协议无关的 TCP 生命周期工作，并调用 `TcpPipe` 进行路由执行和出站建立：

1. **URL 重写** -- 在路由之前应用 `route.url_rewrite` 规则重写会话目标域名
2. **内核速率限制** -- 应用来自配置的按入站默认值（`up_bps` / `down_bps`）；`accept` 期间设置的按用户限制优先
3. **会话准备** -- `prepare_session`（引擎端元数据）
4. **路由并建立** -- `TcpPipe`（条件匹配 + 出站连接）
5. **协议回复** -- 视情况发送 `send_ok` / `send_blocked` / `send_upstream_failure`
6. **空闲超时** -- 使用 `InboundConfig.idle_timeout_secs`（默认 300s）通过 `tokio::time::timeout` 包裹中继
7. **会话生命周期** -- 用 `SessionOutcome` 跟踪 / 完成，结构化日志

添加新的跨切面 TCP 生命周期能力通常需要更改 `serve_inbound()` 或 `TcpPipe`；协议处理程序保持不变。

### 内核原语：熔断器

`zero-engine` 为每个出站标签维护 `OutboundHealth`。在连接到任何出站之前，TCP 管道的候选建立路径通过 `check_outbound_health()` 检查健康状态。如果在 30 秒滑动窗口内累积 5 次失败，该出站被隔离 60 秒。隔离期满后，允许一个探测连接；成功则清除不健康状态，失败则重置冷却期。

## 协议层

- `zero-core`
- `protocols/*`

`zero-core` 持有通用类型和接口。特定协议位于 `protocols/*` 下。

协议通过 feature gate 编译到 `zero-proxy` 中。核心决策层始终编译；协议和管控面能力选择性编译，避免拉入嵌入式场景不需要的模块。

暴露给 GUI 和管控面消费者的协议能力事实从代理运行时协议清单中为当前二进制填充。`zero-api` 定义传输模型；`zero-engine` 不维护协议矩阵。协议无关的描述符和行为特征位于 `zero-traits::protocol`，因此协议 crates 可以暴露元数据和 TCP/UDP 行为，而无需依赖 API 或代理运行时 crates。每个协议 crate 拥有其 `ProtocolMetadata` 描述符，并在握手语义匹配这些特征的地方实现协议行为特征。`TcpTunnelProtocol` 覆盖流级隧道握手，如 SOCKS5、Trojan 和 VLESS。`TcpSessionProtocol` 覆盖返回协议状态的握手，如 Shadowsocks、VMess 和 Mieru。`DeferredTcpTunnelProtocol` 覆盖立即写入请求并将响应验证推迟到流封装器的握手，如 VLESS Reality 单跳。`UdpRelayProtocol` 覆盖 UDP 中继关联握手，如 SOCKS5 UDP ASSOCIATE。`UdpPacketTunnelProtocol`、`UdpPacketFraming` 和 `UdpPacketStreamFraming` 覆盖 UDP-over-stream 协议。`UdpDatagramFraming` 覆盖在一个 UDP datagram 中承载一个完整协议数据包的协议，如 Shadowsocks UDP。VLESS 使用数据包帧封装处理隧道字节；Trojan 使用流帧封装处理长度前缀的 UDP 数据包；Shadowsocks 使用 datagram 帧封装。Mieru 和 Hysteria2 UDP 通过代理运行时 manager 集成，因为它们的会话状态与加密流或 QUIC 连接管理耦合。协议 crates 在可以干净分离的地方拥有数据包握手和帧封装语义，而代理拥有传输设置、socket 设置、路由、会话生命周期、统计、事件和响应桥接。

`TcpTunnelProtocol` 仅用于在同一流上返回已建立隧道的协议握手。`TcpSessionProtocol` 用于返回后续中继代码所需状态的协议握手。`DeferredTcpTunnelProtocol` 用于在建立期间消费响应会破坏中继路径所需的流语义的情况。`UdpPacketTunnelProtocol` 用于在已连接流上建立 UDP 数据包隧道，`UdpPacketFraming` 用于每个 datagram 的隧道字节。`UdpPacketStreamFraming` 用于数据包边界属于已连接流格式的协议。`UdpDatagramFraming` 用于直接在 UDP 上传输的协议 datagram。传输设置如 TLS、Reality、WebSocket、gRPC、H2、QUIC 和 HTTPUpgrade 保持在代理/传输层。

## 网络栈层

- `zero-stack`

实现 `TcpStack` / `UdpStack` / `NetworkStack` 特征（定义在 `zero-traits` 中），在原始 IP 数据包和 `AsyncRead + AsyncWrite` 流或 datagram I/O 之间转换。

两种实现共享同一特征：

| 栈 | 策略 | TCP 终止 | 所需驱动 |
|-------|----------|-----------------|---------------|
| `UserNetworkStack` | 用户空间 TCP 状态机 (`UserTcpStack`) | SYN/SYN-ACK/ACK 握手、seq 跟踪、MSS 协商、FIN/RST 处理 | TUN 设备 |
| `SystemStack` | OS 内核 TCP 监听器 (`SystemTcpStack`) | 委托给 OS 内核 | Linux/macOS 无需 |

该栈可插拔：TUN 入站处理程序消费 `NetworkStack`，因此 `UserStack` 与 `SystemStack` 的选择是配置决策；无需代码更改。

### 用户空间 TCP (zero-stack/src/tcp.rs)

`UserTcpStack` 为每个连接维护最小 TCP 状态机：
- **SYN -> SYN-ACK（包含 MSS 选项）**
- **ACK -> Established -> 数据传输**
- **FIN -> ACK -> CloseWait -> FIN-ACK -> 关闭**
- **RST -> 立即拆除**

载荷提取通过 mpsc channel 进入代理管道。响应数据包（SYN-ACK、ACK、FIN）通过出站 channel 发送到 TUN 设备写入任务。

### 系统 TCP (zero-stack/src/system.rs)

`SystemTcpStack` 包装 `tokio::net::TcpListener`；流量必须由 OS 重定向到此监听器：
- Linux：`iptables -t nat REDIRECT`
- macOS：`pf.conf rdr rule`
- Windows：需要 TUN 设备（wintun）或系统代理

## TUN 设备层

- `zero-tun`

平台无关的 `TunDevice` 特征（`AsyncRead + AsyncWrite`），用于虚拟网络接口：

| 平台 | 后端 | 依赖 |
|----------|---------|------------|
| Linux | `/dev/net/tun` ioctl | 内核内置 |
| macOS | utun socket | 内核内置 |
| Windows | Wintun 驱动 | `wintun.dll`（由 GUI/安装器部署） |

在 Windows 上，`wintun.dll` 是平台依赖，就像 Linux 需要 `/dev/net/tun` 和 macOS 需要 utun。内核 crate (`zero-tun`) 通过 `wintun` crate 声明依赖；DLL 到目标系统的部署是 GUI/安装器层的责任。

### TUN 入站 (zero-proxy/src/inbound/tun.rs)

TUN 入站处理程序从 `TunDevice` 读取原始 IP 数据包，喂入 `NetworkStack`，并通过 `serve_inbound()` 分发已建立的 TCP 连接：

```
TunDevice::read() -> 数据包 -> TcpStack::feed()
     ->                             -> 出站写入任务 -> SYN-ACK/ACK/FIN
     
TcpStack::accept() -> UserTcpStream -> serve_inbound()
```

UDP datagram 由内核 UDP 分发路径处理。分发层拥有路由决策、fallback 候选选择、会话生命周期、统计和 UDP 流的事件集成。按协议 UDP 支持由 `capabilities.protocols` 暴露。

出站流按 [`UdpPathCategory`]（Direct、Relay、Datagram、StreamPacket）分类并相应分发。UDP 中继链使用通用 [`UdpPacketPath`] + [`DatagramCodec`] 特征模型：上一跳提供数据包路径（发送/接收原始载荷），下一跳通过该路径编码其协议 datagram。添加新的链组合需要实现这两个特征，而不是创建按协议对模块。

## 传输层

- `zero-transport`

统一传输抽象：TLS、WebSocket、gRPC、H2、HTTPUpgrade、XHTTP（原 SplitHTTP，配置字段 `split_http`，含 `mode`）、QUIC（共享 H3 基座；VLESS 独立 QUIC 传输已被 XTLS 弃用）、Hysteria2 QUIC、VLESS 传输。还包含内核中继路径使用的共享 `RateLimiter`（GCRA）。TLS 客户端指纹（自定义 ClientHello，用于单跳和 relay-stream 最终跳）经 `zero-ztls` 自定义 TLS 1.3 客户端栈实现（从 REALITY 抽取，uTLS 级浏览器指纹匹配）。

## 支撑 Crates

- `zero-api` -- 管控面 API 类型
- `zero-connector` -- 事件分发 connector（JSONL sink、webhook、push）
- `zero-crypto` -- 加密基础设施（Reality TLS 1.3、密钥交换、证书操作）
- `zero-ztls` -- 通用 TLS 1.3 客户端实现，支持自定义 ClientHello（从 REALITY TLS 1.3 栈抽取），供 `zero-transport` 在单跳与 relay-stream 最终跳路径上做 TLS 客户端指纹匹配（uTLS 级浏览器指纹）
- `zero-logging` -- 结构化日志
- `zero-ffi` -- C 兼容嵌入式接口
- `zero-grpc` -- gRPC 管控面适配器（`grpc_api` feature）
- `zero-dns` -- DNS 子系统（system / UDP / DoH / DoT / Fake IP）

## 抽象层

- `zero-traits`

运行时无关的抽象：

| 特征 | 用途 |
|-------|---------|
| `AsyncSocket` / `TcpListener` / `DatagramSocket` | I/O |
| `TcpStack` / `UdpStack` / `NetworkStack` | 网络数据包到流/datagram 的转换 |
| `ProtocolMetadata` / `TcpTunnelProtocol` / `DeferredTcpTunnelProtocol` / `TcpSessionProtocol` / `UdpRelayProtocol` / `UdpPacketTunnelProtocol` / `UdpPacketFraming` / `UdpPacketStreamFraming` / `UdpDatagramFraming` | 协议元数据和出站行为边界 |
| `DnsResolver` / `TlsConnector` / `TlsAcceptor` | 平台服务 |

## 平台层

- `zero-platform-tokio`
- 为其他平台预留的目录

当前仅实现 Tokio 后端。

## 入站协议

所有入站处理程序实现 `InboundProtocol` 并喂入 `serve_inbound()`：

| 处理程序 | 协议 | 备注 |
|---------|----------|-------|
| `socks5` | SOCKS5 | CONNECT + UDP ASSOCIATE |
| `http_connect` | HTTP CONNECT | |
| `mixed` | 自动检测 | 同一端口上的 SOCKS5 TCP CONNECT、SOCKS5 UDP ASSOCIATE 和 HTTP CONNECT TCP |
| `vless` | VLESS | TCP + UDP-over-TCP |
| `hysteria2` | Hysteria2 | QUIC |
| `shadowsocks` | Shadowsocks | AEAD + 2022-blake3 |
| `trojan` | Trojan | TCP + UDP |
| `vmess` | VMess | 实验性 AEAD TCP、TCP/UDP MUX 和 UDP-over-stream 实现；`cipher: auto` 被规范化为当前 AEAD 基线 |
| `mieru` | Mieru | TCP + UDP，通过加密流封装器 |
| `direct` | Direct | 固定目标转发器，无握手 |
| `tun` | TUN | 虚拟网络接口，消费 `NetworkStack` |
| `system` | System | OS 级流量重定向，消费 `SystemTcpStack` |

## 依赖方向

仅自上而下：

- `zero` -> `config`, `engine`, `proxy`, `api`, `connector`（可选）, `grpc`（可选）
- `proxy` -> `engine`, `config`, `protocols/*`, `transport`, `stack`, `tun`, `dns`
- `transport` -> `config`, `core`, `engine`, `ztls`
- `engine` -> `config`, `router`, `core`, `api`
- `stack` -> `traits`
- `tun` -> `traits`
- `protocols/*` -> `core`, `traits`
- `core` -> `traits`

无反向依赖。
