use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::shared::{
    read_address, read_exact, write_address, CMD_CONNECT, CMD_UDP_ASSOCIATE, METHOD_NO_AUTH,
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

        negotiate_no_auth(stream).await?;

        let request = build_connect_request(session)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write SOCKS5 outbound connect request"))?;

        let _ = read_response(stream).await?;
        Ok(())
    }

    pub async fn establish_udp_association<S>(
        &self,
        stream: &mut S,
    ) -> Result<(Address, u16), Error>
    where
        S: AsyncSocket,
    {
        negotiate_no_auth(stream).await?;

        let request = vec![
            SOCKS5_VERSION,
            CMD_UDP_ASSOCIATE,
            0x00,
            0x01,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write SOCKS5 outbound udp associate request"))?;

        read_response(stream).await
    }
}

fn build_connect_request(session: &Session) -> Result<Vec<u8>, Error> {
    let mut request = vec![SOCKS5_VERSION, CMD_CONNECT, 0x00];
    write_address(&mut request, &session.target)?;

    request.extend_from_slice(&session.port.to_be_bytes());

    Ok(request)
}

async fn negotiate_no_auth<S>(stream: &mut S) -> Result<(), Error>
where
    S: AsyncSocket,
{
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

    Ok(())
}

async fn read_response<S>(stream: &mut S) -> Result<(Address, u16), Error>
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

    let address = read_address(stream, header[3]).await?;

    let mut port = [0_u8; 2];
    read_exact(stream, &mut port).await?;

    Ok((address, u16::from_be_bytes(port)))
}
