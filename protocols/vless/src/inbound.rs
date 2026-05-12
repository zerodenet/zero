use alloc::string::String;

use zero_core::{Address, Error, InboundHandler, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

#[cfg(feature = "reality")]
use crate::flow::{flow_from_byte, flow_read_request, is_aead_flow};
use crate::mux::MuxServer;
use crate::shared::{
    read_addon, read_address, read_exact, CMD_MUX, CMD_TCP, CMD_UDP, VLESS_VERSION,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessInbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUser {
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
    pub flow: Option<&'static str>,
}

impl VlessUser {
    pub fn new() -> Self {
        Self {
            credential_id: None,
            principal_key: None,
            flow: None,
        }
    }
}

impl Default for VlessUser {
    fn default() -> Self {
        Self::new()
    }
}

pub trait VlessUserStore {
    fn find_user(&self, id: &[u8; 16]) -> Option<VlessUser>;
}

impl VlessInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vless
    }

    pub async fn accept_tcp_with_auth<S, A>(
        &self,
        stream: &mut S,
        auth: &A,
    ) -> Result<Session, Error>
    where
        S: AsyncSocket,
        A: VlessUserStore,
    {
        #[cfg(feature = "reality")]
        {
            let (mut session, id) = read_request_with_flow(stream).await?;
            let Some(user) = auth.find_user(&id) else {
                return Err(Error::Unsupported("VLESS user is not authorized"));
            };
            let mut sa = SessionAuth::new("vless");
            sa.credential_id = user.credential_id;
            sa.principal_key = user.principal_key;
            session.auth = Some(sa);
            Ok(session)
        }
        #[cfg(not(feature = "reality"))]
        {
            let (mut session, id) = read_request(stream).await?;
            let Some(user) = auth.find_user(&id) else {
                return Err(Error::Unsupported("VLESS user is not authorized"));
            };
            let mut sa = SessionAuth::new("vless");
            sa.credential_id = user.credential_id;
            sa.principal_key = user.principal_key;
            session.auth = Some(sa);
            Ok(session)
        }
    }

    pub async fn send_response<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        stream
            .write_all(&[VLESS_VERSION, 0x00])
            .await
            .map_err(|_| Error::Io("failed to write VLESS response"))
    }

    pub async fn handshake_with_auth<S, A>(
        &self,
        stream: &mut S,
        auth: &A,
    ) -> Result<Session, Error>
    where
        S: AsyncSocket,
        A: VlessUserStore,
    {
        let session = self.accept_tcp_with_auth(stream, auth).await?;
        self.send_response(stream).await?;
        Ok(session)
    }

    pub async fn accept_mux_header<S, A>(
        &self,
        stream: &mut S,
        auth: &A,
    ) -> Result<MuxServer, Error>
    where
        S: AsyncSocket,
        A: VlessUserStore,
    {
        let (_session, id) = read_request_mux(stream).await?;
        let Some(_user) = auth.find_user(&id) else {
            return Err(Error::Unsupported("VLESS user is not authorized"));
        };
        self.send_response(stream).await?;
        Ok(MuxServer::new())
    }

    /// Check if a Session returned by accept_tcp_with_auth is a MUX session.
    pub fn is_mux_session(session: &Session) -> bool {
        session.port == 0 && matches!(&session.target, Address::Domain(d) if d.is_empty())
    }
}

impl<S> InboundHandler<S> for VlessInbound
where
    S: AsyncSocket,
{
    async fn handshake(&self, _stream: &mut S) -> Result<Session, Error> {
        Err(Error::Config("VLESS inbound requires a user store"))
    }
}

// ── read_request (non-reality) ──

#[cfg(not(feature = "reality"))]
async fn read_request<S>(stream: &mut S) -> Result<(Session, [u8; 16]), Error>
where
    S: AsyncSocket,
{
    let mut version = [0_u8; 1];
    read_exact(stream, &mut version).await?;
    if version[0] != VLESS_VERSION {
        return Err(Error::Protocol("unsupported VLESS version"));
    }
    let mut id = [0_u8; 16];
    read_exact(stream, &mut id).await?;
    read_addon(stream).await?;
    let mut command = [0_u8; 1];
    read_exact(stream, &mut command).await?;
    let mut port = [0_u8; 2];
    read_exact(stream, &mut port).await?;
    let port = u16::from_be_bytes(port);
    if port == 0 {
        return Err(Error::Protocol("VLESS target port must not be 0"));
    }
    let mut atyp = [0_u8; 1];
    read_exact(stream, &mut atyp).await?;
    let target = read_address(stream, atyp[0]).await?;
    command_to_session(command[0], target, port, id)
}

