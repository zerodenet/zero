use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::shared::{
    read_exact, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CMD_CONNECT, METHOD_NO_AUTH,
    REP_ADDRESS_TYPE_NOT_SUPPORTED, REP_COMMAND_NOT_SUPPORTED, REP_CONNECTION_NOT_ALLOWED,
    REP_GENERAL_FAILURE, REP_HOST_UNREACHABLE, REP_SUCCEEDED, SOCKS5_VERSION,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct Socks5Outbound;

impl Socks5Outbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Socks5
    }

    pub async fn establish_tunnel<S>(&self, stream: &mut S, session: &Session) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        stream
            .write_all(&[SOCKS5_VERSION, 0x01, METHOD_NO_AUTH])
            .await
            .map_err(|_| Error::Io("failed to write SOCKS5 outbound auth negotiation"))?;

        let mut auth = [0_u8; 2];
        read_exact(stream, &mut auth).await?;
        if auth[0] != SOCKS5_VERSION {
            return Err(Error::Protocol("invalid SOCKS5 outbound auth version"));
        }
        if auth[1] != METHOD_NO_AUTH {
            return Err(Error::Unsupported(
                "SOCKS5 upstream auth method is not supported",
            ));
        }

        let request = build_connect_request(session)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write SOCKS5 outbound connect request"))?;

        read_connect_response(stream).await
    }
}

fn build_connect_request(session: &Session) -> Result<Vec<u8>, Error> {
    let mut request = vec![SOCKS5_VERSION, CMD_CONNECT, 0x00];

    match &session.target {
        Address::Ipv4(bytes) => {
            request.push(ATYP_IPV4);
            request.extend_from_slice(bytes);
        }
        Address::Ipv6(bytes) => {
            request.push(ATYP_IPV6);
            request.extend_from_slice(bytes);
        }
        Address::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() {
                return Err(Error::Protocol("SOCKS5 outbound domain must not be empty"));
            }
            if bytes.len() > u8::MAX as usize {
                return Err(Error::Unsupported("SOCKS5 outbound domain is too long"));
            }

            request.push(ATYP_DOMAIN);
            request.push(bytes.len() as u8);
            request.extend_from_slice(bytes);
        }
    }

    request.extend_from_slice(&session.port.to_be_bytes());

    Ok(request)
}

async fn read_connect_response<S>(stream: &mut S) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut header = [0_u8; 4];
    read_exact(stream, &mut header).await?;

    if header[0] != SOCKS5_VERSION {
        return Err(Error::Protocol("invalid SOCKS5 outbound response version"));
    }

    if header[1] != REP_SUCCEEDED {
        return Err(match header[1] {
            REP_GENERAL_FAILURE => Error::Route("SOCKS5 upstream general failure"),
            REP_CONNECTION_NOT_ALLOWED => Error::Route("SOCKS5 upstream rejected connection"),
            REP_HOST_UNREACHABLE => Error::Route("SOCKS5 upstream host unreachable"),
            REP_COMMAND_NOT_SUPPORTED => {
                Error::Unsupported("SOCKS5 upstream command is not supported")
            }
            REP_ADDRESS_TYPE_NOT_SUPPORTED => {
                Error::Unsupported("SOCKS5 upstream address type is not supported")
            }
            _ => Error::Protocol("SOCKS5 upstream returned an unknown reply"),
        });
    }

    let address_len = match header[3] {
        ATYP_IPV4 => 4,
        ATYP_IPV6 => 16,
        ATYP_DOMAIN => {
            let mut len = [0_u8; 1];
            read_exact(stream, &mut len).await?;
            len[0] as usize
        }
        _ => {
            return Err(Error::Protocol(
                "invalid SOCKS5 outbound response address type",
            ))
        }
    };

    let mut discard = vec![0_u8; address_len + 2];
    read_exact(stream, &mut discard).await?;

    Ok(())
}
