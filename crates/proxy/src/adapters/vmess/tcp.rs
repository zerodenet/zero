use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::vmess::VmessAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

fn parse_vmess_identity(
    id: &str,
    cipher: &str,
) -> Result<([u8; 16], vmess::VmessCipher), EngineError> {
    let uuid = vmess::parse_uuid(id).map_err(|error| {
        EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
    })?;
    let cipher = vmess::VmessCipher::from_name(cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("vmess unknown cipher: {cipher}"),
        ))
    })?;
    Ok((uuid, cipher))
}

impl VmessAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            mux_idle_timeout_secs,
            tls,
            ws,
            grpc,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        let (uuid, vmess_cipher) =
            parse_vmess_identity(id, cipher).map_err(|error| TcpOutboundFailure {
                stage: "connect_upstream_vmess",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            })?;
        match crate::outbound::vmess::connect_tcp(crate::outbound::vmess::VmessTcpConnectRequest {
            proxy,
            session,
            server,
            port: *port,
            uuid,
            cipher_name: cipher,
            cipher: vmess_cipher,
            mux_concurrency: *mux_concurrency,
            mux_idle_timeout_secs: *mux_idle_timeout_secs,
            tls: *tls,
            ws: *ws,
            grpc: *grpc,
        })
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Vmess {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_vmess",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }

    pub(super) async fn apply_relay_hop_impl(
        &self,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let ResolvedLeafOutbound::Vmess { id, cipher, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let (uuid, vmess_cipher) = parse_vmess_identity(id, cipher)?;
        crate::outbound::vmess::apply_tcp_hop(stream, session, uuid, vmess_cipher).await
    }
}
