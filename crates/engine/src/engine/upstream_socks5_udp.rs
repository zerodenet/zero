#[cfg(feature = "outbound-socks5")]
mod enabled {
    use std::net::{IpAddr, SocketAddr};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use zero_core::{Address, Error as CoreError};
    use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};

    use super::super::error::EngineError;
    use super::super::metered::MeteredStream;
    use super::super::runtime::Engine;
    use super::super::stats::EngineStats;

    pub(crate) struct ActiveUpstreamSocks5UdpAssociation {
        tag: String,
        server: String,
        port: u16,
        stats: Arc<EngineStats>,
        close_recorded: AtomicBool,
        _control: TokioSocket,
        relay: TokioDatagramSocket,
        relay_addr: SocketAddr,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum UpstreamAssociationCloseReason {
        Closed,
        IdleTimeout,
        Dropped,
    }

    impl ActiveUpstreamSocks5UdpAssociation {
        pub(crate) async fn establish(
            engine: &Engine,
            tag: &str,
            server: &str,
            port: u16,
            auth: Option<(&str, &str)>,
            session_id: u64,
        ) -> Result<Self, EngineError> {
            let control = engine
                .protocols
                .direct_outbound
                .connect_host(server, port, &engine.resolver)
                .await?;
            let mut control = MeteredStream::new(control);
            let (relay_address, relay_port) = engine
                .protocols
                .socks5_outbound
                .establish_udp_association_with_auth(
                    &mut control,
                    auth.map(
                        |(username, password)| zero_protocol_socks5::Socks5OutboundAuth {
                            username,
                            password,
                        },
                    ),
                )
                .await?;
            engine.record_session_outbound_traffic(session_id, control.drain_traffic());
            let control = control.into_inner();
            let relay_addr = engine
                .protocols
                .direct_outbound
                .resolve_address(
                    &relay_address,
                    relay_port,
                    &engine.resolver,
                    "failed to resolve upstream socks5 udp relay",
                )
                .await?;

            let bind_addr = match relay_addr {
                SocketAddr::V4(_) => {
                    SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0)
                }
                SocketAddr::V6(_) => {
                    SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), 0)
                }
            };
            let relay = TokioDatagramSocket::bind_addr(bind_addr).await?;

            Ok(Self {
                tag: tag.to_owned(),
                server: server.to_owned(),
                port,
                stats: Arc::clone(&engine.stats),
                close_recorded: AtomicBool::new(false),
                _control: control,
                relay,
                relay_addr,
            })
        }

        pub(crate) fn matches(&self, tag: &str, server: &str, port: u16) -> bool {
            self.tag == tag && self.server == server && self.port == port
        }

        pub(crate) fn outbound_tag(&self) -> &str {
            &self.tag
        }

        pub(crate) fn upstream_endpoint(&self) -> (&str, u16) {
            (&self.server, self.port)
        }

        pub(crate) fn close(self, reason: UpstreamAssociationCloseReason) {
            self.close_recorded.store(true, Ordering::Relaxed);

            match reason {
                UpstreamAssociationCloseReason::Closed => {
                    self.stats.record_udp_upstream_association_closed();
                }
                UpstreamAssociationCloseReason::IdleTimeout => {
                    self.stats.record_udp_upstream_association_idle_timeout();
                }
                UpstreamAssociationCloseReason::Dropped => {
                    self.stats.record_udp_upstream_association_dropped();
                }
            }
        }

        pub(crate) async fn send_packet(
            &self,
            target: &Address,
            port: u16,
            payload: &[u8],
        ) -> Result<usize, EngineError> {
            let packet = zero_protocol_socks5::build_udp_packet(target, port, payload)?;
            self.relay
                .send_to_addr(&packet, self.relay_addr)
                .await
                .map_err(EngineError::from)
        }

        pub(crate) async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
            let (read, sender) = self.relay.recv_from_addr(buf).await?;
            if sender != self.relay_addr {
                return Err(
                    CoreError::Protocol("unexpected UDP sender from SOCKS5 upstream").into(),
                );
            }

            Ok(read)
        }
    }

    impl Drop for ActiveUpstreamSocks5UdpAssociation {
        fn drop(&mut self) {
            if !self.close_recorded.load(Ordering::Relaxed) {
                self.stats.record_udp_upstream_association_closed();
                self.close_recorded.store(true, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(not(feature = "outbound-socks5"))]
mod disabled {
    use zero_core::Address;

    use super::super::error::EngineError;
    use super::super::runtime::Engine;

    pub(crate) struct ActiveUpstreamSocks5UdpAssociation;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum UpstreamAssociationCloseReason {
        Closed,
        IdleTimeout,
        Dropped,
    }

    impl ActiveUpstreamSocks5UdpAssociation {
        pub(crate) async fn establish(
            _engine: &Engine,
            tag: &str,
            _server: &str,
            _port: u16,
            _auth: Option<(&str, &str)>,
            _session_id: u64,
        ) -> Result<Self, EngineError> {
            Err(EngineError::CompiledFeatureDisabled {
                kind: "outbound",
                tag: tag.to_owned(),
                protocol: "socks5",
                feature: "outbound-socks5",
            })
        }

        pub(crate) fn matches(&self, _tag: &str, _server: &str, _port: u16) -> bool {
            false
        }

        pub(crate) fn outbound_tag(&self) -> &str {
            "-"
        }

        pub(crate) fn upstream_endpoint(&self) -> (&str, u16) {
            ("-", 0)
        }

        pub(crate) fn close(self, _reason: UpstreamAssociationCloseReason) {}

        pub(crate) async fn send_packet(
            &self,
            _target: &Address,
            _port: u16,
            _payload: &[u8],
        ) -> Result<usize, EngineError> {
            Err(EngineError::CompiledFeatureDisabled {
                kind: "outbound",
                tag: "socks5-upstream".to_owned(),
                protocol: "socks5",
                feature: "outbound-socks5",
            })
        }

        pub(crate) async fn recv_packet(&self, _buf: &mut [u8]) -> Result<usize, EngineError> {
            Err(EngineError::CompiledFeatureDisabled {
                kind: "outbound",
                tag: "socks5-upstream".to_owned(),
                protocol: "socks5",
                feature: "outbound-socks5",
            })
        }
    }
}

#[cfg(not(feature = "outbound-socks5"))]
pub(crate) use disabled::{ActiveUpstreamSocks5UdpAssociation, UpstreamAssociationCloseReason};
#[cfg(feature = "outbound-socks5")]
pub(crate) use enabled::{ActiveUpstreamSocks5UdpAssociation, UpstreamAssociationCloseReason};
