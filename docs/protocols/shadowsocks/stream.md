# Shadowsocks Stream

对应 `protocols/shadowsocks/src/stream.rs` — `ShadowsocksAeadStream`。

## ShadowsocksAeadStream

实现 `AsyncRead + AsyncWrite`，为 Shadowsocks TCP relay 提供透明双向 AEAD 加解密。

### 结构

```rust
pub struct ShadowsocksAeadStream {
    inner: T,                       // 底层 TCP stream
    cipher: CipherKind,
    key: Vec<u8>,                   // 读方向密钥
    nonce: Vec<u8>,                 // 读方向 nonce
    write_key: Vec<u8>,             // 写方向密钥
    write_nonce: Vec<u8>,           // 写方向 nonce
    read_buf: Vec<u8>,              // 解密后待消费的 buffer
    // ...
}
```

### 职责

- **TCP chunk 读取**：一次读取完整 AEAD chunk（2 字节长度 + payload + 16 字节 tag），解密后缓存供应用消费
- **TCP chunk 写入**：将要发送的数据切分成 AEAD chunk，加密后写入底层 stream
- **下载方向加解密**：使用 `key`/`nonce`
- **上传方向加解密**：使用 `write_key`/`write_nonce`
- **Response salt 生成**：inbound 侧为 server→client 方向生成独立的 response salt + download key
- **Chunk 边界处理**：正确处理跨 AEAD chunk 边界的大 payload

### Chunk 格式

```
[2-byte length (big-endian)][variable-length encrypted payload][16-byte AEAD tag]
```

maximum chunk payload 为 0x3FFF (16383) 字节。

### Inbound Stream 构造

Inbound accept 返回 `ShadowsocksAccept` 后，proxy runtime 用 session key 材料构造 `ShadowsocksAeadStream`：

```rust
ShadowsocksAeadStream::new(
    tcp_stream,
    cipher,
    response_key,      // server→client 读方向 key（client 的上传即 server 的下载）
    response_nonce,
    session_key,       // client→server 读方向 key（client 的下载即 server 的上传）
    session_nonce,
)
```

### Outbound Stream 构造

Outbound 通过 `ShadowsocksOutboundSession` 返回 session key 材料。Proxy runtime 在 `apply_hop_protocol()` 中构造 `ShadowsocksAeadStream` 包裹已连接的 TCP stream。

## AEAD 2022 TCP（SIP022）

`CipherKind::is_blake3()` 派发到 SIP022 路径：`send_request_2022` 写入 `salt + fixed-header chunk (nonce 0) + variable-header chunk (nonce 1)`，body length/payload 对从 nonce 2 继续；inbound `accept_request_2022` 以**单次读取**接收 salt + fixed-header（SIP022 3.1.3 检测防御），失败时 drain 后再关闭。响应流的 fixed-header chunk（nonce 0，含 request-salt 回填）兼作首个 length chunk，首个 payload chunk 在 nonce 1。三个 blake3 cipher 均覆盖。

验证：TCP 入站已通过 `shadowsocks-rust` 参考客户端 `sslocal` 端到端互操作（HTTP 200）；TCP 出站管线已通过 Zero→Zero 验证；常规 AEAD 与 2022 路径由 `is_blake3()` 在 `send_request` / `accept_request` / `ShadowsocksAeadStream` 中区分。

