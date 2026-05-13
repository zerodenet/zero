# Hysteria2 TODO

## 架构改进

### stream.rs 属于传输层，不应在纯协议 crate

**现状：** `Hysteria2Stream` 包装 `quinn::SendStream` + `quinn::RecvStream`，实现在 `protocols/hysteria2/src/stream.rs`，由 `#[cfg(feature = "quic")]` 门控。

**问题：** 这是传输/平台层实现，不应放在协议 crate 中。纯协议部分（`shared.rs`, `inbound.rs`, `outbound.rs`, `udp.rs`）正确——仅依赖 `zero-core` + `zero-traits`，`no_std` 可用。

**遵循项目现有约定：** VLESS 同样在 `protocols/vless/src/transport/quic.rs` 中有 `QuicStream`。两个协议 crate 都存在此问题。

**目标：** 将 `Hysteria2Stream` 和 `QuicStream` 移至 `zero-platform-tokio` 或独立的传输 crate，协议 crate 只保留 trait 层面的引用。

**工时估计：** 与 VLESS 的 transport 归位一起做，3-4h。

---

## 功能待完善

### 出站 auth 流程

**现状：** `connect_via_hysteria2_upstream` 仅建立 QUIC 连接，缺少 HMAC-SHA256 认证握手。

**范围：**
- 建立 QUIC 连接后导出 keying material 作为 salt
- 计算 `HMAC-SHA256(password, salt)` 并发送 auth 帧
- 读服务端响应，验证认证成功

**工时估计：** 1h

### 出站 TCP stream

**现状：** 出站仅返回 `QuicStream`，但 Hysteria2 的 TCP 需要先 open_bi，再发送 connect header。

**范围：**
- `connect_via_hysteria2_upstream` 中 open_bi 获取流
- 发送 `build_tcp_connect_header` 
- 读 connect 响应
- 返回 `Hysteria2Stream` 用于双向中继

**工时估计：** 1h

### UDP 域名解析

**现状：** `hysteria2_datagram_loop` 中域名目标仅记录 warning 跳过。

**范围：** 用 `self.resolver` 解析域名后再转发。

**工时估计：** 0.5h
