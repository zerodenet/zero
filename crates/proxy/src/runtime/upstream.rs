use zero_config::{ClientTlsConfig, RealityConfig};
use zero_core::Session;
use zero_engine::EngineError;
#[cfg(feature = "outbound-vless")]
use zero_protocol_vless::RealityClientOptions;

use crate::runtime::Proxy;
#[cfg(feature = "outbound-socks5")]
use crate::transport::MeteredStream;
use crate::transport::TcpRelayStream;

pub(crate) struct VlessUpstream<'a> {
    pub server: &'a str,
    pub port: u16,
    pub id: &'a str,
    pub tls: Option<&'a ClientTlsConfig>,
    pub reality: Option<&'a RealityConfig>,
    pub ws: Option<&'a zero_config::WebSocketConfig>,
}

impl Proxy {
    #[cfg(feature = "outbound-socks5")]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        session: &Session,
        server: &str,
        port: u16,
        auth: Option<(&str, &str)>,
    ) -> Result<TcpRelayStream, EngineError> {
        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(server, port, &self.resolver)
            .await?;
        let mut upstream = MeteredStream::new(upstream);

        self.protocols
            .socks5_outbound
            .establish_tunnel_with_auth(
                &mut upstream,
                session,
                auth.map(
                    |(username, password)| zero_protocol_socks5::Socks5OutboundAuth {
                        username,
                        password,
                    },
                ),
            )
            .await?;
        self.record_session_outbound_traffic(session.id, upstream.drain_traffic());

        Ok(upstream.into_inner().into())
    }

    #[cfg(not(feature = "outbound-socks5"))]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
        _auth: Option<(&str, &str)>,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "socks5-upstream".to_owned(),
            protocol: "socks5",
            feature: "outbound-socks5",
        })
    }

    #[cfg(feature = "outbound-vless")]
    pub(crate) async fn connect_via_vless_upstream(
        &self,
        session: &Session,
        upstream: VlessUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let id = zero_protocol_vless::parse_uuid(upstream.id)?;
        let socket = self
            .protocols
            .direct_outbound
            .connect_host(upstream.server, upstream.port, &self.resolver)
            .await?;

        let stream = match (upstream.tls, upstream.reality, upstream.ws) {
            (Some(tls), None, Some(ws)) => {
                let tls_stream = crate::transport::connect_tls_upstream(
                    socket,
                    tls,
                    self.config.source_dir(),
                    upstream.server,
                )
                .await?;
                let ws_stream =
                    crate::transport::connect_ws(tls_stream, ws, upstream.server, upstream.port)
                        .await?;
                TcpRelayStream::new(ws_stream)
            }
            (Some(tls), None, None) => {
                let tls_stream = crate::transport::connect_tls_upstream(
                    socket,
                    tls,
                    self.config.source_dir(),
                    upstream.server,
                )
                .await?;
                TcpRelayStream::new(tls_stream)
            }
            (None, Some(reality), None) => {
                let server_name = reality.server_name.as_deref().unwrap_or(upstream.server);
                let reality_stream = zero_protocol_vless::upgrade_reality_client(
                    socket,
                    RealityClientOptions {
                        public_key: &reality.public_key,
                        short_id: &reality.short_id,
                        server_name,
                        cipher_suites: &reality.cipher_suites,
                    },
                )
                .await?;
                TcpRelayStream::new(reality_stream)
            }
            (None, None, Some(ws)) => {
                let ws_stream =
                    crate::transport::connect_ws(socket, ws, upstream.server, upstream.port)
                        .await?;
                TcpRelayStream::new(ws_stream)
            }
            (None, None, None) => socket.into(),
            (Some(_), Some(_), _) | (None, Some(_), Some(_)) => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid vless outbound transport combination",
                )));
            }
        };

        let is_reality = upstream.reality.is_some();
        let mut upstream = crate::transport::MeteredStream::new(stream);

        if is_reality {
            self.protocols
                .vless_outbound
                .send_tcp_request(&mut upstream, session, &id)
                .await?;
            self.record_session_outbound_traffic(session.id, upstream.drain_traffic());

            Ok(TcpRelayStream::new(
                zero_protocol_vless::DeferredVlessResponseStream::new(upstream.into_inner()),
            ))
        } else {
            self.protocols
                .vless_outbound
                .establish_tcp_tunnel(&mut upstream, session, &id)
                .await?;
            self.record_session_outbound_traffic(session.id, upstream.drain_traffic());

            Ok(upstream.into_inner())
        }
    }

    #[cfg(not(feature = "outbound-vless"))]
    pub(crate) async fn connect_via_vless_upstream(
        &self,
        _session: &Session,
        upstream: VlessUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let _ = (
            upstream.server,
            upstream.port,
            upstream.id,
            upstream.tls,
            upstream.reality,
            upstream.ws,
        );

        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "vless-upstream".to_owned(),
            protocol: "vless",
            feature: "outbound-vless",
        })
    }
}
