//! Trojan inbound protocol handler.

use zero_core::{Error, InboundStreamUdpRelay, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

use super::shared::{read_password, read_request, CMD_TCP, CMD_UDP};
use crate::udp::TrojanInboundUdpResponder;

/// Trojan inbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanInboundProfile {
    password: String,
}

impl TrojanInboundProfile {
    pub fn from_config(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
        }
    }

    pub fn from_config_parts(password: impl Into<String>) -> Self {
        Self::from_config(password)
    }

    pub fn from_config_password(password: impl Into<String>) -> Self {
        Self::from_config_parts(password)
    }

    pub fn inbound_auth(&self) -> SessionAuth {
        TrojanInbound.inbound_auth(self.password.clone())
    }

    async fn accept<S: AsyncSocket>(
        &self,
        inbound: TrojanInbound,
        stream: &mut S,
    ) -> Result<TrojanAccept, Error> {
        inbound
            .accept(stream, core::slice::from_ref(&self.password))
            .await
    }

    pub async fn accept_session<S: AsyncSocket>(
        &self,
        inbound: TrojanInbound,
        stream: &mut S,
    ) -> Result<Session, Error> {
        let accept = self.accept(inbound, stream).await?;
        let mut session = accept.session;
        session.apply_auth(self.inbound_auth());
        Ok(session)
    }

    pub async fn accept_client<S: AsyncSocket>(
        &self,
        inbound: TrojanInbound,
        mut stream: S,
    ) -> Result<TrojanInboundAcceptedSession<S>, Error> {
        let session = self.accept_session(inbound, &mut stream).await?;
        Ok(TrojanInboundAcceptedSession::from_session_stream(
            session, stream,
        ))
    }

    pub async fn accept_client_owned<S: AsyncSocket>(
        self,
        inbound: TrojanInbound,
        mut stream: S,
    ) -> Result<TrojanInboundAcceptedSession<S>, Error> {
        let password = self.password;
        let accept = inbound
            .accept(&mut stream, core::slice::from_ref(&password))
            .await?;
        let mut session = accept.session;
        session.apply_auth(TrojanInbound.inbound_auth(password));
        Ok(TrojanInboundAcceptedSession::from_session_stream(
            session, stream,
        ))
    }
}

/// Result of accepting a Trojan connection.
struct TrojanAccept {
    session: Session,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrojanInboundSessionKind {
    Tcp,
    Udp,
}

enum TrojanInboundAcceptedSessionState<S> {
    Tcp {
        session: Session,
        stream: S,
    },
    Udp {
        session: Session,
        relay: TrojanInboundUdpRelay<S>,
    },
}

pub struct TrojanInboundAcceptedSession<S> {
    state: TrojanInboundAcceptedSessionState<S>,
}

pub struct TrojanInboundUdpRelay<S> {
    auth: Option<SessionAuth>,
    responder: TrojanInboundUdpResponder,
    stream: S,
}

fn classify_inbound_session(session: &Session) -> TrojanInboundSessionKind {
    match session.network {
        Network::Udp => TrojanInboundSessionKind::Udp,
        Network::Tcp => TrojanInboundSessionKind::Tcp,
    }
}

impl<S> TrojanInboundUdpRelay<S> {
    fn new(stream: S, responder: TrojanInboundUdpResponder, auth: Option<SessionAuth>) -> Self {
        Self {
            auth,
            responder,
            stream,
        }
    }

    fn into_parts(self) -> (S, TrojanInboundUdpResponder, Option<SessionAuth>) {
        (self.stream, self.responder, self.auth)
    }
}

impl<S> InboundStreamUdpRelay for TrojanInboundUdpRelay<S>
where
    S: AsyncSocket,
{
    type Stream = S;
    type Responder = TrojanInboundUdpResponder;

    fn into_stream_udp_parts(self) -> (Self::Stream, Self::Responder, Option<SessionAuth>) {
        self.into_parts()
    }
}

impl<S> TrojanInboundAcceptedSession<S> {
    fn tcp(session: Session, stream: S) -> Self {
        Self {
            state: TrojanInboundAcceptedSessionState::Tcp { session, stream },
        }
    }

