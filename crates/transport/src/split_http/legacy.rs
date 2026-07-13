use std::io;
use std::time::Duration;

use http::Request;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;
use zero_config::SplitHttpConfig;
use zero_engine::EngineError;

use super::paired::{SplitHttpPairedStream, SplitHttpStream};
use super::registry::{generate_session_id, SplitHttpPending, SplitHttpRegistry};
use super::wire::{
    find_header_end, parse_method_and_session, parse_status, validate_path, write_get_response,
    write_http_request,
};

pub async fn connect_split_http<S>(
    post_stream: S,
    get_stream: S,
    config: &SplitHttpConfig,
) -> Result<SplitHttpStream<S>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let host = config.host.as_deref().unwrap_or("localhost");
    let path = config.path.as_str();
    let session_id = generate_session_id();
    let post_request = Request::builder()
        .method("POST")
        .uri(path)
        .header("Host", host)
        .header("X-Session-Id", &session_id)
        .header("Transfer-Encoding", "chunked")
        .header("Content-Type", "application/octet-stream")
        .body(())
        .map_err(|error| {
            EngineError::Io(io::Error::other(format!(
                "split-http post request: {error}"
            )))
        })?;
    let get_request = Request::builder()
        .method("GET")
        .uri(path)
        .header("Host", host)
        .header("X-Session-Id", &session_id)
        .body(())
        .map_err(|error| {
            EngineError::Io(io::Error::other(format!("split-http get request: {error}")))
        })?;

    let mut post = post_stream;
    let mut request = Vec::new();
    write_http_request(&mut request, &post_request);
    post.write_all(&request).await.map_err(EngineError::Io)?;
    let mut get = get_stream;
    request.clear();
    write_http_request(&mut request, &get_request);
    get.write_all(&request).await.map_err(EngineError::Io)?;

    let (headers, prefetched) = read_headers(&mut get, "connect").await?;
    let status = parse_status(&headers);
    if status != Some(200) {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("split-http connect: expected 200, got {status:?}"),
        )));
    }
    Ok(SplitHttpPairedStream::new_with_prefetched(
        get, post, prefetched,
    ))
}

pub async fn accept_split_http<S>(
    stream: S,
    config: &SplitHttpConfig,
    registry: &SplitHttpRegistry,
) -> Result<Option<SplitHttpStream<S>>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut stream = stream;
    let (headers, _) = read_headers(&mut stream, "accept").await?;
    let (method, session_id) = parse_method_and_session(&headers)?;
    validate_path(&headers, config.path.as_str())?;
    match method.as_str() {
        "POST" => accept_half(stream, session_id, registry, true).await,
        "GET" => accept_half(stream, session_id, registry, false).await,
        other => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("split-http: unexpected method {other}"),
        ))),
    }
}

async fn read_headers<S>(stream: &mut S, stage: &str) -> Result<(Vec<u8>, Vec<u8>), EngineError>
where
    S: AsyncRead + Unpin,
{
    let mut buf = vec![0u8; 4096];
    let mut total = 0;
    loop {
        let n = stream
            .read(&mut buf[total..])
            .await
            .map_err(EngineError::Io)?;
        if n == 0 {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                format!("split-http {stage}: unexpected EOF"),
            )));
        }
        total += n;
        if let Some(end) = find_header_end(&buf[..total]) {
            return Ok((buf[..end].to_vec(), buf[end..total].to_vec()));
        }
        if total >= buf.len() {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("split-http {stage}: headers too large"),
            )));
        }
    }
}

async fn accept_half<S>(
    mut stream: S,
    session_id: String,
    registry: &SplitHttpRegistry,
    is_post: bool,
) -> Result<Option<SplitHttpStream<S>>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut entries = registry.inner.lock().await;
    if let Some(pending) = entries.remove(&session_id) {
        drop(entries);
        let other = downcast(pending)?;
        if is_post {
            let mut get = other;
            write_get_response(&mut get).await?;
            return Ok(Some(SplitHttpPairedStream::new(stream, get)));
        }
        write_get_response(&mut stream).await?;
        return Ok(Some(SplitHttpPairedStream::new(other, stream)));
    }

    let (notify, receiver) = oneshot::channel();
    entries.insert(
        session_id.clone(),
        SplitHttpPending {
            stream: Box::new(stream),
            _notify: notify,
        },
    );
    drop(entries);
    match tokio::time::timeout(Duration::from_secs(60), receiver).await {
        Ok(Ok(())) | Ok(Err(_)) => Ok(None),
        Err(_) => {
            registry.inner.lock().await.remove(&session_id);
            let method = if is_post { "POST" } else { "GET" };
            let peer = if is_post { "GET" } else { "POST" };
            Err(EngineError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("split-http: {method} timed out waiting for {peer}"),
            )))
        }
    }
}

fn downcast<S>(pending: SplitHttpPending) -> Result<S, EngineError>
where
    S: Send + 'static,
{
    pending
        .stream
        .downcast::<S>()
        .map(|stream| *stream)
        .map_err(|_| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "split-http: type mismatch",
            ))
        })
}
