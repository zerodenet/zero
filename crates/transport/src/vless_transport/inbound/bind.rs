use std::io;
use std::path::{Path, PathBuf};

use zero_config::{InboundProtocolConfig, QuicConfig};
use zero_engine::EngineError;

use crate::quic;

#[derive(Debug, Clone, Default)]
pub struct OwnedVlessInboundBindPlan {
    quic_cert_path: Option<String>,
    quic_key_path: Option<String>,
    source_dir: Option<PathBuf>,
}

impl OwnedVlessInboundBindPlan {
    fn from_config_ref(source_dir: Option<&Path>, quic: Option<&QuicConfig>) -> Self {
        Self {
            quic_cert_path: quic.and_then(|config| config.cert_path.clone()),
            quic_key_path: quic.and_then(|config| config.key_path.clone()),
            source_dir: source_dir.map(PathBuf::from),
        }
    }

    async fn bind(&self, listen_addr: &str) -> Result<Option<quic::QuicInbound>, EngineError> {
        match (
            self.quic_cert_path.as_deref(),
            self.quic_key_path.as_deref(),
        ) {
            (Some(cert_path), Some(key_path)) => Ok(Some(
                quic::QuicInbound::bind(
                    listen_addr,
                    cert_path,
                    key_path,
                    self.source_dir.as_deref(),
                )
                .await?,
            )),
            (None, None) => Ok(None),
            _ => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless quic inbound bind requires both cert_path and key_path",
            ))),
        }
    }
}

#[async_trait::async_trait]
impl crate::inbound_route::ProtocolInboundBindPlan for OwnedVlessInboundBindPlan {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        match protocol {
            InboundProtocolConfig::Vless { quic, .. } => Ok(Self::from_config_ref(
                source_dir,
                quic.as_ref().map(|config| config.as_ref()),
            )),
            _ => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless inbound bind received non-vless inbound config",
            ))),
        }
    }

    async fn bind(
        &self,
        listen_addr: &str,
    ) -> Result<crate::inbound_route::TransportInboundBindTarget, EngineError> {
        match OwnedVlessInboundBindPlan::bind(self, listen_addr).await? {
            Some(endpoint) => Ok(crate::inbound_route::TransportInboundBindTarget::Quic(
                endpoint,
            )),
            None => Ok(crate::inbound_route::TransportInboundBindTarget::Tcp),
        }
    }
}
