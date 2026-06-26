use zero_core::{Address, Error, Session};
use zero_traits::DatagramCodec;

use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;

pub(crate) struct ShadowsocksDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) datagram_cache_key: String,
    pub(crate) cipher: shadowsocks::CipherKind,
    pub(crate) codec: std::sync::Arc<dyn DatagramCodec<Address, Error = Error>>,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn send_shadowsocks_datagram(
        &mut self,
        request: ShadowsocksDatagramSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_shadowsocks_udp_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::ShadowsocksUdpFlow {
                    proxy: request.proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    cache_key: request.datagram_cache_key,
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
                password: request.password,
                datagram_cache_key: request.datagram_cache_key.clone(),
                cipher: request.cipher,
                codec: request.codec.clone(),
                payload: request.payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Datagram {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol: ProtocolUdpFlowSnapshot::shadowsocks(
                    request.password,
                    request.datagram_cache_key,
                    request.cipher,
                ),
            }),
            tx_bytes: sent as u64,
        })
    }
}
