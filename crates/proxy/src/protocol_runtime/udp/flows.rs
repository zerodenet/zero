use zero_core::Session;

use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
use crate::runtime::Proxy;

#[cfg(feature = "shadowsocks")]
pub(crate) struct ShadowsocksUdpFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) cipher: &'a str,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "mieru")]
pub(crate) struct MieruUdpRelayFlow<'a> {
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) username: &'a str,
    pub(crate) password: &'a str,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "vless")]
pub(crate) struct VlessUdpFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) id: &'a str,
    pub(crate) flow: Option<&'a str>,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) reality: Option<&'a zero_config::RealityConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) h2: Option<&'a zero_config::H2Config>,
    pub(crate) http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
    pub(crate) split_http: Option<&'a zero_config::SplitHttpConfig>,
    pub(crate) quic: Option<&'a zero_config::QuicConfig>,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "vless")]
pub(crate) struct VlessUdpRelayTwoStream<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) post_carrier: crate::transport::RelayCarrier,
    pub(crate) get_carrier: crate::transport::RelayCarrier,
    pub(crate) id: &'a str,
    pub(crate) split_http: &'a zero_config::SplitHttpConfig,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "vless")]
pub(crate) struct VlessUdpRelayFinalHop<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) id: &'a str,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) reality: Option<&'a zero_config::RealityConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) h2: Option<&'a zero_config::H2Config>,
    pub(crate) http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
    pub(crate) split_http: Option<&'a zero_config::SplitHttpConfig>,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "vmess")]
pub(crate) struct VmessUdpFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) id: &'a str,
    pub(crate) cipher: &'a str,
    pub(crate) mux_concurrency: Option<u32>,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "vmess")]
