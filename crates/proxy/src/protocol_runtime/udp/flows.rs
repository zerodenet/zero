use crate::runtime::Proxy;
use zero_core::Session;

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
