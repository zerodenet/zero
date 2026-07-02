use crate::shared::{
    establish_outbound_session, parse_uuid, VmessCipher, VmessOutboundSession, CMD_TCP,
};
use crate::stream::VmessAeadStream;
use zero_core::{Error, ProtocolType, Session};
use zero_traits::{AsyncSocket, TcpSessionProtocol};

#[derive(Debug, Clone, Copy)]
pub struct VmessOutbound;

impl VmessOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vmess
    }

    pub async fn send_tcp_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<(), Error> {
        establish_outbound_session(stream, session, uuid, cipher, CMD_TCP)
            .await
            .map(|_| ())
    }

    pub async fn establish_tcp_session<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<VmessOutboundSession, Error> {
        establish_outbound_session(stream, session, uuid, cipher, CMD_TCP).await
    }
}

/// Target parameters for VMess TCP session.
#[derive(Debug, Clone, Copy)]
pub struct VmessTcpSessionTarget<'a> {
    pub session: &'a Session,
    pub uuid: &'a [u8; 16],
    pub cipher: VmessCipher,
}

/// Parsed VMess identity settings built from external config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmessTcpConnectConfig {
    uuid: [u8; 16],
    cipher_name: String,
    cipher: VmessCipher,
}

impl VmessTcpConnectConfig {
    pub fn from_config(id: &str, cipher: &str) -> Result<Self, Error> {
        let uuid = parse_uuid(id)?;
        let cipher =
            VmessCipher::from_name(cipher).ok_or(Error::Protocol("vmess unknown cipher"))?;
        Ok(Self {
            uuid,
            cipher_name: cipher.name().to_owned(),
            cipher,
        })
    }

    pub fn mux_pool_identity(&self) -> crate::mux::VmessMuxIdentity {
        crate::mux::VmessMuxIdentity::from_parts(self.uuid, self.cipher_name.clone(), self.cipher)
    }

    pub fn tcp_target<'a>(&'a self, session: &'a Session) -> VmessTcpSessionTarget<'a> {
        VmessTcpSessionTarget {
            session,
            uuid: &self.uuid,
            cipher: self.cipher,
        }
    }

    pub async fn establish_tcp_outbound_session<S>(
        &self,
        stream: &mut S,
        session: &Session,
    ) -> Result<VmessOutboundSession, Error>
    where
        S: AsyncSocket,
    {
        VmessOutbound
            .establish_tcp_session(stream, session, &self.uuid, self.cipher)
            .await
    }

    pub async fn establish_tcp_outbound_stream<S>(
        &self,
        mut stream: S,
        session: &Session,
    ) -> Result<VmessAeadStream<S>, Error>
    where
        S: AsyncSocket,
    {
        let vmess_session = self
            .establish_tcp_outbound_session(&mut stream, session)
            .await?;
        self.wrap_tcp_outbound_stream(stream, vmess_session)
    }

    pub fn wrap_tcp_outbound_stream<S>(
        &self,
        stream: S,
        session: VmessOutboundSession,
    ) -> Result<VmessAeadStream<S>, Error> {
        VmessAeadStream::outbound(stream, session)
    }
}

pub fn tcp_connect_config_from_config(
    id: &str,
    cipher: &str,
) -> Result<VmessTcpConnectConfig, Error> {
    VmessTcpConnectConfig::from_config(id, cipher)
}

impl<'a> TcpSessionProtocol<VmessTcpSessionTarget<'a>> for VmessOutbound {
    type Error = Error;
    type Session = VmessOutboundSession;

    async fn establish_tcp_session<S>(
        &self,
        stream: &mut S,
        target: &VmessTcpSessionTarget<'a>,
    ) -> Result<Self::Session, Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_tcp_session(stream, target.session, target.uuid, target.cipher)
            .await
    }
}

pub async fn establish_tcp_outbound_stream<S>(
    mut stream: S,
    session: &Session,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<VmessAeadStream<S>, Error>
where
    S: AsyncSocket,
{
    let vmess_session = VmessOutbound
        .establish_tcp_session(&mut stream, session, uuid, cipher)
        .await?;
    VmessAeadStream::outbound(stream, vmess_session)
}

pub async fn establish_tcp_outbound_session<S>(
    stream: &mut S,
    session: &Session,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<VmessOutboundSession, Error>
where
    S: AsyncSocket,
{
    VmessOutbound
        .establish_tcp_session(stream, session, uuid, cipher)
        .await
}

pub fn wrap_tcp_outbound_stream<S>(
    stream: S,
    session: VmessOutboundSession,
) -> Result<VmessAeadStream<S>, Error> {
    VmessAeadStream::outbound(stream, session)
}
