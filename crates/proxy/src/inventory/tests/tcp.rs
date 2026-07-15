use std::sync::Arc;

use zero_config::RuntimeConfig;
use zero_core::{Address, Network, ProtocolType, Session};

use super::fixtures::{FakeTcpCapability, TcpCapabilityCalls};
use crate::inventory::ProtocolInventory;
use crate::protocol_registry::{
    fake_direct_leaf, OutboundAdapterContext, ProtocolRegistry, TcpRuntimeServices,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(super) fn proxy_with_fake_tcp(calls: Arc<TcpCapabilityCalls>) -> Proxy {
    let config = RuntimeConfig::parse(
        r#"{
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("minimal runtime config");
    let mut proxy = Proxy::new(config).expect("minimal proxy");
    let mut registry = ProtocolRegistry::default();
    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    registry.register_managed_capability(Arc::new(FakeTcpCapability::new(calls.clone())));
    #[cfg(not(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    )))]
    registry.register_capability(Arc::new(FakeTcpCapability::new(calls)));
    proxy.protocols = ProtocolInventory { registry };
    proxy
}

pub(super) fn session() -> Session {
    Session::new(
        7,
        Address::Domain("example.test".to_owned()),
        443,
        Network::Tcp,
        ProtocolType::Unknown,
    )
}

#[tokio::test]
async fn inventory_invokes_fake_tcp_leaf_and_relay_capabilities() {
    let calls = Arc::new(TcpCapabilityCalls::default());
    let proxy = proxy_with_fake_tcp(calls.clone());
    let leaf = fake_direct_leaf();
    let ctx = OutboundAdapterContext::new(proxy.config.source_dir());
    let claimed = proxy
        .protocols
        .claim_outbound_leaf(leaf.clone())
        .expect("fake leaf claim");

    let prepared = match proxy
        .protocols
        .prepare_claimed_tcp_candidate(ctx.clone(), &claimed)
    {
        Ok(prepared) => prepared,
        Err(_) => panic!("fake leaf prepare failed"),
    };
    let established = match prepared
        .execute(TcpRuntimeServices::from_proxy(&proxy), &session())
        .await
    {
        Ok(established) => established,
        Err(_) => panic!("fake leaf connect failed"),
    };
    assert!(established.into_relay_stream().is_err());

    let (stream, _peer) = tokio::io::duplex(64);
    proxy
        .protocols
        .prepare_claimed_tcp_relay_hop(ctx, &claimed)
        .expect("fake relay prepare")
        .execute(
            TcpRuntimeServices::from_proxy(&proxy),
            TcpRelayStream::new(stream),
            &session(),
        )
        .await
        .expect("fake relay hop");

    assert_eq!(calls.connects(), 1);
    assert_eq!(calls.relay_hops(), 1);
}

#[tokio::test]
async fn inventory_preserves_tcp_and_relay_capability_failures() {
    let calls = Arc::new(TcpCapabilityCalls::default());
    calls.set_fail_tcp(true);
    let proxy = proxy_with_fake_tcp(calls.clone());
    let leaf = fake_direct_leaf();
    let ctx = OutboundAdapterContext::new(proxy.config.source_dir());
    let claimed = proxy
        .protocols
        .claim_outbound_leaf(leaf.clone())
        .expect("fake leaf claim");

    let prepared = match proxy
        .protocols
        .prepare_claimed_tcp_candidate(ctx.clone(), &claimed)
    {
        Ok(prepared) => prepared,
        Err(_) => panic!("fake leaf prepare failed"),
    };
    let failure = match prepared
        .execute(TcpRuntimeServices::from_proxy(&proxy), &session())
        .await
    {
        Ok(_) => panic!("fake TCP connect unexpectedly succeeded"),
        Err(failure) => failure,
    };
    assert_eq!(failure.stage, "fake_tcp_connect");
    assert_eq!(
        failure.upstream_endpoint,
        Some(("fake-tcp.test".to_owned(), 8443))
    );
    assert!(failure.error.to_string().contains("fake TCP failure"));

    calls.set_fail_tcp(false);
    calls.set_fail_relay(true);
    let (stream, _peer) = tokio::io::duplex(64);
    let error = match proxy
        .protocols
        .prepare_claimed_tcp_relay_hop(ctx, &claimed)
        .expect("fake relay prepare")
        .execute(
            TcpRuntimeServices::from_proxy(&proxy),
            TcpRelayStream::new(stream),
            &session(),
        )
        .await
    {
        Ok(_) => panic!("fake relay hop unexpectedly succeeded"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("fake relay failure"));
    assert_eq!(calls.connects(), 1);
    assert_eq!(calls.relay_hops(), 1);
}
