# VLESS

VLESS 是 `partial` 协议能力。基线 TCP 和 UDP-over-stream 路径存在，UDP MUX、部分 final-hop 传输和互操作覆盖仍有缺口。模块结构与 `protocols/vless/src/` 对应：

| 对应源码 | 内容 |
|---------|------|
| `inbound.rs` | VLESS TCP/UDP 入站、Reality/Vision flow |
| `outbound.rs` | VLESS TCP/UDP 出站 |
| `shared.rs` | UUID、flow、命令常量 |

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 入站 | `supported` | 基线 VLESS TCP 入口 |
| TCP 出站 | `supported` | 基线 VLESS TCP 上游 |
| UDP 入站 | `partial` | UDP-over-stream 基线 |
| UDP 出站 | `partial` | 单跳及部分 relay-chain final-hop 路径（XHTTP `stream-one` 单连接最终跳） |
| MUX | `partial` | TCP MUX 已实现；UDP MUX 未实现 |

## 剩余缺口

- `mux_udp_is_not_implemented`
- `vless_quic_transport_deprecated_by_xtls` — XTLS 已移除 VLESS 独立 `quic` 传输，其继任者为 XHTTP `stream-one`（H3）；项目保留 `quic` 配置字段以向后兼容，但标记为弃用
- `udp_relay_final_hop_not_externally_validated`

## 传输方式

VLESS 支持 8 种传输：`tcp`、`tls`、`reality`、`ws`、`grpc`、`h2`、`http_upgrade`、`xhttp`（即原 `split_http`）。

### XHTTP（`split_http` 配置字段）

XTLS 将 SplitHTTP 重命名为 XHTTP，并移除了独立 `quic` 传输。配置字段仍为 `split_http`（向后兼容），新增 `mode` 字段：

| mode | 说明 |
|------|------|
| `auto`（默认）/ `stream-one` | 单条双向连接：POST 上行分块 + 响应下行分块复用同一 TCP/TLS socket。**唯一可作为 relay-chain 最终跳的模式**（relay 前缀只提供单流） |
| `packet-up` / `stream-up` | 旧式双连接模型（POST 上行 + GET 下行），仅单跳直连，不可作为 relay 最终跳 |

`stream-one` 解决了原 SplitHTTP「不可最终跳」的架构约束：此前 SplitHTTP 需两条独立连接，而 relay 链每跳只给一条流，因此无法作为最终跳；现经 `mode = stream-one`（或默认 `auto`）即可在单流上完成最终跳传输。客户端（`connect_xhttp_stream_one`）与服务端（`accept_xhttp_stream_one`）均实现 stream-one 握手——服务端在收到 POST 请求头后立即在同一连接回写 `200` 分块响应，随后该连接双向承载上传分块（POST body）与下载分块（response body）。
