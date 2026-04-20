use alloc::string::String;
use alloc::vec;

use zero_core::{Address, Error, InboundHandler, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::shared::{
    read_exact, write_reply, Socks5Reply, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CMD_CONNECT,
    METHOD_NOT_ACCEPTABLE, METHOD_NO_AUTH, SOCKS5_VERSION,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct Socks5Inbound;

impl Socks5Inbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Socks5
    }

    pub async fn accept_request<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        negotiate_method(stream).await?;
        let (target, port) = read_connect_request(stream).await?;

        Ok(Session::new(
            0,
            target,
            port,
            Network::Tcp,
            ProtocolType::Socks5,
        ))
    }

    pub async fn send_response<S>(&self, stream: &mut S, reply: Socks5Reply) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        write_reply(stream, reply).await
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

async fn read_connect_request<S>(stream: &mut S) -> Result<(Address, u16), Error>
where
    S: AsyncSocket,
{
    let mut header = [0_u8; 4];
    read_exact(stream, &mut header).await?;

    if header[0] != SOCKS5_VERSION {
        return Err(Error::Protocol("invalid SOCKS5 request version"));
    }

    if header[1] != CMD_CONNECT {
        write_reply(stream, Socks5Reply::CommandNotSupported).await?;
        return Err(Error::Unsupported("SOCKS5 command is not supported"));
    }

    let address = match header[3] {
        ATYP_IPV4 => {
            let mut bytes = [0_u8; 4];
            read_exact(stream, &mut bytes).await?;
            Address::Ipv4(bytes)
        }
        ATYP_DOMAIN => {
            let mut length = [0_u8; 1];
            read_exact(stream, &mut length).await?;

            let domain_length = length[0] as usize;
            if domain_length == 0 {
                return Err(Error::Protocol("SOCKS5 domain must not be empty"));
            }

            let mut domain = vec![0_u8; domain_length];
            read_exact(stream, &mut domain).await?;

            let domain = String::from_utf8(domain)
                .map_err(|_| Error::Protocol("SOCKS5 domain is not valid UTF-8"))?;
            Address::Domain(domain)
        }
        ATYP_IPV6 => {
            let mut bytes = [0_u8; 16];
            read_exact(stream, &mut bytes).await?;
            Address::Ipv6(bytes)
        }
        _ => {
            write_reply(stream, Socks5Reply::AddressTypeNotSupported).await?;
            return Err(Error::Unsupported("SOCKS5 address type is not supported"));
        }
    };

    let mut port = [0_u8; 2];
    read_exact(stream, &mut port).await?;

    Ok((address, u16::from_be_bytes(port)))
}
