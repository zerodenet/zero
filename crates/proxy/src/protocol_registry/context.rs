use crate::runtime::Proxy;

#[derive(Clone)]
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct UdpRuntimeServices {
    proxy: Proxy,
}

#[derive(Clone, Copy)]
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) enum UdpAssociationCloseKind {
    Closed,
    IdleTimeout,
    Dropped,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl UdpRuntimeServices {
    pub(crate) async fn connect_upstream(
        &self,
        server: &str,
        port: u16,
    ) -> Result<zero_platform_tokio::TokioSocket, zero_engine::EngineError> {
        self.proxy
            .connect_upstream_host_owned(server.to_owned(), port)
            .await
    }

    pub(crate) async fn resolve_udp_peer(
        &self,
        address: &zero_core::Address,
        port: u16,
        context: &'static str,
    ) -> Result<
        (
            zero_traits::SocketAddress,
            zero_platform_tokio::TokioDatagramSocket,
        ),
        zero_engine::EngineError,
    > {
        let (peer, socket) = crate::runtime::udp_socket::resolve_udp_peer_endpoint(
            &self.proxy,
            address,
            port,
            context,
        )
        .await?;
        Ok((
            zero_platform_tokio::socket_addr_to_socket_address(peer),
            socket,
        ))
    }

    pub(crate) fn record_control_traffic(
        &self,
        session_id: u64,
        traffic: zero_transport::StreamTraffic,
    ) {
        self.proxy
            .record_session_outbound_traffic(session_id, traffic);
    }

    pub(crate) fn record_association_close(&self, kind: UdpAssociationCloseKind) {
        match kind {
            UdpAssociationCloseKind::Closed => self.proxy.record_udp_upstream_association_closed(),
            UdpAssociationCloseKind::IdleTimeout => {
                self.proxy.record_udp_upstream_association_idle_timeout()
            }
            UdpAssociationCloseKind::Dropped => {
                self.proxy.record_udp_upstream_association_dropped()
            }
        }
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) async fn build_udp_socket_carrier(
        &self,
        server: &str,
        port: u16,
        codec: std::sync::Arc<
            dyn zero_traits::DatagramCodec<zero_core::Address, Error = zero_core::Error>,
        >,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        zero_engine::EngineError,
    > {
        crate::runtime::udp_flow::packet_path_chain::carriers::udp_socket_carrier::build(
            &self.proxy,
            server,
            port,
            codec,
        )
        .await
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OutboundAdapterContext<'a> {
    proxy: &'a Proxy,
}

impl<'a> OutboundAdapterContext<'a> {
    pub(crate) fn new(proxy: &'a Proxy) -> Self {
        Self { proxy }
    }

    pub(crate) fn proxy(&self) -> &'a Proxy {
        self.proxy
    }
}

#[derive(Clone, Copy)]
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct UdpAdapterContext<'a> {
    proxy: &'a Proxy,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl<'a> UdpAdapterContext<'a> {
    pub(crate) fn new(proxy: &'a Proxy) -> Self {
        Self { proxy }
    }

    pub(crate) fn proxy(&self) -> &'a Proxy {
        self.proxy
    }

    pub(crate) fn runtime_services(&self) -> UdpRuntimeServices {
        UdpRuntimeServices {
            proxy: self.proxy.clone(),
        }
    }
}