// ── read_request_with_flow (reality) ──

#[cfg(feature = "reality")]
async fn read_request_with_flow<S>(stream: &mut S) -> Result<(Session, [u8; 16]), Error>
where
    S: AsyncSocket,
{
    let mut version = [0_u8; 1];
    read_exact(stream, &mut version).await?;
    if version[0] != VLESS_VERSION {
        return Err(Error::Protocol("unsupported VLESS version"));
    }
    let mut id = [0_u8; 16];
    read_exact(stream, &mut id).await?;
    let mut flow_byte = [0_u8; 1];
    read_exact(stream, &mut flow_byte).await?;
    let flow = flow_from_byte(flow_byte[0]);

    if is_aead_flow(flow) {
        let (command, port, target) = flow_read_request(stream, flow, &id).await?;
        command_to_session(command, target, port, id)
    } else {
        let mut command = [0_u8; 1];
        read_exact(stream, &mut command).await?;
        let mut port = [0_u8; 2];
        read_exact(stream, &mut port).await?;
        let port = u16::from_be_bytes(port);
        if port == 0 {
            return Err(Error::Protocol("VLESS target port must not be 0"));
        }
        let mut atyp = [0_u8; 1];
        read_exact(stream, &mut atyp).await?;
        let target = read_address(stream, atyp[0]).await?;
        command_to_session(command[0], target, port, id)
    }
}

// ── read_request_mux (used by accept_mux_header) ──

async fn read_request_mux<S>(stream: &mut S) -> Result<(Session, [u8; 16]), Error>
where
    S: AsyncSocket,
{
    let mut version = [0_u8; 1];
    read_exact(stream, &mut version).await?;
    if version[0] != VLESS_VERSION {
        return Err(Error::Protocol("unsupported VLESS version"));
    }
    let mut id = [0_u8; 16];
    read_exact(stream, &mut id).await?;

    #[cfg(feature = "reality")]
    {
        let mut flow_byte = [0_u8; 1];
        read_exact(stream, &mut flow_byte).await?;
        let flow = flow_from_byte(flow_byte[0]);
        if is_aead_flow(flow) {
            let (command, _port, _target) = flow_read_request(stream, flow, &id).await?;
            if command != CMD_MUX {
                return Err(Error::Protocol("VLESS MUX expected"));
            }
        } else {
            let mut command = [0_u8; 1];
            read_exact(stream, &mut command).await?;
            if command[0] != CMD_MUX {
                return Err(Error::Protocol("VLESS MUX expected"));
            }
            let mut port = [0_u8; 2];
            read_exact(stream, &mut port).await?;
            let mut atyp = [0_u8; 1];
            read_exact(stream, &mut atyp).await?;
            let _ = read_address(stream, atyp[0]).await?;
        }
    }
    #[cfg(not(feature = "reality"))]
    {
        read_addon(stream).await?;
        let mut command = [0_u8; 1];
        read_exact(stream, &mut command).await?;
        if command[0] != CMD_MUX {
            return Err(Error::Protocol("VLESS MUX expected"));
        }
        let mut port = [0_u8; 2];
        read_exact(stream, &mut port).await?;
        let mut atyp = [0_u8; 1];
        read_exact(stream, &mut atyp).await?;
        let _ = read_address(stream, atyp[0]).await?;
    }

    Ok((
        Session::new(
            0,
            Address::Domain(String::new()),
            0,
            Network::Tcp,
            ProtocolType::Vless,
        ),
        id,
    ))
}

// ── command dispatch helper ──

fn command_to_session(
    command: u8,
    target: Address,
    port: u16,
    id: [u8; 16],
) -> Result<(Session, [u8; 16]), Error> {
    match command {
        CMD_TCP => Ok((
            Session::new(0, target, port, Network::Tcp, ProtocolType::Vless),
            id,
        )),
        CMD_UDP => Ok((
            Session::new(0, target, port, Network::Udp, ProtocolType::Vless),
            id,
        )),
        CMD_MUX => Ok((
            Session::new(
                0,
                Address::Domain(String::new()),
                0,
                Network::Tcp,
                ProtocolType::Vless,
            ),
            id,
        )),
        _ => Err(Error::Unsupported("VLESS command is not supported")),
    }
}
