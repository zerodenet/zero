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

## 未完成：AEAD 2022 TCP

当前 TCP 2022 仍使用现有 AEAD stream wrapper，没有实现 SIP022 的 TCP request/response header protocol。

完成标准：
- 实现 AEAD 2022 TCP request header
- 实现 AEAD 2022 TCP response header
- 区分常规 AEAD TCP chunking 与 AEAD 2022 TCP header/chunking
- 与 `shadowsocks-rust` 做本地 TCP 外部互通测试
