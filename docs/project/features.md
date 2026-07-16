# 构建特性

Zero 使用 Cargo features 来控制哪些能力子集被包含在编译后的二进制文件中，允许按需裁剪二进制大小和依赖范围。

## 预设

| 预设 | 包含内容 | 适用场景 |
|--------|---------|----------|
| `default` | `full` + `status_api` | 客户端本地使用 |
| `full` | 所有入站/出站协议 + DNS | 完整代理节点 |

```bash
# 默认构建（客户端场景，无需 connector）
cargo build --release

# 等效命令
cargo build --release --features full,status_api
```

## 入站协议

每个入站协议独立受 feature gate 控制，可按需裁剪。

| Feature | 协议 | 额外依赖 |
|---------|------|----------|
| `socks5` | SOCKS5 入站 | -- |
| `http` | HTTP CONNECT 入站 | -- |
| `mixed` | Mixed 入站（同端口 SOCKS5 TCP/UDP + HTTP CONNECT TCP） | 隐含 `socks5` + `http` |
| `vless` | VLESS 入站 | TLS / Reality / WebSocket / gRPC / H2 / HTTP Upgrade / XHTTP 等传输 |
| `hysteria2` | Hysteria2 入站 | QUIC (quinn) |
| `shadowsocks` | Shadowsocks 入站 | AEAD 加密 + 2022-blake3 |
| `trojan` | Trojan 入站 | TLS |
| `vmess` | VMess 入站 | 实验性 AEAD 实现 |
| `mieru` | Mieru 入站 | XChaCha20-Poly1305 会话帧封装 |
| -- | `direct` 入站 | 始终编译，无需 feature gate（固定目标转发器） |
| -- | `tun` 入站 | 始终编译，无需 feature gate（虚拟网络接口：Linux ioctl、macOS utun socket、Windows Wintun） |

```bash
# 裁剪示例：仅 SOCKS5 + HTTP CONNECT
cargo build --release --no-default-features \
  --features socks5,http,status_api
```

## 出站协议

| Feature | 协议 | 额外依赖 |
|---------|------|----------|
| `socks5` | SOCKS5 出站 | -- |
| `vless` | VLESS 出站 | 与入站相同的传输栈 |
| `hysteria2` | Hysteria2 出站 | QUIC (quinn) |
| `shadowsocks` | Shadowsocks 出站 | 与入站相同的加密 |
| `trojan` | Trojan 出站 | TLS |
| `vmess` | VMess 出站 | 实验性 AEAD 实现；`cipher: auto` 被规范化为当前 AEAD 基线 |
| `mieru` | Mieru 出站 | TCP 和 UDP socks5-in-tunnel 路径 |

`direct` 和 `block` 出站始终可用，无需 feature gate——它们不需要协议实现。

## DNS

| Feature | 描述 |
|---------|------|
| `dns` | DNS 解析器、缓存、路由、Fake IP 和 UDP DNS 后端 |

> 当 `dns` 未启用时，DNS 退回到系统解析器（`tokio::net::lookup_host`）。

## 管控面（服务器部署）

以下 features 用于将 Zero 部署为服务器/面板节点，**不在默认 `full` 预设中**。

| Feature | 描述 | 隐含 |
|---------|------|------|
| `status_api` | 运行时控制端点和 selector 切换，包括 HTTP 状态 API | -- |
| `grpc_api` | gRPC 管控面端点 | `dep:zero-grpc` |
| `event_dispatcher` | 事件分发器：将 zero 事件投递到外部 sink 并暴露 sink 投递状态 | `dep:zero-connector` |
| `sink_jsonl` | JSON Lines 文件 sink（事件持久化） | `event_dispatcher` |
| `panel_connector` | 面板 connector：心跳 + 远程命令，节点上报 | `status_api` + `event_dispatcher` |

```bash
# 服务器构建（包含面板 connector）
cargo build --release --features full,status_api,panel_connector
```

**`panel_connector` 依赖范围：**

- `status_api` -- HTTP 控制端点
- `event_dispatcher` -- 事件投递基础设施和 sink 投递状态
- `zero-connector` crate -- PushConnector（心跳/命令轮询）、EventDispatcher（事件分发）、Webhook sink

## 客户端 vs 服务器

