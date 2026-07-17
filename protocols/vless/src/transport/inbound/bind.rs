use std::io;
use std::path::{Path, PathBuf};

use super::super::options::VlessQuicBindOptionsRef;
use super::super::profile::VlessQuicBindProfile;
use zero_transport::RuntimeError;

use zero_transport::quic;

#[derive(Debug, Clone, Default)]
pub struct VlessInboundBindPlan {
    quic_cert_path: Option<String>,
    quic_key_path: Option<String>,
    quic_alpn_protocols: Vec<Vec<u8>>,
    source_dir: Option<PathBuf>,
}

impl VlessInboundBindPlan {
    pub fn from_options_refs(
        source_dir: Option<&Path>,
        quic: Option<VlessQuicBindOptionsRef<'_>>,
    ) -> Self {
        let quic = quic.map(VlessQuicBindProfile::from);
        Self::from_quic_profile(source_dir, quic.as_ref())
    }

    fn from_quic_profile(source_dir: Option<&Path>, quic: Option<&VlessQuicBindProfile>) -> Self {
        Self {
            quic_cert_path: quic.and_then(|config| config.cert_path.clone()),
            quic_key_path: quic.and_then(|config| config.key_path.clone()),
            quic_alpn_protocols: quic
                .map(VlessQuicBindProfile::alpn_protocols)
                .unwrap_or_default(),
            source_dir: source_dir.map(PathBuf::from),
        }
    }

    pub async fn bind(&self, listen_addr: &str) -> Result<Option<quic::QuicInbound>, RuntimeError> {
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
                    &self.quic_alpn_protocols,
                )
                .await?,
            )),
            (None, None) => Ok(None),
            _ => Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless quic inbound bind requires both cert_path and key_path",
            ))),
        }
    }
}
