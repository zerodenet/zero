use alloc::borrow::ToOwned;
use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error, ProtocolType, Session};
use zero_traits::{AsyncSocket, TcpTunnelProtocol, UdpRelayProtocol};

use crate::shared::{
    read_address, read_exact, write_address, CMD_CONNECT, CMD_UDP_ASSOCIATE, METHOD_NO_AUTH,
    METHOD_USERNAME_PASSWORD, REP_ADDRESS_TYPE_NOT_SUPPORTED, REP_COMMAND_NOT_SUPPORTED,
    REP_CONNECTION_NOT_ALLOWED, REP_GENERAL_FAILURE, REP_HOST_UNREACHABLE, REP_SUCCEEDED,
    SOCKS5_VERSION, USERPASS_STATUS_SUCCESS, USERPASS_VERSION,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct Socks5Outbound;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Socks5OutboundAuth<'a> {
    pub username: &'a str,
    pub password: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpFlowResume {
    username: Option<alloc::string::String>,
    password: Option<alloc::string::String>,
}

impl Socks5UdpFlowResume {
    pub fn new(username: Option<&str>, password: Option<&str>) -> Self {
        Self {
            username: username.map(ToOwned::to_owned),
            password: password.map(ToOwned::to_owned),
        }
    }

    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Socks5TcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub auth: Option<Socks5OutboundAuth<'a>>,
}

impl Socks5Outbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Socks5
    }

    pub async fn establish_tunnel<S>(&self, stream: &mut S, session: &Session) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.establish_tunnel_with_auth(stream, session, None).await
    }

    pub async fn establish_tunnel_with_auth<S>(
        &self,
        stream: &mut S,
        session: &Session,
        auth: Option<Socks5OutboundAuth<'_>>,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        negotiate_auth(stream, auth).await?;

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
        self.establish_udp_association_with_auth(stream, None).await
    }

    pub async fn establish_udp_association_with_auth<S>(
        &self,
        stream: &mut S,
        auth: Option<Socks5OutboundAuth<'_>>,
    ) -> Result<(Address, u16), Error>
    where
        S: AsyncSocket,
    {
        negotiate_auth(stream, auth).await?;

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

impl<'a> TcpTunnelProtocol<Socks5TcpTunnelTarget<'a>> for Socks5Outbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &Socks5TcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_tunnel_with_auth(stream, target.session, target.auth)
            .await
    }
}

/// Target parameters for SOCKS5 UDP relay association.
#[derive(Debug, Clone, Copy)]
pub struct Socks5UdpRelayTarget<'a> {
    pub auth: Option<Socks5OutboundAuth<'a>>,
}

impl<'a> UdpRelayProtocol<Socks5UdpRelayTarget<'a>> for Socks5Outbound {
    type Error = Error;
    type RelayEndpoint = (Address, u16);

    async fn establish_udp_relay<S>(
        &self,
        control_stream: &mut S,
        target: &Socks5UdpRelayTarget<'a>,
    ) -> Result<Self::RelayEndpoint, Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_udp_association_with_auth(control_stream, target.auth)
            .await
    }
}

fn build_connect_request(session: &Session) -> Result<Vec<u8>, Error> {
    let mut request = vec![SOCKS5_VERSION, CMD_CONNECT, 0x00];
    write_address(&mut request, &session.target)?;

    request.extend_from_slice(&session.port.to_be_bytes());

    Ok(request)
}

async fn negotiate_auth<S>(
    stream: &mut S,
    auth: Option<Socks5OutboundAuth<'_>>,
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    match auth {
        Some(auth) => {
            validate_outbound_auth(auth)?;
            stream
                .write_all(&[SOCKS5_VERSION, 0x01, METHOD_USERNAME_PASSWORD])
                .await
                .map_err(|_| Error::Io("failed to write SOCKS5 outbound auth negotiation"))?;
        }
        None => {
            stream
                .write_all(&[SOCKS5_VERSION, 0x01, METHOD_NO_AUTH])
                .await
                .map_err(|_| Error::Io("failed to write SOCKS5 outbound auth negotiation"))?;
        }
    }

    let mut selected = [0_u8; 2];
    read_exact(stream, &mut selected).await?;
    if selected[0] != SOCKS5_VERSION {
        return Err(Error::Protocol("invalid SOCKS5 outbound auth version"));
    }

    match (auth, selected[1]) {
        (None, METHOD_NO_AUTH) => Ok(()),
        (Some(auth), METHOD_USERNAME_PASSWORD) => {
            authenticate_username_password(stream, auth).await
        }
        _ => Err(Error::Unsupported(
            "SOCKS5 upstream auth method is not supported",
        )),
    }
}

async fn authenticate_username_password<S>(
    stream: &mut S,
    auth: Socks5OutboundAuth<'_>,
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let username = auth.username.as_bytes();
    let password = auth.password.as_bytes();
    let mut request = Vec::with_capacity(3 + username.len() + password.len());
    request.push(USERPASS_VERSION);
    request.push(username.len() as u8);
    request.extend_from_slice(username);
    request.push(password.len() as u8);
    request.extend_from_slice(password);
    stream
        .write_all(&request)
        .await
        .map_err(|_| Error::Io("failed to write SOCKS5 username/password credentials"))?;

    let mut response = [0_u8; 2];
    read_exact(stream, &mut response).await?;
    if response[0] != USERPASS_VERSION {
        return Err(Error::Protocol(
            "invalid SOCKS5 username/password auth response version",
        ));
    }
    if response[1] != USERPASS_STATUS_SUCCESS {
        return Err(Error::Unsupported(
            "SOCKS5 upstream username/password authentication failed",
        ));
    }
    Ok(())
}

fn validate_outbound_auth(auth: Socks5OutboundAuth<'_>) -> Result<(), Error> {
    validate_credential_part(auth.username, "username")?;
    validate_credential_part(auth.password, "password")
}

fn validate_credential_part(value: &str, field: &'static str) -> Result<(), Error> {
    let len = value.len();
    if len == 0 {
        return Err(Error::Config(match field {
            "username" => "SOCKS5 username must not be empty",
            "password" => "SOCKS5 password must not be empty",
            _ => "SOCKS5 credential must not be empty",
        }));
    }
    if len > u8::MAX as usize {
        return Err(Error::Config(match field {
            "username" => "SOCKS5 username is too long",
            "password" => "SOCKS5 password is too long",
            _ => "SOCKS5 credential is too long",
        }));
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
