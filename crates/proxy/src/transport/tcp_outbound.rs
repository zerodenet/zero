use zero_core::Session;

use super::super::runtime::upstream::VlessUpstream;
use super::super::runtime::Proxy;
use super::stream::TcpRelayStream;
use zero_engine::EngineError;
use zero_engine::{ResolvedLeafOutbound, ResolvedOutbound};

pub(crate) enum EstablishedTcpOutbound {
    Direct {
        tag: String,
        upstream: TcpRelayStream,
    },
    Block {
        tag: String,
    },
    Socks5 {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
    Vless {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
}

pub(crate) struct TcpOutboundFailure {
    pub stage: &'static str,
    pub error: EngineError,
    pub upstream_endpoint: Option<(String, u16)>,
}

impl Proxy {
    pub(crate) async fn establish_tcp_outbound(
        &self,
        session: &Session,
        resolved: ResolvedOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match resolved {
            ResolvedOutbound::Single(candidate) => {
                self.establish_tcp_candidate(session, candidate).await
            }
            ResolvedOutbound::Fallback { candidates } => {
                let mut last_failure = None;

                for candidate in candidates {
                    match self.establish_tcp_candidate(session, candidate).await {
                        Ok(outbound) => return Ok(outbound),
                        Err(failure) => last_failure = Some(failure),
                    }
                }

                Err(last_failure
                    .expect("validated fallback groups always have at least one candidate"))
            }
        }
    }

    async fn establish_tcp_candidate(
        &self,
        session: &Session,
        candidate: ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                match self
                    .protocols
                    .direct_outbound
                    .connect(session, &self.resolver)
                    .await
                {
                    Ok(upstream) => Ok(EstablishedTcpOutbound::Direct {
                        tag: tag.unwrap_or("direct").to_owned(),
                        upstream: upstream.into(),
                    }),
                    Err(error) => Err(TcpOutboundFailure {
                        stage: "connect_direct",
                        error: error.into(),
                        upstream_endpoint: None,
                    }),
                }
            }
            ResolvedLeafOutbound::Block { tag } => Ok(EstablishedTcpOutbound::Block {
                tag: tag.unwrap_or("block").to_owned(),
            }),
            ResolvedLeafOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => {
                match self
                    .connect_via_socks5_upstream(session, server, port, username.zip(password))
                    .await
                {
                    Ok(upstream) => Ok(EstablishedTcpOutbound::Socks5 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        upstream,
                    }),
                    Err(error) => Err(TcpOutboundFailure {
                        stage: "connect_upstream_socks5",
                        error,
                        upstream_endpoint: Some((server.to_owned(), port)),
                    }),
                }
            }
            ResolvedLeafOutbound::Vless {
                tag,
                server,
                port,
                id,
                flow,
                mux_concurrency,
                mux_idle_timeout_secs,
                tls,
                reality,
                ws,
            } => {
                match self
                    .connect_via_vless_upstream(
                        session,
                        VlessUpstream {
                            server,
                            port,
                            id,
                            flow,
                            mux_concurrency,
                            mux_idle_timeout_secs,
                            tls,
                            reality,
                            ws,
                        },
                    )
                    .await
                {
                    Ok(upstream) => Ok(EstablishedTcpOutbound::Vless {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        upstream,
                    }),
                    Err(error) => Err(TcpOutboundFailure {
                        stage: "connect_upstream_vless",
                        error,
                        upstream_endpoint: Some((server.to_owned(), port)),
                    }),
                }
            }
        }
    }
}
