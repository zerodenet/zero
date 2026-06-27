// Hysteria2 inbound protocol — inbound.rs

use alloc::string::String;
use alloc::vec::Vec;
use zero_core::{Error, Network, ProtocolType, Session, SessionAuth};

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

    pub fn connect_ok_response(&self) -> Vec<u8> {
        crate::shared::build_connect_ok()
    }

    pub fn connect_error_response(&self, message: &str) -> Vec<u8> {
        crate::shared::build_connect_error(message)
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
