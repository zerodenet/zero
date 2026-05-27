# v0.0.4

这一版继续留在 `v0.0.x` 预发布线，不做正式产品定义。

`v0.0.3` 已经补齐 DNS 子系统（缓存、路由、拦截、Fake IP、DNS-over-HTTPS / DNS-over-TLS）、事件系统、认证强制执行、配置热重载、EnginePlan 收紧和零默认配置路径。`v0.0.4` 完成两件大事：**架构重构**（协议入站管线统一化）和 **四个内核原语补齐**（速率限制、空闲超时、出站熔断、域名重写），外加 `domain-regex` 规则条件和 **TUN 虚拟网卡**。

## 要交付什么

### 1. 入站架构重构

`v0.0.4` 之前，各协议的 TCP 会话处理散落在 `impl Proxy` 的不同方法里，通过 `TcpInboundProtocol` 枚举分发。入站和出站之间存在跨模块依赖。这一版做了彻底重构：

- **direct inbound（固定目标转发器）** —— 新入站类型，监听端口接受原始 TCP 连接（无协议握手），将所有流量转发到配置的出站。目标地址来自入站配置而非客户端，适合端口转发/流量重定向场景。
- **引入 `InboundProtocol` trait** —— 统一协议处理器接口。每个协议实现 `accept`（握手并建立流）、`send_ok`（上游连接成功后的响应）、`send_blocked`（被规则拦截）、`send_upstream_failure`（上游连接失败）和 `relay`（双向数据转发）。协议特有的逻辑全部内聚在各自的 handler 里。
- **`serve_inbound()` 作为唯一内核入口** —— 所有 TCP 入站协议都经过同一条管线：`accept` -> 路由决策 -> `send_ok` / `send_blocked` / `send_upstream_failure` -> `relay`。速率限制和空闲超时也在这条管线上统一施加。
- **删除 `handle_tcp_session` 和 `TcpInboundProtocol` 枚举** —— 不再需要 match 分发。
- **传输层纯化** —— `zero-transport` 不再依赖 `zero-proxy`，只做纯 I/O。
- **消除入站到出站的跨模块依赖** —— 入站 handler 不再直接引用出站类型。

架构收益：新加一个入站协议只需实现 `InboundProtocol` trait，无需修改管线逻辑。

### 2. 速率限制（GCRA 算法）

用 GCRA（Generic Cell Rate Algorithm）替代了旧的 sleep-based 限速：

- **`RateLimitedWriter`** / **`RateLimiter`** —— 基于 GCRA 的高精度速率控制，不再依赖 `tokio::time::sleep` 粗粒度节流。
- `serve_inbound()` 管线上统一施加，对每个 TCP 连接的上行/下行方向分别限速。
- 支持 per-user 速率限制（配合 `Session::apply_auth` 统一认证后按用户粒度施加）。

### 3. TCP 空闲超时

- 默认 300 秒，可在每个入站上通过 `idle_timeout_secs` 覆盖。
- 作用于 `serve_inbound()` 管线的 relay 阶段：连接在到达超时后自动关闭，防止僵死连接长期占用资源。

### 4. 出站健康追踪 / 熔断器

- 阈值：5 次失败在 30 秒窗口内触发熔断。
- 熔断行为：出站被隔离 60 秒，期间不接收新连接。
- 恢复：60 秒后自动探活，成功后恢复。
- 纯内核原语，不绑定特定出站类型，对所有 outbound group 透明生效。

### 5. URL 域名重写

- 配置项 `route.url_rewrite`：`Vec<UrlRewriteRule>`。
- 每条规则包含 `from`（精确匹配）或 `from_regex`（正则匹配）→ `to`（替换目标）。
- 可选 `status_code` 字段：设为 302 时触发 HTTP 重定向（对 HTTP 代理场景透明生效），不设则执行域名重写后继续代理。

### 6. `domain-regex` 规则条件

