use super::ProtocolInventory;
use crate::transport::DirectConnector;

#[cfg(feature = "http_connect")]
use http_connect::HttpConnectInbound;
#[cfg(feature = "shadowsocks")]
use shadowsocks::ShadowsocksOutbound;
#[cfg(feature = "socks5")]
use socks5::Socks5Inbound;
#[cfg(feature = "socks5")]
use socks5::Socks5Outbound;
#[cfg(feature = "trojan")]
use trojan::TrojanOutbound;
#[cfg(feature = "vless")]
use vless::VlessInbound;
#[cfg(feature = "vless")]
use vless::VlessOutbound;
#[cfg(feature = "vmess")]
use vmess::VmessOutbound;

impl ProtocolInventory {
    pub(crate) fn direct_connector(&self) -> DirectConnector {
        DirectConnector
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn socks5_inbound_protocol(&self) -> Socks5Inbound {
        Socks5Inbound
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn socks5_outbound_protocol(&self) -> Socks5Outbound {
        Socks5Outbound
    }

    #[cfg(feature = "http_connect")]
    pub(crate) fn http_connect_inbound_protocol(&self) -> HttpConnectInbound {
        HttpConnectInbound
    }

    #[cfg(feature = "vless")]
    pub(crate) fn vless_inbound_protocol(&self) -> VlessInbound {
        VlessInbound
    }

    #[cfg(feature = "vless")]
    pub(crate) fn vless_outbound_protocol(&self) -> VlessOutbound {
        VlessOutbound
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn shadowsocks_outbound_protocol(&self) -> ShadowsocksOutbound {
        ShadowsocksOutbound
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan_outbound_protocol(&self) -> TrojanOutbound {
        TrojanOutbound
    }

    #[cfg(feature = "vmess")]
    pub(crate) fn vmess_outbound_protocol(&self) -> VmessOutbound {
        VmessOutbound
    }
}
