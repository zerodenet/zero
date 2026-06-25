use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vmess::VmessAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, UdpDispatch, VmessDatagramSend, VmessRelaySend,
};
use crate::runtime::Proxy;

fn parse_vmess_udp_identity(
    id: &str,
    cipher: &str,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> Result<([u8; 16], vmess::VmessCipher), FlowFailure> {
    let uuid = vmess::parse_uuid(id).map_err(|error| FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error)),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })?;
    let cipher = vmess::VmessCipher::from_name(cipher).ok_or_else(|| FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("vmess unknown cipher: {cipher}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })?;
    Ok((uuid, cipher))
}

impl VmessAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            mux_idle_timeout_secs: _,
            tls,
            ws,
            grpc,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let tag_owned = (*tag).to_string();
        let (uuid, vmess_cipher) = parse_vmess_udp_identity(
            id,
            cipher,
            "udp_vmess_parse_identity",
            Some((server, *port)),
        )?;
        dispatch
            .send_vmess_datagram(VmessDatagramSend {
                proxy,
                session,
                server,
                port: *port,
                uuid,
                cipher_name: cipher,
                cipher: vmess_cipher,
                mux_concurrency: *mux_concurrency,
                tls: *tls,
                ws: *ws,
                grpc: *grpc,
                payload,
            })
            .await?;

        Ok(FlowStartResult::ManagedFlow {
            session_id: session.id,
            tag: tag_owned,
        })
    }

    pub(super) async fn start_udp_relay_final_hop_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            tls,
            ws,
            grpc,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let tag_owned = (*tag).to_string();
        let (uuid, vmess_cipher) = parse_vmess_udp_identity(
            id,
            cipher,
            "udp_vmess_relay_final_hop_parse_identity",
            Some((server, *port)),
        )?;
        dispatch
            .send_vmess_relay(VmessRelaySend {
                proxy,
                session,
                carrier,
                server,
                port: *port,
                uuid,
                cipher: vmess_cipher,
                tls: *tls,
                ws: *ws,
                grpc: *grpc,
                payload,
            })
            .await?;

        Ok(FlowStartResult::ManagedFlow {
            session_id: session.id,
            tag: tag_owned,
        })
    }
}