- `RuleConditionConfig` 新增 `DomainRegex` 变体。
- 路由引擎支持正则匹配域名，补充已有的 `domain`（精确/子域名通配）、`domain-keyword`、`ip`、`geoip` 条件。

### 7. 配置变更汇总

| 新增项 | 位置 | 类型 | 说明 |
|--------|------|------|------|
| `idle_timeout_secs` | `InboundConfig` | `Option<u64>` | TCP 空闲超时秒数，默认 300 |
| `url_rewrite` | `RouteConfig` | `Vec<UrlRewriteRule>` | URL 域名重写规则列表 |
| `DomainRegex` | `RuleConditionConfig` | variant | 正则匹配域名 |
| `rate_limits()` | `InboundProtocolConfig` | method | 返回该入站的速率限制配置 |
| `tun` | `InboundProtocolConfig` | variant | TUN 虚拟网卡入站类型 |

### 8. TUN 虚拟网卡

- **`crates/tun/` 新 crate** — 定义 `TunDevice` trait，提供 Linux（ioctl）、macOS（utun socket）、Windows（Wintun）三个平台后端。
- **`crates/proxy/src/inbound/tun.rs`** — TUN 入站监听器，实现 TCP 状态机，从虚拟网卡读取 IP 数据包、重组 TCP 流，集成到 `serve_inbound()` 统一管线。
- **Runtime API**: `Proxy::start_tun(name, addr, mask, mtu, tag)` — 创建 TUN 设备并启动数据包读取循环，将识别出的 TCP 连接送入内核代理管线。
- **无 feature gate**：始终编译，不依赖可选 Cargo feature。
- **路由集成**：TUN 流量以入站 `tag` 落入路由表，可通过标准路由规则定向到任意出站或出站组。

## 明确不做什么

- 不在这一版引入新的传输层协议或出站协议。
- 不做 QUIC 层面的拥塞控制或传输优化。
- 不扩展熔断器到更复杂的健康检测（心跳、延迟采样等）——当前只按失败次数触发。
- URL 重写不做路径级别的重写，只做域名级别。

## 怎么推进

1. 入站架构重构先落地（`InboundProtocol` trait + `serve_inbound()` 统一管线），因为它影响所有现有协议。
2. 在内核管线上挂接三个原语：GCRA 速率限制、TCP 空闲超时、出站熔断。
3. URL 域名重写和 `domain-regex` 规则条件作为独立特性并行推进。
4. 最后补齐 HTTP 302 重定向扩展，收尾版本号。

提交序列：
- `5afc896` — 入站架构重构
- `fcaca59` — GCRA 速率限制
- `e4d6bea` — TCP 空闲超时
- `b9d3021` — 出站健康追踪 / 熔断器
- `73c4080` — domain-regex 规则条件
- `2a03384` — URL 域名重写
- `7293f29` — HTTP 重定向支持
- `81b5c45` — 版本号提升至 0.0.4

## 怎么验收

1. **入站架构**：所有协议（SOCKS5、HTTP CONNECT、混合、VLESS、Hysteria2、Shadowsocks、Trojan）的入站 TCP 代理链路正常，行为与重构前一致。
2. **速率限制**：对单个连接配置上行/下行限速后，实际吞吐量不超过配置值的 GCRA 容差范围（1 burst）。
3. **空闲超时**：建立一个 TCP 代理连接后不传输任何数据，在 `idle_timeout_secs` 秒后自动关闭。
4. **熔断器**：对一个正常工作的出站上游，连续制造 5 次连接失败后，该出站进入 quarantine 状态；60 秒后自动探活恢复。
5. **域名重写**：访问 `example.com` 的请求被重写到 `target.example`，代理行为按重写后的目标执行。
6. **HTTP 重定向**：配置 `status_code: 302` 的重写规则后，HTTP 代理请求收到 302 重定向响应。
7. **domain-regex**：用正则 `.*\.internal\.io` 匹配 `app.internal.io`，规则命中并执行对应路由动作。
