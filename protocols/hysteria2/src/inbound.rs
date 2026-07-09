// Hysteria2 inbound protocol — inbound.rs

use alloc::string::String;
use alloc::vec::Vec;
use core::future::Future;
#[cfg(all(feature = "tokio", feature = "crypto"))]
use tokio::task::JoinSet;
use zero_core::{Error, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

/// Hysteria2 inbound handler — validates client auth and dispatches streams.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hysteria2Inbound;

/// Per-user configuration for Hysteria2 authentication.
#[derive(Debug, Clone)]
pub struct Hysteria2User {
    pub password: String,
}

/// Protocol-owned validated inbound profile.
///
/// Proxy listener code owns QUIC accept and task scheduling; this profile owns
/// Hysteria2 authentication material and protocol response framing.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone)]
pub struct Hysteria2InboundProfile {
    password: String,
}

/// Protocol-owned TCP stream accept/response helper.
///
/// Proxy code owns QUIC connection scheduling, while this type owns Hysteria2
/// TCP connect request parsing and connect response framing.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hysteria2InboundTcpAcceptor {
    inbound: Hysteria2Inbound,
}

#[cfg(all(feature = "tokio", feature = "crypto"))]
pub struct Hysteria2AcceptedQuicConnection {
    conn: std::sync::Arc<quinn::Connection>,
    tcp_acceptor: Hysteria2InboundTcpAcceptor,
}

#[cfg(all(feature = "tokio", feature = "crypto"))]
pub trait Hysteria2AcceptedQuicDispatcher<S> {
    type Error;

    async fn dispatch_udp_session(
        &mut self,
        conn: std::sync::Arc<quinn::Connection>,
        responder: crate::udp::Hysteria2InboundUdpResponder,
        tasks: &mut JoinSet<Result<(), Self::Error>>,
    ) -> Result<(), Self::Error>;

    async fn dispatch_tcp_stream(
        &mut self,
        session: Session,
        stream: S,
        tasks: &mut JoinSet<Result<(), Self::Error>>,
    ) -> Result<(), Self::Error>;

    async fn dispatch_stream_task_result(
        &mut self,
        result: Result<Result<(), Self::Error>, tokio::task::JoinError>,
    ) -> Result<(), Self::Error>;
}

#[cfg(all(feature = "tokio", feature = "crypto"))]
impl Hysteria2AcceptedQuicConnection {
    pub fn new(conn: quinn::Connection) -> Self {
        Self {
            conn: std::sync::Arc::new(conn),
            tcp_acceptor: Hysteria2InboundTcpAcceptor::new(),
        }
    }

    pub fn connection(&self) -> std::sync::Arc<quinn::Connection> {
        self.conn.clone()
    }

    pub fn accept_udp_session(&self) -> crate::udp::Hysteria2InboundUdpResponder {
        Hysteria2Inbound.accept_udp_session()
    }

    pub async fn accept_next_tcp_stream<S, F>(
        &self,
        stream_factory: F,
    ) -> Result<Option<(Session, S)>, Error>
    where
        S: AsyncSocket,
        F: FnOnce(quinn::SendStream, quinn::RecvStream) -> S,
    {
        let (send, recv) = self
            .conn
            .accept_bi()
            .await
            .map_err(|_| Error::Io("hysteria2: accept tcp stream"))?;
        let mut stream = stream_factory(send, recv);
        let session = self.tcp_acceptor.accept_stream(&mut stream).await?;
        Ok(Some((session, stream)))
    }

    pub async fn dispatch_session<S, F, D>(
        &self,
        stream_factory: F,
        dispatcher: &mut D,
    ) -> Result<(), D::Error>
    where
        S: AsyncSocket + Send + 'static,
        F: Fn(quinn::SendStream, quinn::RecvStream) -> S + Copy,
        D: Hysteria2AcceptedQuicDispatcher<S>,
        D::Error: From<Error> + Send + 'static,
    {
        let mut stream_tasks = JoinSet::new();
        dispatcher
            .dispatch_udp_session(
                self.connection(),
                self.accept_udp_session(),
                &mut stream_tasks,
            )
            .await?;

        loop {
            tokio::select! {
                accepted_stream = self.accept_next_tcp_stream(stream_factory) => {
                    match accepted_stream? {
                        Some((session, stream)) => {
                            dispatcher
                                .dispatch_tcp_stream(session, stream, &mut stream_tasks)
                                .await?;
                        }
                        None => break,
                    }
                }
                result = stream_tasks.join_next(), if !stream_tasks.is_empty() => {
                    if let Some(result) = result {
                        dispatcher.dispatch_stream_task_result(result).await?;
                    }
                }
            }
        }

        stream_tasks.abort_all();
        Ok(())
    }

