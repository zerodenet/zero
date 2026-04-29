#[cfg(feature = "outbound-vless")]
use tokio::io::{AsyncRead, AsyncWrite};
#[cfg(feature = "outbound-vless")]
use tokio_tungstenite::tungstenite::http::Request;
#[cfg(feature = "outbound-vless")]
use zero_config::WebSocketConfig;
#[cfg(feature = "outbound-vless")]
use zero_engine::EngineError;

#[cfg(feature = "outbound-vless")]
pub(crate) struct WebSocketSocket<S> {
    inner: tokio_tungstenite::WebSocketStream<S>,
    read_buffer: Vec<u8>,
    read_offset: usize,
}

#[cfg(feature = "outbound-vless")]
impl<S> WebSocketSocket<S> {
    pub(crate) fn new(inner: tokio_tungstenite::WebSocketStream<S>) -> Self {
        Self {
            inner,
            read_buffer: Vec::new(),
            read_offset: 0,
        }
    }
}

#[cfg(feature = "outbound-vless")]
pub(crate) async fn connect_ws<S>(
    stream: S,
    ws: &WebSocketConfig,
    server: &str,
    port: u16,
) -> Result<WebSocketSocket<S>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let host = format!("{server}:{port}");
    let path = if ws.path.starts_with('/') {
        ws.path.clone()
    } else {
        format!("/{}", ws.path)
    };
    let url = format!("ws://{host}{path}");

    let mut request_builder = Request::builder()
        .uri(url)
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        );

    for (key, value) in &ws.headers {
        request_builder = request_builder.header(key, value);
    }

    let request = request_builder.body(()).map_err(|e| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("WebSocket request build failed: {e}"),
        ))
    })?;

    let (ws_stream, _) = tokio_tungstenite::client_async(request, stream)
        .await
        .map_err(|e| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("WebSocket handshake failed: {e}"),
            ))
        })?;

    Ok(WebSocketSocket::new(ws_stream))
}

#[cfg(feature = "outbound-vless")]
impl<S> tokio::io::AsyncRead for WebSocketSocket<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use futures_util::StreamExt;
        use std::pin::Pin;
        use tokio_tungstenite::tungstenite::Message;

        if self.read_offset < self.read_buffer.len() {
            let available = self.read_buffer.len() - self.read_offset;
            let to_copy = available.min(buf.remaining());
            buf.put_slice(&self.read_buffer[self.read_offset..self.read_offset + to_copy]);
            self.read_offset += to_copy;
            return std::task::Poll::Ready(Ok(()));
        }

        match Pin::new(&mut self.inner).poll_next_unpin(cx) {
            std::task::Poll::Ready(Some(Ok(msg))) => match msg {
                Message::Binary(data) => {
                    self.read_buffer = data;
                    self.read_offset = 0;
                    let to_copy = self.read_buffer.len().min(buf.remaining());
                    buf.put_slice(&self.read_buffer[..to_copy]);
                    self.read_offset = to_copy;
                    std::task::Poll::Ready(Ok(()))
                }
                Message::Text(data) => {
                    self.read_buffer = data.into_bytes();
                    self.read_offset = 0;
                    let to_copy = self.read_buffer.len().min(buf.remaining());
                    buf.put_slice(&self.read_buffer[..to_copy]);
                    self.read_offset = to_copy;
                    std::task::Poll::Ready(Ok(()))
                }
                Message::Close(_) => std::task::Poll::Ready(Ok(())),
                _ => std::task::Poll::Pending,
            },
            std::task::Poll::Ready(Some(Err(e))) => {
                std::task::Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e)))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

#[cfg(feature = "outbound-vless")]
impl<S> tokio::io::AsyncWrite for WebSocketSocket<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        use futures_util::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let inner = &mut self.inner;
        match inner.poll_ready_unpin(cx) {
            std::task::Poll::Ready(Ok(())) => {
                match inner.start_send_unpin(Message::Binary(buf.to_vec())) {
                    Ok(()) => std::task::Poll::Ready(Ok(buf.len())),
                    Err(e) => std::task::Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e,
                    ))),
                }
            }
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            ))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        use futures_util::SinkExt;

        let inner = &mut self.inner;
        match inner.poll_flush_unpin(cx) {
            std::task::Poll::Ready(Ok(())) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            ))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        use futures_util::SinkExt;

        let inner = &mut self.inner;
        match inner.poll_close_unpin(cx) {
            std::task::Poll::Ready(Ok(())) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            ))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
