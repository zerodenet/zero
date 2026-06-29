// Hysteria2 inbound protocol — inbound.rs

use alloc::string::String;
use alloc::vec::Vec;
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

    pub fn authenticate_client(&self, salt: &[u8; 32], auth_frame: &[u8]) -> Result<(), Error> {
        let client_hmac = crate::shared::parse_auth_frame(auth_frame)?;
        if crate::shared::verify_hmac(&self.password, salt, &client_hmac) {
            Ok(())
        } else {
            Err(Error::Protocol("hysteria2: authentication failed"))
        }
    }

    pub fn auth_ok_response(&self) -> Vec<u8> {
        crate::shared::build_auth_ok()
    }

    pub fn auth_error_response(&self, message: &str) -> Vec<u8> {
        crate::shared::build_auth_error(message)
    }

    pub async fn authenticate_connection<S>(
        &self,
        stream: &mut S,
        salt: &[u8; 32],
    ) -> Result<(), Error>
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
    pub async fn authenticate_quic_connection<S>(
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
