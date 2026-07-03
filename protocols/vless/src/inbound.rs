use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use zero_core::{Address, Error, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

#[cfg(feature = "reality")]
use crate::flow::{flow_from_byte, flow_read_request, is_aead_flow};
use crate::mux::{MuxServer, VlessInboundMuxContext};
#[cfg(not(feature = "reality"))]
use crate::shared::read_addon;
use crate::shared::{
    parse_uuid, read_address, read_exact, CMD_MUX, CMD_TCP, CMD_UDP, VLESS_VERSION,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessInbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUser {
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
    pub up_bps: Option<u64>,
    pub down_bps: Option<u64>,
    pub flow: Option<&'static str>,
}

impl VlessUser {
    pub fn new() -> Self {
        Self {
            credential_id: None,
            principal_key: None,
            up_bps: None,
            down_bps: None,
            flow: None,
        }
    }

    pub fn from_config(
        flow: Option<&str>,
        credential_id: Option<String>,
        principal_key: Option<String>,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<Self, Error> {
        #[cfg(feature = "reality")]
        let flow = flow.map(crate::flow::parse_flow).transpose()?;
        #[cfg(not(feature = "reality"))]
        let flow = {
            if flow.is_some() {
                return Err(Error::Unsupported(
                    "VLESS flow requires the `reality` feature",
                ));
            }
            None
        };
        Ok(Self {
            credential_id,
            principal_key,
            up_bps,
            down_bps,
            flow,
        })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessConfiguredUser {
    pub id: [u8; 16],
    pub user: VlessUser,
}

pub type VlessInboundUserConfigParts = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<u64>,
    Option<u64>,
);

pub type BorrowedVlessInboundUserConfigParts<'a> = (
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    Option<u64>,
    Option<u64>,
);

impl VlessConfiguredUser {
    pub fn from_config(
        id: &str,
        flow: Option<&str>,
        credential_id: Option<String>,
        principal_key: Option<String>,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<Self, Error> {
        Ok(Self {
            id: parse_uuid(id)?,
            user: VlessUser::from_config(flow, credential_id, principal_key, up_bps, down_bps)?,
        })
    }
}

pub struct VlessConfiguredUsers<'a> {
    users: &'a [VlessConfiguredUser],
}

impl<'a> VlessConfiguredUsers<'a> {
    pub fn new(users: &'a [VlessConfiguredUser]) -> Self {
        Self { users }
    }
}

impl VlessUserStore for VlessConfiguredUsers<'_> {
    fn find_user(&self, id: &[u8; 16]) -> Option<VlessUser> {
        self.users
            .iter()
            .find(|user| &user.id == id)
            .map(|user| user.user.clone())
    }
}

pub struct VlessAcceptedSession {
    session: Session,
    mux_context: VlessInboundMuxContext,
}

pub struct VlessAcceptedClient<S> {
    accepted: VlessAcceptedSession,
    stream: S,
}

pub enum VlessAcceptedClientRoute<S> {
    Tcp {
        session: Session,
        stream: S,
    },
    #[cfg(feature = "reality")]
    Udp {
        session: Session,
        relay: VlessInboundUdpRelay<S>,
    },
    #[cfg(feature = "reality")]
    Mux {
        mux_server: crate::mux::VlessInboundMuxServer,
        stream: S,
    },
}

pub struct VlessInboundUdpRelay<S> {
    auth: Option<SessionAuth>,
    responder: crate::udp::VlessInboundUdpResponder,
    stream: S,
}

pub trait VlessAcceptedClientRouteDispatcher<S> {
    type Error;

    async fn dispatch_tcp_session(
        &mut self,
        session: Session,
        stream: S,
    ) -> Result<(), Self::Error>;

    #[cfg(feature = "reality")]
    async fn dispatch_udp_session(
        &mut self,
        session: Session,
        relay: VlessInboundUdpRelay<S>,
    ) -> Result<(), Self::Error>;

    #[cfg(feature = "reality")]
    async fn dispatch_mux_session(
        &mut self,
        mux_server: crate::mux::VlessInboundMuxServer,
        stream: S,
    ) -> Result<(), Self::Error>;
}

pub struct VlessClientAcceptError<S> {
    error: Error,
    stream: S,
}

pub trait VlessFallbackCapture {
    type Stream;

    fn into_vless_fallback_replay(self) -> VlessFallbackReplay<Self::Stream>;
}

pub struct VlessFallbackReplay<S> {
    stream: S,
    replay_head: Vec<u8>,
}

pub enum VlessFallbackAlpnDecision<S> {
    Replay(VlessFallbackReplay<S>),
    Continue { stream: S, replay_head: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessFallbackAlpnPolicy {
    expected: Option<String>,
}

impl VlessFallbackAlpnPolicy {
    pub fn from_expected(expected: Option<&str>) -> Self {
        Self {
            expected: expected.map(String::from),
        }
    }

    pub fn matches_client_alpns<'a, I>(&self, client_alpns: I) -> bool
    where
        I: IntoIterator<Item = &'a str>,
    {
        let Some(expected) = self.expected.as_deref() else {
            return false;
        };
        client_alpns.into_iter().any(|alpn| alpn == expected)
    }
}

pub fn fallback_alpn_matches<'a, I>(expected: Option<&str>, client_alpns: I) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    VlessFallbackAlpnPolicy::from_expected(expected).matches_client_alpns(client_alpns)
}

pub fn fallback_replay_for_alpns<'a, I, S>(
    expected: Option<&str>,
    client_alpns: I,
    stream: S,
    replay_head: Vec<u8>,
) -> VlessFallbackAlpnDecision<S>
where
    I: IntoIterator<Item = &'a str>,
{
    if fallback_alpn_matches(expected, client_alpns) {
        VlessFallbackAlpnDecision::Replay(VlessFallbackReplay::new(stream, replay_head))
    } else {
        VlessFallbackAlpnDecision::Continue {
            stream,
            replay_head,
        }
    }
}

impl VlessAcceptedSession {
    fn new(session: Session, user_id: [u8; 16]) -> Self {
        Self {
            session,
            mux_context: VlessInboundMuxContext::from_uuid(user_id),
        }
    }

    pub fn into_session(self) -> Session {
        self.session
    }

    pub fn into_parts(self) -> (Session, VlessInboundMuxContext) {
        (self.session, self.mux_context)
    }
}

impl<S> VlessAcceptedClient<S> {
    fn new(accepted: VlessAcceptedSession, stream: S) -> Self {
        Self { accepted, stream }
    }

    pub fn into_parts(self) -> (Session, VlessInboundMuxContext, S) {
        let (session, mux_context) = self.accepted.into_parts();
        (session, mux_context, self.stream)
    }

    pub async fn into_route(self) -> Result<VlessAcceptedClientRoute<S>, Error>
    where
        S: AsyncSocket,
    {
        self.into_route_with_sni(None).await
    }

    pub async fn into_route_with_sni(
        self,
        sni: Option<String>,
    ) -> Result<VlessAcceptedClientRoute<S>, Error>
    where
        S: AsyncSocket,
    {
        let (mut session, mux_context, mut stream) = self.into_parts();
        match classify_inbound_session(&session) {
            VlessInboundSessionKind::Tcp => {
                session.sni = sni;
                Ok(VlessAcceptedClientRoute::Tcp { session, stream })
            }
            VlessInboundSessionKind::Udp => {
                session.sni = sni;
                #[cfg(feature = "reality")]
                {
                    let auth = session.auth.clone();
                    let responder = VlessInbound.accept_udp_session(&mut stream).await?;
                    Ok(VlessAcceptedClientRoute::Udp {
                        session,
                        relay: VlessInboundUdpRelay::new(stream, responder, auth),
                    })
                }
                #[cfg(not(feature = "reality"))]
                {
                    Err(Error::Unsupported(
                        "VLESS UDP requires the `reality` feature",
                    ))
                }
            }
            VlessInboundSessionKind::Mux => {
                #[cfg(feature = "reality")]
                {
                    let auth = session.auth.clone();
                    let mux_server = VlessInbound
                        .accept_mux_session_with_auth(&mut stream, mux_context, auth)
                        .await?;
                    Ok(VlessAcceptedClientRoute::Mux { mux_server, stream })
                }
                #[cfg(not(feature = "reality"))]
                {
                    let _ = mux_context;
                    Err(Error::Unsupported(
                        "VLESS MUX requires the `reality` feature",
                    ))
                }
            }
        }
    }
}

impl<S> VlessInboundUdpRelay<S> {
    fn new(
        stream: S,
        responder: crate::udp::VlessInboundUdpResponder,
        auth: Option<SessionAuth>,
    ) -> Self {
        Self {
            auth,
            responder,
            stream,
        }
    }

    pub fn into_parts(self) -> (S, crate::udp::VlessInboundUdpResponder, Option<SessionAuth>) {
        (self.stream, self.responder, self.auth)
    }
}

impl<S> VlessAcceptedClientRoute<S> {
    #[cfg(feature = "reality")]
    pub async fn dispatch<Tcp, TcpFut, Udp, UdpFut, Mux, MuxFut, E>(
        self,
        tcp: Tcp,
        udp: Udp,
        mux: Mux,
    ) -> Result<(), E>
    where
        Tcp: FnOnce(Session, S) -> TcpFut,
        TcpFut: core::future::Future<Output = Result<(), E>>,
        Udp: FnOnce(Session, VlessInboundUdpRelay<S>) -> UdpFut,
        UdpFut: core::future::Future<Output = Result<(), E>>,
        Mux: FnOnce(crate::mux::VlessInboundMuxServer, S) -> MuxFut,
        MuxFut: core::future::Future<Output = Result<(), E>>,
    {
        match self {
            Self::Tcp { session, stream } => tcp(session, stream).await,
            Self::Udp { session, relay } => udp(session, relay).await,
            Self::Mux { mux_server, stream } => mux(mux_server, stream).await,
        }
    }

    #[cfg(not(feature = "reality"))]
    pub async fn dispatch<Tcp, TcpFut, Udp, UdpFut, Mux, MuxFut, E>(
        self,
        tcp: Tcp,
        _udp: Udp,
        _mux: Mux,
    ) -> Result<(), E>
    where
        Tcp: FnOnce(Session, S) -> TcpFut,
        TcpFut: core::future::Future<Output = Result<(), E>>,
    {
        match self {
            Self::Tcp { session, stream } => tcp(session, stream).await,
        }
    }

    pub async fn dispatch_with<D>(self, dispatcher: &mut D) -> Result<(), D::Error>
    where
        D: VlessAcceptedClientRouteDispatcher<S>,
    {
        match self {
            Self::Tcp { session, stream } => dispatcher.dispatch_tcp_session(session, stream).await,
            #[cfg(feature = "reality")]
            Self::Udp { session, relay } => dispatcher.dispatch_udp_session(session, relay).await,
            #[cfg(feature = "reality")]
            Self::Mux { mux_server, stream } => {
                dispatcher.dispatch_mux_session(mux_server, stream).await
            }
        }
    }
}

impl<S> VlessClientAcceptError<S> {
    fn new(error: Error, stream: S) -> Self {
        Self { error, stream }
    }

    pub fn into_parts(self) -> (Error, S) {
        (self.error, self.stream)
    }

    pub fn into_fallback_replay(self) -> (Error, VlessFallbackReplay<S::Stream>)
    where
        S: VlessFallbackCapture,
    {
        (self.error, self.stream.into_vless_fallback_replay())
    }
}

impl<S> VlessFallbackReplay<S> {
    pub fn new(stream: S, replay_head: Vec<u8>) -> Self {
        Self {
            stream,
            replay_head,
        }
    }

    pub fn into_stream(self) -> S {
        self.stream
    }

    pub fn replay_head(&self) -> &[u8] {
        &self.replay_head
    }

    pub async fn write_replay_head<W>(&self, writer: &mut W) -> Result<(), W::Error>
    where
        W: AsyncSocket,
    {
        if !self.replay_head.is_empty() {
            writer.write_all(&self.replay_head).await?;
        }
        Ok(())
    }

    pub async fn replay_to_upstream<W>(self, writer: &mut W) -> Result<S, W::Error>
    where
        W: AsyncSocket,
    {
        let Self {
            stream,
            replay_head,
        } = self;
        if !replay_head.is_empty() {
            writer.write_all(&replay_head).await?;
        }
        Ok(stream)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessInboundSessionKind {
    Tcp,
    Udp,
    Mux,
}

pub fn classify_inbound_session(session: &Session) -> VlessInboundSessionKind {
    if VlessInbound::is_mux_session(session) {
        VlessInboundSessionKind::Mux
    } else {
        match session.network {
            Network::Udp => VlessInboundSessionKind::Udp,
            Network::Tcp => VlessInboundSessionKind::Tcp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessInboundProfile {
    users: Arc<[VlessConfiguredUser]>,
}

impl VlessInboundProfile {
    pub fn from_users(users: Vec<VlessConfiguredUser>) -> Self {
        Self {
            users: users.into(),
        }
    }

    pub fn from_config_parts<I>(users: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = VlessInboundUserConfigParts>,
    {
        users
            .into_iter()
            .map(
                |(id, flow, credential_id, principal_key, up_bps, down_bps)| {
                    VlessConfiguredUser::from_config(
                        &id,
                        flow.as_deref(),
                        credential_id,
                        principal_key,
                        up_bps,
                        down_bps,
                    )
                },
            )
            .collect::<Result<Vec<_>, Error>>()
            .map(Self::from_users)
    }

    pub fn from_config_users<I, U>(users: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = U>,
        U: IntoVlessInboundUserConfig,
    {
        Self::from_config_parts(users.into_iter().map(U::into_vless_inbound_user_config))
    }

    pub async fn accept_tcp_with_auth_and_id<S>(
        &self,
        inbound: VlessInbound,
        stream: &mut S,
    ) -> Result<(Session, [u8; 16]), Error>
    where
        S: AsyncSocket,
    {
        let auth = VlessConfiguredUsers::new(&self.users);
        inbound.accept_tcp_with_auth_and_id(stream, &auth).await
    }

    pub async fn accept_tcp_with_auth_context<S>(
        &self,
        inbound: VlessInbound,
        stream: &mut S,
    ) -> Result<VlessAcceptedSession, Error>
    where
        S: AsyncSocket,
    {
        let (session, user_id) = self.accept_tcp_with_auth_and_id(inbound, stream).await?;
        Ok(VlessAcceptedSession::new(session, user_id))
    }

    pub async fn accept_client<S>(
        &self,
        inbound: VlessInbound,
        mut stream: S,
    ) -> Result<VlessAcceptedClient<S>, VlessClientAcceptError<S>>
    where
        S: AsyncSocket,
    {
        match self
            .accept_tcp_with_auth_context(inbound, &mut stream)
            .await
        {
            Ok(accepted) => Ok(VlessAcceptedClient::new(accepted, stream)),
            Err(error) => Err(VlessClientAcceptError::new(error, stream)),
        }
    }

    pub async fn accept_tcp_with_auth<S>(
        &self,
        inbound: VlessInbound,
        stream: &mut S,
    ) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        let auth = VlessConfiguredUsers::new(&self.users);
        inbound.accept_tcp_with_auth(stream, &auth).await
    }
}

pub fn inbound_profile_from_config_users<I, U>(users: I) -> Result<VlessInboundProfile, Error>
where
    I: IntoIterator<Item = U>,
    U: IntoVlessInboundUserConfig,
{
    VlessInboundProfile::from_config_users(users)
}

pub trait IntoVlessInboundUserConfig {
    fn into_vless_inbound_user_config(self) -> VlessInboundUserConfigParts;
}

impl IntoVlessInboundUserConfig for VlessInboundUserConfigParts {
    fn into_vless_inbound_user_config(self) -> VlessInboundUserConfigParts {
        self
    }
}

impl IntoVlessInboundUserConfig for BorrowedVlessInboundUserConfigParts<'_> {
    fn into_vless_inbound_user_config(self) -> VlessInboundUserConfigParts {
        let (id, flow, credential_id, principal_key, up_bps, down_bps) = self;
        (
            id.to_owned(),
            flow.map(str::to_owned),
            credential_id.map(str::to_owned),
            principal_key.map(str::to_owned),
            up_bps,
            down_bps,
        )
    }
}

impl VlessInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vless
    }

    #[cfg(feature = "reality")]
    pub async fn accept_mux_session<S>(
        &self,
        stream: &mut S,
        mux_context: crate::mux::VlessInboundMuxContext,
    ) -> Result<crate::mux::VlessInboundMuxServer, Error>
    where
        S: AsyncSocket,
    {
        self.send_response(stream).await?;
        Ok(crate::mux::VlessInboundMuxServer::from_context(mux_context))
    }

    #[cfg(feature = "reality")]
    pub async fn accept_mux_session_with_auth<S>(
        &self,
        stream: &mut S,
        mux_context: crate::mux::VlessInboundMuxContext,
        auth: Option<SessionAuth>,
    ) -> Result<crate::mux::VlessInboundMuxServer, Error>
    where
        S: AsyncSocket,
    {
        self.send_response(stream).await?;
        Ok(crate::mux::VlessInboundMuxServer::from_context_with_auth(
            mux_context,
            auth,
        ))
    }

    /// Accept a VLESS connection, authenticate the user, and return both
    /// the session and the raw UUID (needed for MUX stream encryption).
    pub async fn accept_tcp_with_auth_and_id<S, A>(
        &self,
        stream: &mut S,
        auth: &A,
    ) -> Result<(Session, [u8; 16]), Error>
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
            sa.up_bps = user.up_bps;
            sa.down_bps = user.down_bps;
            session.apply_auth(sa);
            Ok((session, id))
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
            sa.up_bps = user.up_bps;
            sa.down_bps = user.down_bps;
            session.apply_auth(sa);
            Ok((session, id))
        }
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

    pub async fn send_ok<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_response(stream).await
    }

    pub async fn send_blocked<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let _ = stream.shutdown().await;
        Ok(())
    }

    pub async fn send_upstream_failure<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let _ = stream.shutdown().await;
        Ok(())
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

// ── Config adapter ──
