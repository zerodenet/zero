# VMess MUX

对应 `protocols/vmess/src/mux.rs` — `MuxFrame`、`VmessMuxStream`、Mux.Cool 连接池。

## MuxFrame

Mux.Cool 帧格式：

```
[2-byte length][2-byte session_id][variable payload]
```

```rust
pub struct MuxFrame {
    pub session_id: u16,
    pub payload: Vec<u8>,
}
```

帧类型由 payload 内容隐式决定：
- 首次帧包含 target address = 新建子连接
- 空 payload = 关闭子连接 (FIN)
- 数据 payload = 数据传输

## VmessMuxStream

```rust
pub struct VmessMuxStream<T> {
    inner: T,
    session_id: u16,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
    // ...
}
```

实现 `AsyncRead + AsyncWrite`，封装单个 MUX 子连接的读写：
- `AsyncRead::poll_read`: 从共享上游 stream 读取，过滤出当前 `session_id` 的帧
- `AsyncWrite::poll_write`: 数据累积到 `write_buf`，flush 时编码成 MuxFrame 写入

### MUX 编码/解码

```rust
pub fn encode_mux_frame(frame: &MuxFrame) -> Vec<u8>
pub fn decode_mux_frame(data: &[u8]) -> Result<MuxFrame, VmessError>
```

## VmessMuxConnectionPool

Outbound MUX 连接池：

```rust
pub struct VmessMuxConnectionPool {
    connections: HashMap<PoolKey, PoolEntry>,
    // ...
}

struct PoolKey {
    server: String,
    port: u16,
    uuid: Uuid,
    cipher: VmessCipher,
    transport: String,
}
```

### 池管理

- 按 `(server, port, uuid, cipher, transport)` 五元组分桶
- 同一分桶内的多路 TCP/UDP 子连接共享底层 VMess TLS stream
- `mux_concurrency` 限制最大并发子连接数
- `mux_idle_timeout_secs` 控制空闲连接回收

### UDP over MUX

MUX 同时支持 TCP 子连接和 UDP 子连接。UDP 子连接在 MUX stream 上传输 VMess CMD_UDP packet。

## 帧生命周期

```
[新建 session] → 发送含 target address 的首帧
[数据传输] → 发送含 payload 的数据帧
[关闭 session] → 发送空 payload 帧 (FIN)
```
