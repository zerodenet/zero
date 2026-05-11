use alloc::string::String;

use zero_core::{Error, InboundHandler, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

use crate::shared::{
    read_addon, read_address, read_exact, CMD_MUX, CMD_TCP, CMD_UDP, VLESS_VERSION,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessInbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUser {
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
}

impl VlessUser {
    pub fn new() -> Self {
        Self {
            credential_id: None,
            principal_key: None,
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
        let (mut session, id) = read_request(stream).await?;
        let Some(user) = auth.find_user(&id) else {
            return Err(Error::Unsupported("VLESS user is not authorized"));
        };

        let mut session_auth = SessionAuth::new("vless");
        session_auth.credential_id = user.credential_id;
        session_auth.principal_key = user.principal_key;
        session.auth = Some(session_auth);

        Ok(session)
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
}

impl<S> InboundHandler<S> for VlessInbound
where
    S: AsyncSocket,
{
    async fn handshake(&self, _stream: &mut S) -> Result<Session, Error> {
        Err(Error::Config("VLESS inbound requires a user store"))
    }
}

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

    match command[0] {
        CMD_TCP => Ok((
            Session::new(0, target, port, Network::Tcp, ProtocolType::Vless),
            id,
        )),
        CMD_UDP => Ok((
            Session::new(0, target, port, Network::Udp, ProtocolType::Vless),
            id,
        )),
        CMD_MUX => Err(Error::Unsupported("VLESS MUX command is not supported")),
        _ => Err(Error::Unsupported("VLESS command is not supported")),
    }
}