    pub async fn dispatch_session_with_handlers<
        S,
        F,
        Udp,
        UdpFut,
        Tcp,
        TcpFut,
        TaskResult,
        TaskResultFut,
        E,
    >(
        &self,
        stream_factory: F,
        mut on_udp_session: Udp,
        mut on_tcp_stream: Tcp,
        mut on_stream_task_result: TaskResult,
    ) -> Result<(), E>
    where
        S: AsyncSocket + Send + 'static,
        F: Fn(quinn::SendStream, quinn::RecvStream) -> S + Copy,
        Udp: FnMut(
            std::sync::Arc<quinn::Connection>,
            crate::udp::Hysteria2InboundUdpResponder,
            &mut JoinSet<Result<(), E>>,
        ) -> UdpFut,
        UdpFut: Future<Output = Result<(), E>>,
        Tcp: FnMut(Session, S, &mut JoinSet<Result<(), E>>) -> TcpFut,
        TcpFut: Future<Output = Result<(), E>>,
        TaskResult: FnMut(Result<Result<(), E>, tokio::task::JoinError>) -> TaskResultFut,
        TaskResultFut: Future<Output = Result<(), E>>,
        E: From<Error> + Send + 'static,
    {
        let mut stream_tasks = JoinSet::new();
        on_udp_session(
            self.connection(),
            self.accept_udp_session(),
            &mut stream_tasks,
        )
        .await?;

        loop {
            tokio::select! {
                accepted_stream = self.accept_next_tcp_stream(stream_factory) => {
                    match accepted_stream? {
                        Some((session, stream)) => {
                            on_tcp_stream(session, stream, &mut stream_tasks).await?;
                        }
                        None => break,
                    }
                }
                result = stream_tasks.join_next(), if !stream_tasks.is_empty() => {
                    if let Some(result) = result {
                        on_stream_task_result(result).await?;
                    }
                }
            }
        }

        stream_tasks.abort_all();
        Ok(())
    }
}

impl Hysteria2InboundTcpAcceptor {
    pub fn new() -> Self {
        Self {
            inbound: Hysteria2Inbound,
        }
    }

    pub async fn accept_stream<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        self.inbound.accept_tcp_stream(stream).await
    }

    pub async fn send_ok<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.inbound.send_connect_ok(stream).await
    }

    pub async fn send_error<S>(&self, stream: &mut S, message: &str) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.inbound.send_connect_error(stream, message).await
    }
}

#[cfg(feature = "crypto")]
impl Hysteria2InboundProfile {
    pub fn from_config(password: &str) -> Self {
        Self {
            password: String::from(password),
        }
    }

    pub fn from_config_parts(password: &str) -> Self {
        Self::from_config(password)
    }

    pub fn from_config_password(password: &str) -> Self {
        Self::from_config_parts(password)
    }

    fn authenticate_client(&self, salt: &[u8; 32], auth_frame: &[u8]) -> Result<(), Error> {
        let client_hmac = crate::shared::parse_auth_frame(auth_frame)?;
        if crate::shared::verify_hmac(&self.password, salt, &client_hmac) {
            Ok(())
        } else {
            Err(Error::Protocol("hysteria2: authentication failed"))
        }
    }

    fn auth_ok_response(&self) -> Vec<u8> {
        crate::shared::build_auth_ok()
    }

    fn auth_error_response(&self, message: &str) -> Vec<u8> {
        crate::shared::build_auth_error(message)
    }

    async fn authenticate_connection<S>(&self, stream: &mut S, salt: &[u8; 32]) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let mut auth_buf = [0u8; 64];
        let n = stream
            .read(&mut auth_buf)
            .await
            .map_err(|_| Error::Io("hysteria2: read auth"))?;
        if n == 0 {
            return Err(Error::Protocol("hysteria2: EOF on auth stream"));
        }

        if self.authenticate_client(salt, &auth_buf[..n]).is_err() {
            let err_resp = self.auth_error_response("authentication failed");
            let _ = stream.write_all(&err_resp).await;
            return Err(Error::Protocol("hysteria2: auth failed"));
        }

