use std::sync::Arc;

use zero_engine::ResolvedLeafOutbound;

use super::ProtocolRegistry;
#[cfg(any(
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::protocol_registry::ManagedUdpHandlerProvider;
#[cfg(feature = "socks5")]
use crate::protocol_registry::UpstreamUdpHandlerProvider;
use crate::protocol_registry::{
    InboundListenerCapability, OutboundLeafClaim, ProtocolSupportCapability, TcpOutboundCapability,
};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::protocol_registry::{UdpFlowCapability, UdpPacketPathCapability};

type OutboundLeafClaimFn<T> =
    for<'a> fn(&T, ResolvedLeafOutbound<'a>) -> Option<OutboundLeafClaim<'a>>;

#[cfg(any(
    not(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    )),
    feature = "http",
    feature = "mixed"
))]
struct NoOutboundClaimer<T> {
    adapter: Arc<T>,
}

#[cfg(any(
    not(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    )),
    feature = "http",
    feature = "mixed"
))]
impl<T> super::OutboundLeafClaimer for NoOutboundClaimer<T>
where
    T: TcpOutboundCapability + Send + Sync + 'static,
{
    fn claim_outbound_leaf<'a>(
        &self,
        _leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafClaim<'a>> {
        let _ = &self.adapter;
        None
    }
}

#[cfg(any(
    not(any(feature = "http", feature = "mixed")),
    feature = "http",
    feature = "mixed"
))]
struct ProjectedOutboundClaimer<T> {
    adapter: Arc<T>,
    claim: OutboundLeafClaimFn<T>,
}

#[cfg(any(
    not(any(feature = "http", feature = "mixed")),
    feature = "http",
    feature = "mixed"
))]
impl<T> super::OutboundLeafClaimer for ProjectedOutboundClaimer<T>
where
    T: Send + Sync + 'static,
{
    fn claim_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafClaim<'a>> {
        (self.claim)(self.adapter.as_ref(), leaf)
    }
}

impl ProtocolRegistry {
    #[cfg(any(
        not(any(
            feature = "socks5",
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        )),
        feature = "http",
        feature = "mixed"
    ))]
    pub(crate) fn register_core_capability<T>(
        &mut self,
        adapter: Arc<T>,
        claim: Option<OutboundLeafClaimFn<T>>,
    ) where
        T: ProtocolSupportCapability + InboundListenerCapability + TcpOutboundCapability + 'static,
    {
        self.entries.push(super::RegisteredProtocolEntry {
            support: adapter.clone(),
            inbound: adapter.clone(),
            outbound: match claim {
                Some(claim) => Arc::new(ProjectedOutboundClaimer {
                    adapter: adapter.clone(),
                    claim,
                }),
                None => Arc::new(NoOutboundClaimer {
                    adapter: adapter.clone(),
                }),
            },
            #[cfg(test)]
            tcp: adapter,
            #[cfg(all(
                test,
                any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                )
            ))]
            udp: None,
            #[cfg(any(
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            managed_udp_handlers: None,
            #[cfg(feature = "socks5")]
            upstream_udp_handler: None,
            #[cfg(all(
                test,
                any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                )
            ))]
            packet_path: None,
        });
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
    pub(crate) fn register_capability<T>(&mut self, adapter: Arc<T>, claim: OutboundLeafClaimFn<T>)
    where
        T: ProtocolSupportCapability
            + InboundListenerCapability
            + TcpOutboundCapability
            + UdpFlowCapability
            + UdpPacketPathCapability
            + 'static,
    {
        self.entries.push(super::RegisteredProtocolEntry {
            support: adapter.clone(),
            inbound: adapter.clone(),
            outbound: Arc::new(ProjectedOutboundClaimer {
                adapter: adapter.clone(),
                claim,
            }),
            #[cfg(test)]
            tcp: adapter.clone(),
            #[cfg(all(
                test,
                any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                )
            ))]
            udp: Some(adapter.clone()),
            #[cfg(any(
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            managed_udp_handlers: None,
            #[cfg(feature = "socks5")]
            upstream_udp_handler: None,
            #[cfg(all(
                test,
                any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                )
            ))]
            packet_path: Some(adapter),
        });
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn register_upstream_capability<T>(
        &mut self,
        adapter: Arc<T>,
        claim: OutboundLeafClaimFn<T>,
    ) where
        T: ProtocolSupportCapability
            + InboundListenerCapability
            + TcpOutboundCapability
            + UdpFlowCapability
            + UdpPacketPathCapability
            + UpstreamUdpHandlerProvider
            + 'static,
    {
        self.entries.push(super::RegisteredProtocolEntry {
            support: adapter.clone(),
            inbound: adapter.clone(),
            outbound: Arc::new(ProjectedOutboundClaimer {
                adapter: adapter.clone(),
                claim,
            }),
            #[cfg(test)]
            tcp: adapter.clone(),
            #[cfg(all(
                test,
                any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                )
            ))]
            udp: Some(adapter.clone()),
            #[cfg(any(
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            managed_udp_handlers: None,
            upstream_udp_handler: Some(adapter.clone()),
            #[cfg(all(
                test,
                any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                )
            ))]
            packet_path: Some(adapter),
        });
    }

    #[cfg(any(
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn register_managed_capability<T>(
        &mut self,
        adapter: Arc<T>,
        claim: OutboundLeafClaimFn<T>,
    ) where
        T: ProtocolSupportCapability
            + InboundListenerCapability
            + TcpOutboundCapability
            + UdpFlowCapability
            + UdpPacketPathCapability
            + ManagedUdpHandlerProvider
            + 'static,
    {
        self.entries.push(super::RegisteredProtocolEntry {
            support: adapter.clone(),
            inbound: adapter.clone(),
            outbound: Arc::new(ProjectedOutboundClaimer {
                adapter: adapter.clone(),
                claim,
            }),
            #[cfg(test)]
            tcp: adapter.clone(),
            #[cfg(all(
                test,
                any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                )
            ))]
            udp: Some(adapter.clone()),
            managed_udp_handlers: Some(adapter.clone()),
            #[cfg(feature = "socks5")]
            upstream_udp_handler: None,
            #[cfg(all(
                test,
                any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                )
            ))]
            packet_path: Some(adapter),
        });
    }

    #[cfg(any(
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn managed_udp_handler_providers(
        &self,
    ) -> impl Iterator<Item = &Arc<dyn ManagedUdpHandlerProvider>> {
        self.entries
            .iter()
            .filter_map(|entry| entry.managed_udp_handlers.as_ref())
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn upstream_udp_handler_providers(
        &self,
    ) -> impl Iterator<Item = &Arc<dyn UpstreamUdpHandlerProvider>> {
        self.entries
            .iter()
            .filter_map(|entry| entry.upstream_udp_handler.as_ref())
    }
}
