use zero_core::Session;
use zero_platform_tokio::TokioSocket;

use super::error::EngineError;
use super::resolve::{ResolvedLeafOutbound, ResolvedOutbound};
use super::runtime::Engine;

pub(crate) enum EstablishedTcpOutbound {
    Direct {
        tag: String,
        upstream: TokioSocket,
    },
    Block {
        tag: String,
    },
    Socks5 {
        tag: String,
        server: String,
        port: u16,
        upstream: TokioSocket,
    },
}

pub(crate) struct TcpOutboundFailure {
    pub stage: &'static str,
    pub error: EngineError,
    pub upstream_endpoint: Option<(String, u16)>,
}

impl Engine {
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
                        upstream,
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
        }
    }
}
