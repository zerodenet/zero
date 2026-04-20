use alloc::vec;

use zero_core::{Address, Error, InboundHandler, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::shared::{
    read_address, read_exact, write_reply, write_reply_with_address, Socks5Reply, CMD_CONNECT,
    CMD_UDP_ASSOCIATE, METHOD_NOT_ACCEPTABLE, METHOD_NO_AUTH, SOCKS5_VERSION,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct Socks5Inbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Socks5Request {
    Connect(Session),
    UdpAssociate(Socks5UdpAssociateRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpAssociateRequest {
    pub client_hint: Address,
    pub client_port: u16,
}

impl Socks5Inbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Socks5
    }

    pub async fn accept_request<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        match self.accept_command(stream).await? {
            Socks5Request::Connect(session) => Ok(session),
            Socks5Request::UdpAssociate(_) => {
                write_reply(stream, Socks5Reply::CommandNotSupported).await?;
                Err(Error::Unsupported("SOCKS5 command is not supported"))
            }
        }
    }

    pub async fn accept_command<S>(&self, stream: &mut S) -> Result<Socks5Request, Error>
    where
        S: AsyncSocket,
    {
        negotiate_method(stream).await?;
        read_request(stream).await
    }

    pub async fn send_response<S>(&self, stream: &mut S, reply: Socks5Reply) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        write_reply(stream, reply).await
    }

    pub async fn send_response_with_bound<S>(
        &self,
        stream: &mut S,
        reply: Socks5Reply,
        address: &Address,
        port: u16,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        write_reply_with_address(stream, reply, address, port).await
    }

    pub async fn handshake<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        let session = self.accept_request(stream).await?;
        self.send_response(stream, Socks5Reply::Succeeded).await?;

        Ok(session)
    }
}

impl<S> InboundHandler<S> for Socks5Inbound
where
    S: AsyncSocket,
{
    async fn handshake(&self, stream: &mut S) -> Result<Session, Error> {
        Self::handshake(self, stream).await
    }
}

async fn negotiate_method<S>(stream: &mut S) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut header = [0_u8; 2];
    read_exact(stream, &mut header).await?;

    if header[0] != SOCKS5_VERSION {
        return Err(Error::Protocol("invalid SOCKS5 version"));
    }

    let method_count = header[1] as usize;
    if method_count == 0 {
        return Err(Error::Protocol("SOCKS5 method list is empty"));
    }

    let mut methods = vec![0_u8; method_count];
    read_exact(stream, &mut methods).await?;

    if !methods.contains(&METHOD_NO_AUTH) {
        stream
            .write_all(&[SOCKS5_VERSION, METHOD_NOT_ACCEPTABLE])
            .await
            .map_err(|_| Error::Io("failed to write SOCKS5 auth negotiation response"))?;
        return Err(Error::Unsupported("SOCKS5 auth method is not supported"));
    }

    stream
        .write_all(&[SOCKS5_VERSION, METHOD_NO_AUTH])
        .await
        .map_err(|_| Error::Io("failed to write SOCKS5 auth negotiation response"))?;

    Ok(())
}

async fn read_request<S>(stream: &mut S) -> Result<Socks5Request, Error>
where
    S: AsyncSocket,
{
    let mut header = [0_u8; 4];
    read_exact(stream, &mut header).await?;

    if header[0] != SOCKS5_VERSION {
        return Err(Error::Protocol("invalid SOCKS5 request version"));
    }

    let address = match read_address(stream, header[3]).await {
        Ok(address) => address,
        Err(Error::Unsupported(_)) => {
            write_reply(stream, Socks5Reply::AddressTypeNotSupported).await?;
            return Err(Error::Unsupported("SOCKS5 address type is not supported"));
        }
        Err(error) => return Err(error),
    };

    let mut port = [0_u8; 2];
    read_exact(stream, &mut port).await?;

    let port = u16::from_be_bytes(port);

    match header[1] {
        CMD_CONNECT => Ok(Socks5Request::Connect(Session::new(
            0,
            address,
            port,
            Network::Tcp,
            ProtocolType::Socks5,
        ))),
        CMD_UDP_ASSOCIATE => Ok(Socks5Request::UdpAssociate(Socks5UdpAssociateRequest {
            client_hint: address,
            client_port: port,
        })),
        _ => {
            write_reply(stream, Socks5Reply::CommandNotSupported).await?;
            Err(Error::Unsupported("SOCKS5 command is not supported"))
        }
    }
}