        let ok_resp = self.auth_ok_response();
        stream
            .write_all(&ok_resp)
            .await
            .map_err(|_| Error::Io("hysteria2: write auth ok"))
    }

    #[cfg(all(feature = "tokio", feature = "crypto"))]
    async fn authenticate_quic_connection<S>(
        &self,
        conn: &quinn::Connection,
        stream: &mut S,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let mut salt = [0u8; 32];
        conn.export_keying_material(&mut salt, b"hysteria2 auth", &[])
            .map_err(|_| Error::Io("hysteria2 key export failed"))?;

        self.authenticate_connection(stream, &salt).await
    }

    #[cfg(all(feature = "tokio", feature = "crypto"))]
    async fn accept_authenticated_quic_connection<S, F>(
        &self,
        conn: &quinn::Connection,
        stream_factory: F,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
        F: FnOnce(quinn::SendStream, quinn::RecvStream) -> S,
    {
        let (send, recv) = conn
            .accept_bi()
            .await
            .map_err(|_| Error::Io("hysteria2: accept auth stream"))?;
        let mut auth_stream = stream_factory(send, recv);
        self.authenticate_quic_connection(conn, &mut auth_stream)
            .await
    }

    #[cfg(all(feature = "tokio", feature = "crypto"))]
    pub async fn accept_authenticated_quic_session<S, F>(
        &self,
        conn: quinn::Connection,
        stream_factory: F,
    ) -> Result<Hysteria2AcceptedQuicConnection, Error>
    where
        S: AsyncSocket,
        F: FnOnce(quinn::SendStream, quinn::RecvStream) -> S,
    {
        self.accept_authenticated_quic_connection(&conn, stream_factory)
            .await?;
        Ok(Hysteria2AcceptedQuicConnection::new(conn))
    }
}

#[cfg(feature = "crypto")]
pub fn inbound_profile_from_config_password(password: &str) -> Hysteria2InboundProfile {
    Hysteria2InboundProfile::from_config_password(password)
}

/// Trait for looking up Hysteria2 users by password validation.
pub trait Hysteria2UserStore {
    fn validate_password(&self, hmac: &[u8; 32], salt: &[u8; 32]) -> Option<&Hysteria2User>;
}

impl Hysteria2Inbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Hysteria2
    }

    #[cfg(feature = "tokio")]
    pub fn udp_session(&self) -> crate::udp::Hysteria2InboundUdpSession {
        crate::udp::Hysteria2InboundUdpSession::new()
    }

    #[cfg(feature = "tokio")]
    pub fn udp_responder(&self) -> crate::udp::Hysteria2InboundUdpResponder {
        crate::udp::Hysteria2InboundUdpResponder::new(self.udp_session())
    }

    #[cfg(feature = "tokio")]
    pub fn accept_udp_session(&self) -> crate::udp::Hysteria2InboundUdpResponder {
        self.udp_responder()
    }

    pub fn accept_tcp_connect_header(&self, header: &[u8]) -> Result<Session, Error> {
        let (target, port) = crate::shared::parse_tcp_connect_header(header)?;
        Ok(Session::new(
            0,
            target,
            port,
            Network::Tcp,
            ProtocolType::Hysteria2,
        ))
    }

    pub async fn accept_tcp_stream<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        let mut header_buf = [0u8; 512];
        let n = stream
            .read(&mut header_buf)
            .await
            .map_err(|_| Error::Io("hysteria2: read tcp connect header"))?;
        if n == 0 {
            return Err(Error::Protocol("hysteria2: EOF on tcp connect stream"));
        }
        self.accept_tcp_connect_header(&header_buf[..n])
    }

    pub fn connect_ok_response(&self) -> Vec<u8> {
        crate::shared::build_connect_ok()
    }

    pub fn connect_error_response(&self, message: &str) -> Vec<u8> {
        crate::shared::build_connect_error(message)
    }

    pub async fn send_connect_ok<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let response = self.connect_ok_response();
        stream
            .write_all(&response)
            .await
            .map_err(|_| Error::Io("hysteria2: write connect ok"))
    }

    pub async fn send_connect_error<S>(&self, stream: &mut S, message: &str) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let response = self.connect_error_response(message);
        stream
            .write_all(&response)
            .await
            .map_err(|_| Error::Io("hysteria2: write connect error"))
    }

    /// Validate client authentication using HMAC-SHA256(password, salt).
    pub fn validate_auth(
        &self,
        hmac: &[u8; 32],
        salt: &[u8; 32],
        store: &impl Hysteria2UserStore,
    ) -> Result<Session, Error> {
        store
            .validate_password(hmac, salt)
            .ok_or(Error::Protocol("hysteria2: authentication failed"))?;

        let auth = SessionAuth::new("hysteria2");
        let mut session = Session::new(
            0,
            zero_core::Address::Domain(String::new()),
            0,
            zero_core::Network::Tcp,
            ProtocolType::Hysteria2,
        );
        session.auth = Some(auth);
        Ok(session)
    }
}