```
客户端场景：  full + status_api  （默认）
                  - 入站/出站协议
                  - DNS
                  - HTTP 状态端点（本地调试）

服务器场景：  + panel_connector
                  - 事件分发（webhook / jsonl）
                  - 面板心跳上报 + 远程命令
```

## 与协议实现的关系

协议 crates 通过上述根 Cargo features 编译。协议在 workspace 中存在本身并不意味着与每个外部生态系统导出具有生产级兼容性。

机器可读的协议矩阵通过 `capabilities.protocols` 暴露。它记录当前二进制文件的当前 TCP、UDP、MUX、传输、兼容性基线和限制事实。`zero-api` 定义响应结构；代理运行时从已编译的协议清单中填充协议事实。参见 [protocol-capabilities.md](protocol-capabilities.md)。

| 协议 | Feature | 备注 |
|------|---------|------|
| VMess | `vmess` | 实验性 AEAD 实现。来自 Xray/Clash 导出的 `cipher: auto` 被规范化为当前 AEAD 基线 |
| Mieru | `mieru` | TCP/UDP 入站和出站基线可用；能力状态以运行时矩阵为准 |
| HTTP CONNECT 出站 | -- | 出站方向未实现 |

入站/出站 features 不对等是正常的——某些协议不需要相反方向。

## 二进制大小

二进制大小取决于目标平台、Rust 版本、链接器、调试信息、LTO 和 strip 设置。本文档不维护容易失真的固定数值；需要比较 feature 裁剪效果时，应在相同工具链和构建参数下生成基准。

---

# 内核原语

这些跨切面能力位于内核管道中，统一应用于所有 TCP 协议。

## 空闲超时

每个 TCP 中继都包裹在空闲超时中。如果配置的持续时间内任一方向都没有数据流动，会话将被干净地终止。

- **默认值**：300 秒（5 分钟）
- **配置**：`InboundConfig.idle_timeout_secs`（可选，按入站配置）
- **作用范围**：在 `serve_inbound()` 中通过 `tokio::time::timeout` 包裹 `protocol.relay()` 来应用
- **行为**：空闲超时不是错误——会话以其当前结果（`DirectRelayed` 或 `ChainedRelayed`）结束

## 出站健康 / 熔断器

`zero-engine` 为每个出站标签维护一个 `OutboundHealth` 跟踪器。在连接到任何出站（除 `direct` 和 `block` 外）之前，内核检查该出站是否健康。

- **失败阈值**：30 秒滑动窗口内 5 次失败
- **隔离时间**：60 秒——该出站被跳过，不接受所有新连接
- **探测**：隔离期满后，允许一个连接作为探测；成功则恢复健康，失败则重置冷却期
- **跟踪**：连接错误时调用 `record_outbound_failure()`，中继完成时调用 `record_outbound_success()`
- **作用范围**：适用于 fallback 组候选选择和所有链式出站连接
- **错误类型**：`EngineError::UnhealthyOutbound { tag }`——被视为连接失败，触发下一个 fallback 候选

## URL 域名重写

在路由之前应用的基于域名的 URL 重写。规则按首次匹配优先的方式执行；一旦某条规则匹配，不再评估后续规则。

- **配置**：`route.url_rewrite`（`UrlRewriteRule` 数组）
- **匹配类型**：
  - `from` -- 精确域名匹配
  - `from_regex` -- 正则表达式模式匹配，支持捕获组替换（`$1`、`$2` 等）
- **替换**：`to` 字段指定替换域名
- **HTTP 重定向**：`status_code` 字段（如 `302`）对 HTTP CONNECT 触发 HTTP 重定向响应；非 HTTP 协议静默重写
- **作用范围**：在 `serve_inbound()` 中进行路由查找之前应用；也在 HTTP CONNECT 自身处理程序中应用以进行即时重定向

```json
{
  "route": {
    "url_rewrite": [
      { "from": "old.example.com", "to": "new.example.com" },
      { "from_regex": "^(.+)\\.mirror\\.example\\.com$", "to": "$1.example.com" },
      { "from": "temp.example.com", "to": "permanent.example.com", "status_code": 301 }
    ]
  }
}
```

## 域名正则路由条件

路由条件类型 `domain_regex` 根据一个或多个正则表达式模式匹配目标域名。

