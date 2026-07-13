use std::io;
use std::path::{Path, PathBuf};

use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

#[cfg(feature = "hysteria2")]
use zero_core::InboundClientResponse;
#[cfg(feature = "hysteria2")]
use zero_traits::AsyncSocket;

#[cfg(feature = "hysteria2")]
use super::{OwnedHysteria2InboundProfile, OwnedHysteria2InboundTcpResponseProtocol};

#[derive(Debug, Clone)]
pub struct OwnedHysteria2InboundBindPlan {
    cert_path: String,
    key_path: String,
    source_dir: Option<PathBuf>,
}

impl OwnedHysteria2InboundBindPlan {
    pub fn from_config_ref(
        source_dir: Option<&Path>,
        cert_path: Option<&str>,
        key_path: Option<&str>,
    ) -> Self {
        Self {
            cert_path: cert_path.unwrap_or("certs/fullchain.pem").to_owned(),
            key_path: key_path.unwrap_or("certs/privkey.pem").to_owned(),
            source_dir: source_dir.map(PathBuf::from),
        }
    }

    pub fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        match protocol {
            InboundProtocolConfig::Hysteria2 {
                cert_path,
                key_path,
                ..
            } => Ok(Self::from_config_ref(
                source_dir,
                cert_path.as_deref(),
                key_path.as_deref(),
            )),
            _ => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hysteria2 inbound bind plan received non-hysteria2 inbound config",
            ))),
        }
    }

    pub async fn bind(&self, listen_addr: &str) -> Result<crate::quic::QuicInbound, EngineError> {
        crate::quic::QuicInbound::bind(
            listen_addr,
            &self.cert_path,
            &self.key_path,
            self.source_dir.as_deref(),
        )
        .await
    }
}

#[async_trait::async_trait]
impl crate::inbound_route::ProtocolInboundBindPlan for OwnedHysteria2InboundBindPlan {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        Self::from_protocol_config(protocol, source_dir)
    }

    async fn bind(
        &self,
        listen_addr: &str,
    ) -> Result<crate::inbound_route::TransportInboundBindTarget, EngineError> {
        Ok(crate::inbound_route::TransportInboundBindTarget::Quic(
            OwnedHysteria2InboundBindPlan::bind(self, listen_addr).await?,
        ))
    }
}

#[cfg(feature = "hysteria2")]
pub fn inbound_profile_from_protocol(
    protocol: &InboundProtocolConfig,
) -> Result<OwnedHysteria2InboundProfile, EngineError> {
    match protocol {
        InboundProtocolConfig::Hysteria2 { password, .. } => Ok(OwnedHysteria2InboundProfile::new(
            hysteria2::inbound::inbound_profile_from_config_password(password.as_str()),
        )),
        _ => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "hysteria2 inbound profile received non-hysteria2 inbound config",
        ))),
    }
}

#[cfg(feature = "hysteria2")]
pub fn inbound_tcp_acceptor() -> OwnedHysteria2InboundTcpResponseProtocol {
    OwnedHysteria2InboundTcpResponseProtocol {
        protocol: hysteria2::inbound::Hysteria2InboundTcpAcceptor::new(),
    }
}

#[cfg(feature = "hysteria2")]
impl OwnedHysteria2InboundProfile {
    fn new(protocol: hysteria2::inbound::Hysteria2InboundProfile) -> Self {
        Self { protocol }
    }

    pub fn tcp_response_protocol(&self) -> OwnedHysteria2InboundTcpResponseProtocol {
        inbound_tcp_acceptor()
    }
}

#[cfg(feature = "hysteria2")]
impl<S> InboundClientResponse<S> for OwnedHysteria2InboundTcpResponseProtocol
where
    S: AsyncSocket,
{
    async fn send_ok(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_ok(client).await
    }

    async fn send_blocked(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_blocked(client).await
    }

    async fn send_upstream_failure(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_upstream_failure(client).await
    }
}
