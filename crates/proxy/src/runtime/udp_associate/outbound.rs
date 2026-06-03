use zero_engine::ResolvedLeafOutbound;

use crate::outbound::socks5::{send_socks5_udp_packet, Socks5UdpAssociation};
use crate::runtime::Proxy;

use super::context::{
    Socks5UdpPacketContext, UdpCandidateContext, UdpCandidateFailure, UdpCandidateStart,
};
use super::sessions::UdpFlowOutbound;

impl Proxy {
    pub(super) async fn start_udp_flow_candidate(
        &self,
        candidate: ResolvedLeafOutbound<'_>,
        context: UdpCandidateContext<'_>,
    ) -> Result<UdpCandidateStart, UdpCandidateFailure> {
        match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                let target_addr = self
                    .protocols
                    .direct_outbound
                    .resolve_target_addr(context.session, self.resolver.as_ref())
                    .await
                    .map_err(|error| UdpCandidateFailure {
                        stage: "resolve_udp_target",
                        error: error.into(),
                        upstream: None,
                    })?;

                let sent = context
                    .relay
                    .send_to_addr(context.payload, target_addr)
                    .await
                    .map_err(|error| UdpCandidateFailure {
                        stage: "udp_direct_send",
                        error: error.into(),
                        upstream: None,
                    })?;

                Ok(UdpCandidateStart::Flow {
                    outbound: UdpFlowOutbound::Direct {
                        tag: tag.unwrap_or("direct").to_owned(),
                        target_addr,
                    },
                    outbound_tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Block { tag } => Ok(UdpCandidateStart::Blocked {
                tag: tag.unwrap_or("block").to_owned(),
            }),
            ResolvedLeafOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let association = Socks5UdpAssociation {
                    tag: tag.to_owned(),
                    server: server.to_owned(),
                    port,
                    auth: username
                        .zip(password)
                        .map(|(u, p)| (u.to_owned(), p.to_owned())),
                };

                let sent = send_socks5_udp_packet(
                    self,
                    context.inbound_tag,
                    &association,
                    context.session,
                    context.payload,
                    context.upstream_association,
                    context.upstream_idle_deadline,
                )
                .await
                .map_err(|error| UdpCandidateFailure {
                    stage: "udp_upstream_send",
                    error,
                    upstream: Some((server.to_owned(), port)),
                })?;

                Ok(UdpCandidateStart::Flow {
                    outbound: UdpFlowOutbound::Socks5 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.map(ToOwned::to_owned),
                        password: password.map(ToOwned::to_owned),
                    },
                    outbound_tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Vless { .. } => Err(UdpCandidateFailure {
                stage: "udp_vless_outbound",
                error: zero_core::Error::Unsupported(
                    "VLESS UDP chain outbound must be handled in VLESS inbound handler",
                )
                .into(),
                upstream: None,
            }),
            #[cfg(feature = "hysteria2")]
            ResolvedLeafOutbound::Hysteria2 {
                tag,
                server,
                port,
                password,
                ..
            } => {
                let sent = crate::outbound::hysteria2::send_h2_udp_packet(
                    self,
                    context.session,
                    server,
                    port,
                    password,
                    &context.session.target,
                    context.session.port,
                    context.payload,
                )
                .await
                .map_err(|error| UdpCandidateFailure {
                    stage: "udp_h2_send",
                    error,
                    upstream: Some((server.to_owned(), port)),
                })?;

                Ok(UdpCandidateStart::Flow {
                    outbound: UdpFlowOutbound::Hysteria2 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                    },
                    outbound_tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "hysteria2"))]
            ResolvedLeafOutbound::Hysteria2 { .. } => Err(UdpCandidateFailure {
                stage: "udp_hysteria2_outbound",
                error: zero_core::Error::Unsupported(
                    "Hysteria2 UDP outbound requires Cargo feature `hysteria2`",
                )
                .into(),
                upstream: None,
            }),
            #[allow(unused_variables)]
            ResolvedLeafOutbound::Shadowsocks {
                tag,
                server,
                port,
                password,
                cipher,
                ..
            } => {
                #[cfg(feature = "shadowsocks")]
                {
                    let sent = crate::outbound::shadowsocks::send_ss_udp_packet(
                        server,
                        port,
                        password,
                        cipher,
                        &context.session.target,
                        context.session.port,
                        context.payload,
                    )
                    .await
                    .map_err(|error| UdpCandidateFailure {
                        stage: "udp_ss_encrypt_send",
                        error,
                        upstream: Some((server.to_owned(), port)),
                    })?;

                    Ok(UdpCandidateStart::Flow {
                        outbound: UdpFlowOutbound::Shadowsocks {
                            tag: tag.to_owned(),
                            server: server.to_owned(),
                            port,
                            password: password.to_owned(),
                            cipher: cipher.to_owned(),
                        },
                        outbound_tx_bytes: sent as u64,
                    })
                }
                #[cfg(not(feature = "shadowsocks"))]
                {
                    Err(UdpCandidateFailure {
                        stage: "udp_shadowsocks_outbound",
                        error: zero_core::Error::Unsupported(
                            "Shadowsocks UDP outbound requires Cargo feature `shadowsocks`",
                        )
                        .into(),
                        upstream: None,
                    })
                }
            }
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
            } => {
                let sent = crate::outbound::trojan::send_trojan_udp_packet(
                    self,
                    context.session,
                    server,
                    port,
                    password,
                    sni,
                    insecure,
                    &context.session.target,
                    context.session.port,
                    context.payload,
                )
                .await
                .map_err(|error| UdpCandidateFailure {
                    stage: "udp_trojan_send",
                    error,
                    upstream: Some((server.to_owned(), port)),
                })?;

                Ok(UdpCandidateStart::Flow {
                    outbound: UdpFlowOutbound::Trojan {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        sni: sni.map(|s| s.to_owned()),
                        insecure,
                    },
                    outbound_tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "trojan"))]
            ResolvedLeafOutbound::Trojan { .. }
            | ResolvedLeafOutbound::Vmess { .. }
            | ResolvedLeafOutbound::Mieru { .. } => Err(UdpCandidateFailure {
                stage: "trojan/vmess/mieru",
                error: zero_core::Error::Unsupported("UDP not supported").into(),
                upstream: None,
            }),
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Vmess { .. }
            | ResolvedLeafOutbound::Mieru { .. } => Err(UdpCandidateFailure {
                stage: "vmess/mieru",
                error: zero_core::Error::Unsupported("vmess/mieru UDP not supported").into(),
                upstream: None,
            }),
        }
    }

    pub(super) async fn send_socks5_udp_packet(
        &self,
        context: Socks5UdpPacketContext<'_>,
    ) -> Result<usize, UdpCandidateFailure> {
        let association = Socks5UdpAssociation {
            tag: context.tag.to_owned(),
            server: context.server.to_owned(),
            port: context.port,
            auth: context.auth.map(|(u, p)| (u.to_owned(), p.to_owned())),
        };

        send_socks5_udp_packet(
            self,
            context.inbound_tag,
            &association,
            context.session,
            context.payload,
            context.upstream_association,
            context.upstream_idle_deadline,
        )
        .await
        .map_err(|error| UdpCandidateFailure {
            stage: "udp_upstream_send",
            error,
            upstream: Some((context.server.to_owned(), context.port)),
        })
    }
}