- **配置**：`{ "type": "domain_regex", "values": ["^.*\\.google\\..*$", "^.*\\.youtube\\..*$"] }`
- **匹配**：模式在启动时编译一次（`regex::Regex`），然后在决策时与目标域名匹配
- **捕获组**：不用于路由——仅用于匹配。如需基于捕获的重写，使用 `url_rewrite.from_regex`
- **作用范围**：作为规则条件系统的一部分，可与 `and`/`or` 组合

```json
{
  "condition": { "type": "domain_regex", "values": ["^.*\\.google\\..*$"] },
  "action": { "type": "route", "outbound": "proxy" }
}
```

## GCRA 速率限制

使用通用信元速率算法（GCRA）的令牌桶速率限制。在 TCP 中继流上限制按字节吞吐量。

- **配置**：`InboundProtocolConfig` 上的按入站 `up_bps` 和 `down_bps`（Hysteria2、Shadowsocks、Trojan）
- **按用户**：协议 `accept` 处理程序可以设置按用户限制（例如 SOCKS5 通过 `AuthHandler::rate_limit_for()`）；按用户限制优先于按入站默认值
- **内核集成**：`serve_inbound()` 中的 `apply_kernel_rate_limits()` 为未设置按用户限制的会话填充默认值
- **传输层**：`RateLimiter` 在 `tcp_relay.rs` 中包装 `AsyncWrite`；非阻塞——通过 `poll_write` 集成
- **突发容忍度**：每个流 16 KB 余量，避免饿死小写入
- **作用范围**：在双向中继路径中的 `protocol.relay()` 期间应用

```json
{
  "tag": "hysteria2-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "hysteria2",
    "password": "secret",
    "up_bps": 10485760,
    "down_bps": 52428800
  }
}
```

## TUN（虚拟网络接口）

TUN 创建一个虚拟网络接口，在第 3 层捕获 IP 数据包并通过代理内核路由它们。始终编译，无需 feature gate。

### 架构

```
TunDevice (zero-tun)          -> 平台后端（Linux ioctl、macOS utun、Windows Wintun）
    -> NetworkStack (zero-traits)    -> TcpStack / UdpStack 特征
    -> UserTcpStack (zero-stack)     -> 用户空间 TCP 状态机（SYN -> SYN-ACK -> ACK -> 数据 -> FIN）
    -> TUN 入站 (zero-proxy)      -> tokio::select!{ 读取数据包 -> 喂入栈 -> accept -> serve_inbound() }
```

### 网络栈特征

`zero-traits` 定义 `TcpStack` / `UdpStack` / `NetworkStack`；这是原始 IP 数据包和面向连接的 I/O 之间的边界。两种实现：

| 实现 | 策略 | 驱动 |
|---------------|----------|--------|
| `UserNetworkStack` | 用户空间 TCP 状态机（SYN -> Established -> CloseWait，MSS 选项，seq/ack 跟踪） | 需要 TUN 设备 |
| `SystemStack` | OS TCP 监听器（iptables/pf redirect -> accept TcpStream） | Linux/macOS 无需 |

该栈通过特征可插拔；切换实现不需要更改入站处理程序。

### TCP 状态机 (UserTcpStack)

- **SYN** -> 发送 SYN-ACK 包含 MSS 选项 -> 存储在 SynReceived 状态
- **ACK** -> 转换为 Established -> 可通过 `TcpStack::accept()` 获取
- **数据** -> 提取载荷，通过 channel 转发到代理，发送 ACK
- **FIN** -> 发送 ACK，转换为 CloseWait -> 代理关闭触发我们的 FIN
- **RST** -> 立即拆除

### 平台支持

| 平台 | 后端 | 依赖 | 提供方 |
|----------|---------|------------|-------------|
| Linux | `/dev/net/tun` ioctl | 内核内置 | OS |
| macOS | utun socket | 内核内置 | OS |
| Windows | Wintun 驱动 | `wintun.dll` | GUI / 安装器 |

在 Windows 上，`wintun.dll` 是平台资源，就像 Linux 上的 `/dev/net/tun`；它必须存在于目标系统上，但内核仅声明依赖（通过 `wintun` crate），不管理 DLL 生命周期。

### CLI 命令

```bash
zero tun start --addr 10.0.0.1 --tag proxy    # 启动 TUN
zero tun stop                                  # 停止 TUN
zero tun status                                # 查看状态
```

命令通过 IPC 路由（`ProxyHandle` 在 TUN 命令到达 engine 之前拦截它们）。
