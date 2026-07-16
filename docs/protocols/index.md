# 协议概览

本节记录当前代码中的协议能力，面向 GUI、面板适配器和内核集成者。运行时消费者应以 `capabilities.protocols` 响应作为机器可读的权威来源。

## 状态术语

| 状态 | 含义 |
|------|------|
| `supported` | 正常内核能力，无已知协议级缺口 |
| `partial` | 基线路径存在，但互操作、MUX、特殊传输或某个方向仍有缺口 |
| `experimental` | 已实现，但不适合做生产兼容性假设 |
| `unsupported` | 未实现 |
| `not_applicable` | 协议不定义此方向 |

## 能力矩阵

| 协议 | 配置类型 | 总体状态 | TCP 入站 | TCP 出站 | UDP 入站 | UDP 出站 | 详情 |
|------|----------|----------|----------|----------|----------|----------|------|
| SOCKS5 | `socks5` | `supported` | `supported` | `supported` | `supported` | `supported` | [SOCKS5](./socks5/index.md) |
| HTTP CONNECT | `http` | `supported` | `supported` | `unsupported` | `not_applicable` | `not_applicable` | [HTTP CONNECT](./http/index.md) |
| Mixed | `mixed` | `supported` | `supported` | `unsupported` | `supported` | `unsupported` | [Mixed](./mixed/index.md) |
| VLESS | `vless` | `partial` | `supported` | `supported` | `partial` | `partial` | [VLESS](./vless/index.md) |
| Hysteria2 | `hysteria2` | `partial` | `supported` | `supported` | `partial` | `partial` | [Hysteria2](./hysteria2/index.md) |
| Shadowsocks | `shadowsocks` | `partial` | `supported` | `supported` | `supported` | `supported` | [Shadowsocks](./shadowsocks/index.md) |
| Trojan | `trojan` | `partial` | `supported` | `supported` | `partial` | `partial` | [Trojan](./trojan/index.md) |
| Mieru | `mieru` | `supported` | `supported` | `supported` | `supported` | `supported` | [Mieru](./mieru/index.md) |
| VMess | `vmess` | `partial` | `partial` | `partial` | `partial` | `partial` | [VMess](./vmess/index.md) |

总体状态不能代替方向状态。例如一个协议可能具有稳定 TCP 基线，但 UDP、MUX 或外部互操作覆盖仍为 `partial`。完整限制码见[协议能力](../project/protocol-capabilities.md)。

## 内核动作

| 名称 | 状态 | 说明 |
|------|------|------|
| `direct` | `supported` | 内核直连动作，TCP 和 UDP 出站可用 |
| `block` | `supported` | 内核拒绝动作，可用于 TCP 和 UDP 路由决策 |
| `tun` | `supported` | 三层虚拟接口，通过 CLI 或控制面管理 |

## 文档结构

每个协议在 `docs/protocols/` 下拥有独立目录。页面按实际需要拆分，不要求所有协议使用完全相同的文件集合：

```text
docs/protocols/{protocol}/
  index.md
  inbound.md
  outbound.md
  shared.md
  metadata.md
  stream.md
  crypto.md
  mux.md
  udp.md
```

公共页面：

- [配置速查](./configuration.md)：常用协议配置示例。
- [未完成项](./incomplete.md)：跨协议缺口索引。
- [协议能力](../project/protocol-capabilities.md)：描述符、状态和运行时能力模型。
