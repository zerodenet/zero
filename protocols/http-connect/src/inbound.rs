use alloc::vec::Vec;

use zero_core::{Error, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::parse::{first_line, parse_connect_request};

const MAX_REQUEST_SIZE: usize = 8192;
const HEADERS_END: &[u8] = b"\r\n\r\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpConnectResponse {
    ConnectionEstablished,
    BadRequest,
    MethodNotAllowed,
    Forbidden,
    BadGateway,
}

impl HttpConnectResponse {
    fn status_line(self) -> &'static str {
        match self {
            Self::ConnectionEstablished => "HTTP/1.1 200 Connection Established\r\n\r\n",
            Self::BadRequest => {
                "HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
            }
            Self::MethodNotAllowed => {
                "HTTP/1.1 405 Method Not Allowed\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
            }
            Self::Forbidden => {
                "HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
            }
            Self::BadGateway => {
                "HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HttpConnectInbound;

impl HttpConnectInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::HttpConnect
    }

    pub async fn accept_request<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        let request = read_request_head(stream).await?;
        let line = first_line(&request)?;
        let (target, port) = parse_connect_request(line)?;

        Ok(Session::new(
            0,
            target,
            port,
            Network::Tcp,
            ProtocolType::HttpConnect,
        ))
    }

    pub async fn send_response<S>(
        &self,
        stream: &mut S,
        response: HttpConnectResponse,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        stream
            .write_all(response.status_line().as_bytes())
            .await
            .map_err(|_| Error::Io("failed to write HTTP CONNECT response"))
    }

    pub async fn handshake<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        let session = self.accept_request(stream).await?;
        self.send_response(stream, HttpConnectResponse::ConnectionEstablished)
            .await?;
        Ok(session)
    }
}

async fn read_request_head<S>(stream: &mut S) -> Result<Vec<u8>, Error>
where
    S: AsyncSocket,
{
    let mut request = Vec::new();

    loop {
        if request.len() >= MAX_REQUEST_SIZE {
            return Err(Error::Protocol("HTTP CONNECT request head is too large"));
        }

        let mut byte = [0_u8; 1];
        let read = stream
            .read(&mut byte)
            .await
            .map_err(|_| Error::Io("failed to read HTTP CONNECT request"))?;

        if read == 0 {
            return Err(Error::Io(
                "unexpected EOF while reading HTTP CONNECT request",
            ));
        }

        request.push(byte[0]);

        if request.ends_with(HEADERS_END) {
            return Ok(request);
        }
    }
}