pub(crate) struct VmessUdpRelayFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) id: &'a str,
    pub(crate) cipher: &'a str,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    #[cfg(feature = "shadowsocks")]
    pub(crate) async fn start_shadowsocks_udp_flow(
        &mut self,
        flow: ShadowsocksUdpFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .shadowsocks
            .send_existing(crate::protocol_runtime::udp::SsSendExisting {
                chain_tasks: &mut self.chain_tasks,
                session_id: flow.session.id,
                proxy: flow.proxy,
                server: flow.server,
                port: flow.port,
                password: flow.password,
                cipher: flow.cipher,
                target: &flow.session.target,
                target_port: flow.session.port,
                payload: flow.payload,
            })
            .await
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) async fn start_hysteria2_udp_flow(
        &mut self,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        client_fingerprint: Option<&str>,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .hysteria2
            .send_existing(crate::protocol_runtime::udp::H2SendExisting {
                chain_tasks: &mut self.chain_tasks,
                session_id: session.id,
                server,
                port,
                password,
                client_fingerprint,
                target: &session.target,
                target_port: session.port,
                payload,
            })
            .await
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "trojan")]
    pub(crate) async fn start_trojan_udp_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        relay_chain: bool,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .trojan
            .send_existing(crate::protocol_runtime::udp::TrojanSendExisting {
                chain_tasks: &mut self.chain_tasks,
                session_id: session.id,
                proxy,
                session,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
                relay_chain,
                target: &session.target,
                target_port: session.port,
                payload,
            })
            .await
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "trojan")]
    pub(crate) async fn start_trojan_udp_relay_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .trojan
            .send_relay_existing(crate::protocol_runtime::udp::TrojanRelayExisting {
                chain_tasks: &mut self.chain_tasks,
                session_id: session.id,
                stream: carrier.stream,
                tls_server_name: None,
                proxy,
                session,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
                target: &session.target,
                target_port: session.port,
                payload,
            })
            .await
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "mieru")]
    pub(crate) async fn start_mieru_udp_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        username: &str,
        password: &str,
        relay_chain: bool,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .mieru
            .send_existing(
                &mut self.chain_tasks,
                session.id,
                proxy,
                session,
                server,
                port,
                username,
                password,
                relay_chain,
                &session.target,
                session.port,
                payload,
            )
            .await
    }

    #[cfg(feature = "mieru")]
    pub(crate) async fn start_mieru_udp_relay_flow(
        &mut self,
        flow: MieruUdpRelayFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .mieru
            .send_relay_existing(
                &mut self.chain_tasks,
                flow.session.id,
                flow.carrier.stream,
                flow.server,
                flow.port,
                flow.username,
                flow.password,
                &flow.session.target,
                flow.session.port,
                flow.payload,
            )
            .await
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn start_vless_udp_flow(
        &mut self,
        flow: VlessUdpFlow<'_>,
    ) -> Result<(), FlowFailure> {
        let transport = crate::protocol_runtime::vless_udp::VlessUdpTransport {
            tls: flow.tls,
            reality: flow.reality,
            ws: flow.ws,
            grpc: flow.grpc,
            h2: flow.h2,
            http_upgrade: flow.http_upgrade,
            split_http: flow.split_http,
            quic: flow.quic,
        };
        self.protocol_state
            .vless
            .start_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::vless_udp::VlessUdpStartFlow {
                    proxy: flow.proxy,
                    session: flow.session,
                    server: flow.server,
                    port: flow.port,
                    id: flow.id,
                    flow: flow.flow,
                    transport,
                    payload: flow.payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_upstream",
                error,
                upstream: Some((flow.server.to_string(), flow.port)),
            })?;
        Ok(())
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn start_vless_udp_relay_two_stream(
        &mut self,
        flow: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .vless
            .start_relay_two_stream(
                &mut self.chain_tasks,
                crate::protocol_runtime::vless_udp::VlessUdpRelayTwoStream {
                    proxy: flow.proxy,
                    session: flow.session,
                    post_carrier: flow.post_carrier,
                    get_carrier: flow.get_carrier,
                    id: flow.id,
                    split_http: flow.split_http,
                    payload: flow.payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_relay_chain",
                error,
                upstream: None,
            })?;
        Ok(())
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn start_vless_udp_relay_final_hop(
        &mut self,
        flow: VlessUdpRelayFinalHop<'_>,
    ) -> Result<(), FlowFailure> {
        let transport = crate::protocol_runtime::vless_udp::VlessUdpTransport {
            tls: flow.tls,
            reality: flow.reality,
            ws: flow.ws,
            grpc: flow.grpc,
            h2: flow.h2,
            http_upgrade: flow.http_upgrade,
            split_http: flow.split_http,
            quic: None,
        };
        self.protocol_state
            .vless
            .start_relay_final_hop(
                &mut self.chain_tasks,
                crate::protocol_runtime::vless_udp::VlessUdpRelayFinalHop {
                    proxy: flow.proxy,
                    session: flow.session,
                    carrier: flow.carrier,
                    id: flow.id,
                    transport,
                    payload: flow.payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_relay_chain",
                error,
                upstream: None,
            })?;
        Ok(())
    }

    #[cfg(feature = "vmess")]
    pub(crate) async fn start_vmess_udp_flow(
        &mut self,
        flow: VmessUdpFlow<'_>,
    ) -> Result<(), FlowFailure> {
        let transport = crate::protocol_runtime::vmess_udp::VmessUdpTransport {
            tls: flow.tls,
            ws: flow.ws,
            grpc: flow.grpc,
        };
        self.protocol_state
            .vmess
            .start_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::vmess_udp::VmessUdpStartFlow {
                    proxy: flow.proxy,
                    session: flow.session,
                    server: flow.server,
                    port: flow.port,
                    id: flow.id,
                    cipher: flow.cipher,
                    mux_concurrency: flow.mux_concurrency,
                    transport,
                    payload: flow.payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_upstream",
                error,
                upstream: Some((flow.server.to_string(), flow.port)),
            })?;
        Ok(())
    }

    #[cfg(feature = "vmess")]
    pub(crate) async fn start_vmess_udp_relay_flow(
        &mut self,
        flow: VmessUdpRelayFlow<'_>,
    ) -> Result<(), FlowFailure> {
        let transport = crate::protocol_runtime::vmess_udp::VmessUdpTransport {
            tls: flow.tls,
            ws: flow.ws,
            grpc: flow.grpc,
        };
        self.protocol_state
            .vmess
            .start_relay_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::vmess_udp::VmessUdpRelayFlow {
                    proxy: flow.proxy,
                    session: flow.session,
                    carrier: flow.carrier,
                    server: flow.server,
                    port: flow.port,
                    id: flow.id,
                    cipher: flow.cipher,
                    transport,
                    payload: flow.payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_relay_chain",
                error,
                upstream: None,
            })?;
        Ok(())
    }
}
