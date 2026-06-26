use zero_core::{Address, Error, Session};
use zero_traits::DatagramCodec;

use crate::protocol_runtime::udp::{ProtocolUdpFlowResume, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;

pub(crate) struct ShadowsocksDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) codec: std::sync::Arc<dyn DatagramCodec<Address, Error = Error>>,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn send_shadowsocks_datagram(
        &mut self,
        request: ShadowsocksDatagramSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.shadowsocks() else {
            return Err(FlowFailure {
                stage: "udp_shadowsocks_resume",
                error: zero_engine::EngineError::Io(std::io::Error::other(
                    "expected Shadowsocks UDP flow resume",
                )),
                upstream: Some((request.server.to_string(), request.port)),
            });
        };
        self.protocol_state
            .start_shadowsocks_udp_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::ShadowsocksUdpFlow {
                    proxy: request.proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    cache_key: resume.cache_key().to_owned(),
                    codec: request.codec.clone(),
                    payload: request.payload,
                },
            )
            .await
    }

    pub(crate) async fn start_shadowsocks_datagram_flow(
        &mut self,
        request: ShadowsocksDatagramSend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let sent = self
            .send_shadowsocks_datagram(ShadowsocksDatagramSend {
                proxy: request.proxy,
                tag: request.tag,
                session: request.session,
                server: request.server,
                port: request.port,
                resume: request.resume.clone(),
                codec: request.codec.clone(),
                payload: request.payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Datagram {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol: ProtocolUdpFlowSnapshot::managed(request.resume),
            }),
            tx_bytes: sent as u64,
        })
    }
}
