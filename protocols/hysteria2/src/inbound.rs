// Hysteria2 inbound protocol — inbound.rs

use alloc::string::String;
use zero_core::{Error, ProtocolType, Session, SessionAuth};

/// Hysteria2 inbound handler — validates client auth and dispatches streams.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hysteria2Inbound;

/// Per-user configuration for Hysteria2 authentication.
#[derive(Debug, Clone)]
pub struct Hysteria2User {
    pub password: String,
}

/// Trait for looking up Hysteria2 users by password validation.
pub trait Hysteria2UserStore {
    fn validate_password(&self, hmac: &[u8; 32], salt: &[u8; 32]) -> Option<&Hysteria2User>;
}

impl Hysteria2Inbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Hysteria2
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
            .ok_or_else(|| Error::Protocol("hysteria2: authentication failed"))?;

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
