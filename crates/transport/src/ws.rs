use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::tungstenite::http::Request;
use zero_config::WebSocketConfig;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use zero_platform_tokio::ClientStream;

pub struct WebSocketSocket<S> {
    inner: tokio_tungstenite::WebSocketStream<S>,
    read_buffer: Vec<u8>,
    read_offset: usize,
}

impl<S> WebSocketSocket<S> {
    pub fn new(inner: tokio_tungstenite::WebSocketStream<S>) -> Self {
        Self {
            inner,
            read_buffer: Vec::new(),
            read_offset: 0,
        }
    }
}

pub async fn accept_ws<S>(
    stream: S,
    expected_path: &str,
) -> Result<WebSocketSocket<S>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

    let callback = |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
        let path = request.uri().path();
        if path != expected_path {
            return Err(ErrorResponse::new(Some(format!(
                "expected path {expected_path}, got {path}"
            ))));
        }
        Ok(response)
    };

    let ws_stream = tokio_tungstenite::accept_hdr_async(stream, callback)
        .await
        .map_err(|e| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("WebSocket accept failed: {e}"),
            ))
        })?;

    Ok(WebSocketSocket::new(ws_stream))
}

pub async fn connect_ws<S>(
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
                std::task::Poll::Ready(Err(std::io::Error::other(e)))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

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

        match self.inner.poll_ready_unpin(cx) {
            std::task::Poll::Ready(Ok(())) => {
                match self.inner.start_send_unpin(Message::Binary(buf.to_vec())) {
                    Ok(()) => std::task::Poll::Ready(Ok(buf.len())),
                    Err(e) => std::task::Poll::Ready(Err(std::io::Error::other(e))),
                }
            }
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::other(e))),
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
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::other(e))),
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
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::other(e))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl<S> AsyncSocket for WebSocketSocket<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync,
{
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        use tokio::io::AsyncReadExt;
        AsyncReadExt::read(self, buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        use tokio::io::AsyncWriteExt;
        AsyncWriteExt::write_all(self, buf).await?;
        AsyncWriteExt::flush(self).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        use tokio::io::AsyncWriteExt;
        AsyncWriteExt::shutdown(self).await
    }
}

impl<S> ClientStream for WebSocketSocket<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync,
{
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "WebSocketSocket does not expose local_addr",
        ))
    }
}