    fn udp(session: Session, relay: TrojanInboundUdpRelay<S>) -> Self {
        Self {
            state: TrojanInboundAcceptedSessionState::Udp { session, relay },
        }
    }

    fn from_session_stream(session: Session, stream: S) -> Self {
        match classify_inbound_session(&session) {
            TrojanInboundSessionKind::Tcp => Self::tcp(session, stream),
            TrojanInboundSessionKind::Udp => {
                let auth = session.auth.clone();
                Self::udp(
                    session,
                    TrojanInboundUdpRelay::new(stream, TrojanInbound.accept_udp_session(), auth),
                )
            }
        }
    }

    async fn dispatch<Tcp, TcpFut, Udp, UdpFut, E>(self, tcp: Tcp, udp: Udp) -> Result<(), E>
    where
        Tcp: FnOnce(Session, S) -> TcpFut,
        TcpFut: core::future::Future<Output = Result<(), E>>,
        Udp: FnOnce(Session, TrojanInboundUdpRelay<S>) -> UdpFut,
        UdpFut: core::future::Future<Output = Result<(), E>>,
    {
        match self.state {
            TrojanInboundAcceptedSessionState::Tcp { session, stream } => {
                tcp(session, stream).await
            }
            TrojanInboundAcceptedSessionState::Udp { session, relay } => udp(session, relay).await,
        }
    }
}

#[async_trait::async_trait]
impl<S> zero_core::InboundStreamRoute for TrojanInboundAcceptedSession<S>
where
    S: AsyncSocket,
{
    type TcpStream = S;
    type UdpRelay = TrojanInboundUdpRelay<S>;

    async fn dispatch_inbound_route<E, FTcp, FTcpFut, FUdp, FUdpFut>(
        self,
        on_tcp: FTcp,
        on_udp: FUdp,
    ) -> Result<(), E>
    where
        FTcp: FnOnce(Session, Self::TcpStream) -> FTcpFut + Send,
        FTcpFut: core::future::Future<Output = Result<(), E>> + Send,
        FUdp: FnOnce(Session, Self::UdpRelay) -> FUdpFut + Send,
        FUdpFut: core::future::Future<Output = Result<(), E>> + Send,
    {
        self.dispatch(on_tcp, on_udp).await
    }
}

impl TrojanInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    pub fn inbound_auth(&self, password: impl Into<String>) -> SessionAuth {
        let mut auth = SessionAuth::new("trojan");
        auth.principal_key = Some(password.into());
        auth
    }

    /// Accept a Trojan TCP connection.
    ///
    /// Reads password hash + command + target address from the stream.
    /// The password is validated against `passwords` (hex SHA224 hashes).
    async fn accept<S: AsyncSocket>(
        &self,
        stream: &mut S,
        passwords: &[String],
    ) -> Result<TrojanAccept, Error> {
        let password_hash = read_password(stream).await?;

        // Validate password.
        if !passwords.iter().any(|p| {
            #[cfg(feature = "crypto")]
            {
                use sha2::{Digest, Sha224};
                password_hash == super::shared::hex::encode(&Sha224::digest(p.as_bytes()))
            }
            #[cfg(not(feature = "crypto"))]
            {
                let _ = (p, &password_hash);
                false
            }
        }) {
            return Err(Error::Protocol("trojan: invalid password"));
        }

        let (cmd, addr, port) = read_request(stream).await?;

        let network = match cmd {
            CMD_TCP => Network::Tcp,
            CMD_UDP => Network::Udp,
            _ => return Err(Error::Protocol("trojan: unsupported command")),
        };

        Ok(TrojanAccept {
            session: Session::new(0, addr, port, network, ProtocolType::Trojan),
        })
    }
}
