use std::sync::Arc;

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
use super::fixtures::FakeProviderResume;
#[cfg(feature = "socks5")]
use super::fixtures::FakeUpstreamResume;
use super::fixtures::{FakeTcpCapability, TcpCapabilityCalls};
use super::tcp::{proxy_with_fake_tcp, session};
use crate::protocol_registry::{fake_direct_leaf, UdpAdapterContext, UdpRuntimeServices};
use crate::runtime::udp_dispatch::FlowStartResult;
use crate::transport::{RelayCarrier, TcpRelayStream};

#[tokio::test]
async fn inventory_invokes_fake_udp_leaf_capability() {
    let calls = Arc::new(TcpCapabilityCalls::default());
    let proxy = proxy_with_fake_tcp(calls.clone());
    let mut dispatch = crate::runtime::udp_ingress::UdpIngressRuntime::from_proxy(&proxy)
        .new_dispatch("fake-inbound")
        .await
        .expect("UDP dispatch");
    let ctx = UdpAdapterContext::new(
        proxy.config.source_dir(),
        UdpRuntimeServices::from_proxy(&proxy),
    );
    let payload = b"capability payload";
    let leaf = fake_direct_leaf();
    let claimed = proxy
        .protocols
        .claim_outbound_leaf(leaf.clone())
        .expect("fake UDP claim");

    let prepared = match proxy
        .protocols
        .prepare_claimed_udp_leaf_candidate(ctx.clone(), &claimed)
    {
        Ok(prepared) => prepared,
        Err(_) => panic!("fake UDP prepare failed"),
    };

    let result = match prepared
        .execute(&mut dispatch, ctx, &session(), payload)
        .await
    {
        Ok(result) => result,
        Err(_) => panic!("fake UDP start failed"),
    };

    match result {
        FlowStartResult::Blocked { tag } => assert_eq!(tag, "fake-udp"),
        FlowStartResult::Flow { .. } => panic!("unexpected fake UDP flow"),
    }
    assert_eq!(calls.udp_starts(), 1);
    assert_eq!(calls.udp_payload_bytes(), payload.len());
}

#[tokio::test]
async fn inventory_preserves_fake_udp_failure_metadata() {
    let calls = Arc::new(TcpCapabilityCalls::default());
    calls.set_fail_udp(true);
    let proxy = proxy_with_fake_tcp(calls);
    let mut dispatch = crate::runtime::udp_ingress::UdpIngressRuntime::from_proxy(&proxy)
        .new_dispatch("fake-inbound")
        .await
        .expect("UDP dispatch");
    let ctx = UdpAdapterContext::new(
        proxy.config.source_dir(),
        UdpRuntimeServices::from_proxy(&proxy),
    );
    let leaf = fake_direct_leaf();
    let claimed = proxy
        .protocols
        .claim_outbound_leaf(leaf.clone())
        .expect("fake UDP claim");

    let prepared = match proxy
        .protocols
        .prepare_claimed_udp_leaf_candidate(ctx.clone(), &claimed)
    {
        Ok(prepared) => prepared,
        Err(_) => panic!("fake UDP prepare failed"),
    };

    let failure = match prepared
        .execute(&mut dispatch, ctx, &session(), b"failure")
        .await
    {
        Ok(_) => panic!("fake UDP start unexpectedly succeeded"),
        Err(failure) => failure,
    };

    assert_eq!(failure.stage, "fake_udp_start");
    assert_eq!(
        failure.upstream,
        Some(("fake-upstream.test".to_owned(), 5353))
    );
    assert!(failure.error.to_string().contains("fake udp failure"));
}

#[tokio::test]
async fn inventory_invokes_fake_udp_relay_capabilities() {
    let calls = Arc::new(TcpCapabilityCalls::default());
    let proxy = proxy_with_fake_tcp(calls.clone());
    let mut dispatch = crate::runtime::udp_ingress::UdpIngressRuntime::from_proxy(&proxy)
        .new_dispatch("fake-inbound")
        .await
        .expect("UDP dispatch");
    let ctx = UdpAdapterContext::new(
        proxy.config.source_dir(),
        UdpRuntimeServices::from_proxy(&proxy),
    );
    let leaf = fake_direct_leaf();
    let claimed = proxy
        .protocols
        .claim_outbound_leaf(leaf.clone())
        .expect("fake UDP claim");

    assert!(claimed.udp_relay_needs_two_streams(ctx.source_dir()));

    let two_stream_payload = b"two-stream capability";
    let (post_stream, _post_peer) = tokio::io::duplex(64);
    let (get_stream, _get_peer) = tokio::io::duplex(64);
    let two_stream_operation = match claimed
        .clone()
        .into_claimed()
        .prepare_owned_udp_relay_two_stream(
            RelayCarrier {
                stream: TcpRelayStream::new(post_stream),
                server: "fake-relay-post.test".to_owned(),
                port: 9443,
            },
            RelayCarrier {
                stream: TcpRelayStream::new(get_stream),
                server: "fake-relay-get.test".to_owned(),
                port: 9444,
            },
            ctx.source_dir(),
        ) {
        Ok(operation) => operation,
        Err(_) => panic!("two-stream relay prepare failed"),
    };

    let two_stream = match two_stream_operation
        .execute(&mut dispatch, ctx.clone(), &session(), two_stream_payload)
        .await
    {
        Ok(result) => result,
        Err(_) => panic!("two-stream relay capability failed"),
    };
    assert!(matches!(
        two_stream,
        FlowStartResult::Blocked { tag } if tag == "fake-two-stream"
    ));

    let (stream, _peer) = tokio::io::duplex(64);
    let final_payload = b"final-hop capability";
    let final_hop_operation = match claimed.into_claimed().prepare_owned_udp_relay_final_hop(
        RelayCarrier {
            stream: TcpRelayStream::new(stream),
            server: "relay-carrier.test".to_owned(),
            port: 9443,
        },
        ctx.source_dir(),
    ) {
        Ok(operation) => operation,
        Err(_) => panic!("final-hop relay prepare failed"),
    };

    let final_hop = match final_hop_operation
        .execute(&mut dispatch, ctx, &session(), final_payload)
        .await
    {
        Ok(result) => result,
        Err(_) => panic!("final-hop relay capability failed"),
    };
    assert!(matches!(
        final_hop,
        FlowStartResult::Blocked { tag } if tag == "fake-final-hop"
    ));

    assert_eq!(calls.udp_two_stream_starts(), 1);
    assert_eq!(calls.udp_final_hop_starts(), 1);
    assert_eq!(calls.udp_final_hop_port(), 9443);
    assert_eq!(
        calls.udp_payload_bytes(),
        two_stream_payload.len() + final_payload.len()
    );
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
#[tokio::test]
async fn inventory_executes_handler_produced_by_registered_provider() {
    use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
    use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
    use crate::runtime::udp_flow::registered::RegisteredUdpState;
    use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

    let calls = Arc::new(TcpCapabilityCalls::default());
    let proxy = proxy_with_fake_tcp(calls.clone());
    let mut state = RegisteredUdpState::new(proxy.protocols.registered_udp_handlers());
    let managed = state.register_managed_flow(ManagedUdpFlowResume::new(FakeProviderResume {
        generation: calls.provider_generation(),
    }));
    let flow = UdpFlowSnapshot {
        session: zero_core::Session::new(
            41,
            zero_core::Address::Domain("provider-target.test".to_owned()),
            53,
            zero_core::Network::Udp,
            zero_core::ProtocolType::Unknown,
        ),
        outbound: UdpFlowOutbound::Datagram {
            tag: "fake-provider".to_owned(),
            server: "provider-upstream.test".to_owned(),
            port: 5353,
            managed,
        },
        client_session_id: None,
    };
    let mut chain_tasks = tokio::task::JoinSet::new();
    let payload = b"provider handler payload";

    let sent = match state
        .forward_existing_managed_flow(
            &mut chain_tasks,
            crate::protocol_registry::UdpRuntimeServices::from_proxy(&proxy),
            (&flow, payload),
        )
        .await
    {
        Ok(sent) => sent,
        Err(_) => panic!("provider-produced handler did not execute"),
    };

    assert_eq!(sent, payload.len());
    assert_eq!(calls.provider_forwards(), 1);
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
#[tokio::test]
async fn reload_invalidates_provider_resumes_before_new_generation_flows() {
    use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
    use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
    use crate::runtime::udp_flow::registered::RegisteredUdpState;
    use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

    let calls = Arc::new(TcpCapabilityCalls::default());
    let proxy = proxy_with_fake_tcp(calls.clone());
    let mut state = RegisteredUdpState::new(proxy.protocols.registered_udp_handlers());
    let old_ref = state.register_managed_flow(ManagedUdpFlowResume::new(FakeProviderResume {
        generation: calls.provider_generation(),
    }));
    let old_flow = UdpFlowSnapshot {
        session: zero_core::Session::new(
            51,
            zero_core::Address::Domain("reload-target.test".to_owned()),
            53,
            zero_core::Network::Udp,
            zero_core::ProtocolType::Unknown,
        ),
        outbound: UdpFlowOutbound::Datagram {
            tag: "old-generation".to_owned(),
            server: "reload-upstream.test".to_owned(),
            port: 5353,
            managed: old_ref,
        },
        client_session_id: None,
    };
    let mut chain_tasks = tokio::task::JoinSet::new();
    let payload = b"reload generation payload";

    assert!(state
        .forward_existing_managed_flow(
            &mut chain_tasks,
            crate::protocol_registry::UdpRuntimeServices::from_proxy(&proxy),
            (&old_flow, payload),
        )
        .await
        .is_ok());
    proxy.protocols.on_config_reloaded();
    let stale = state
        .forward_existing_managed_flow(
            &mut chain_tasks,
            crate::protocol_registry::UdpRuntimeServices::from_proxy(&proxy),
            (&old_flow, payload),
        )
        .await;
    assert!(stale.is_err(), "pre-reload resume must not be reused");

    let new_ref = state.register_managed_flow(ManagedUdpFlowResume::new(FakeProviderResume {
        generation: calls.provider_generation(),
    }));
    let new_flow = UdpFlowSnapshot {
        session: old_flow.session.clone(),
        outbound: UdpFlowOutbound::Datagram {
            tag: "new-generation".to_owned(),
            server: "reload-upstream.test".to_owned(),
            port: 5353,
            managed: new_ref,
        },
        client_session_id: None,
    };
    assert!(state
        .forward_existing_managed_flow(
            &mut chain_tasks,
            crate::protocol_registry::UdpRuntimeServices::from_proxy(&proxy),
            (&new_flow, payload),
        )
        .await
        .is_ok());

    assert_eq!(calls.provider_generation(), 1);
    assert_eq!(calls.provider_forwards(), 2);
}

#[cfg(feature = "socks5")]
#[tokio::test]
async fn inventory_executes_handler_produced_by_upstream_provider() {
    use crate::inventory::ProtocolInventory;
    use crate::protocol_registry::ProtocolRegistry;
    use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
    use crate::runtime::udp_flow::registered::{RegisteredUdpState, UpstreamAssociationSend};
    use zero_config::RuntimeConfig;

    let calls = Arc::new(TcpCapabilityCalls::default());
    let config =
        RuntimeConfig::parse(r#"{ "route": { "rules": [], "final": { "type": "direct" } } }"#)
            .expect("minimal runtime config");
    let mut proxy = crate::runtime::Proxy::new(config).expect("minimal proxy");
    let mut registry = ProtocolRegistry::default();
    registry.register_upstream_capability(Arc::new(FakeTcpCapability::new(calls.clone())));
    proxy.protocols = ProtocolInventory { registry };

    let mut state = RegisteredUdpState::new(proxy.protocols.registered_udp_handlers());
    let session = session();
    let payload = b"upstream provider payload";
    let sent = match state
        .start_upstream_udp_flow(
            "fake-inbound",
            UpstreamAssociationSend {
                services: Some(crate::protocol_registry::UdpRuntimeServices::from_proxy(
                    &proxy,
                )),
                session: &session,
                server: "upstream-provider.test",
                port: 1080,
                resume: ManagedUdpFlowResume::new(FakeUpstreamResume),
                payload,
            },
        )
        .await
    {
        Ok(sent) => sent,
        Err(_) => panic!("provider-produced upstream handler did not execute"),
    };

    assert_eq!(sent, payload.len());
    assert_eq!(calls.upstream_provider_sends(), 1);
}

#[tokio::test]
async fn inventory_composes_packet_path_roles_and_builds_carrier() {
    let calls = Arc::new(TcpCapabilityCalls::default());
    let proxy = proxy_with_fake_tcp(calls.clone());
    let leaf = fake_direct_leaf();
    let target = zero_core::Address::Domain("target.test".to_owned());
    let payload = b"packet";
    let mut dispatch = crate::runtime::udp_ingress::UdpIngressRuntime::from_proxy(&proxy)
        .new_dispatch("fake-inbound")
        .await
        .expect("UDP dispatch");
    let claimed = proxy
        .protocols
        .claim_outbound_leaf(leaf.clone())
        .expect("fake packet-path claim");

    let (binding, request) = proxy
        .protocols
        .prepare_claimed_udp_packet_path_pair(
            41,
            &claimed,
            &claimed,
            crate::runtime::udp_flow::packet_path::UdpPacketRef {
                target: &target,
                port: 53,
                payload,
            },
        )
        .expect("fake packet-path pair");
    let (source, snapshot) = binding.into_parts();
    assert_eq!(source.descriptor().tag, "fake-datagram");
    assert_eq!(
        snapshot.lookup_key().datagram_endpoint(),
        ("datagram.test".to_owned(), 2443)
    );
    assert_eq!(request.carrier.descriptor.server, "carrier.test");
    assert_eq!(request.carrier.descriptor.port, 1443);
    assert_eq!(request.datagram.descriptor().cache_key, "fake-datagram-key");

    let sent = match dispatch
        .send_packet_path_chain(
            crate::protocol_registry::UdpAdapterContext::new(
                proxy.config.source_dir(),
                crate::protocol_registry::UdpRuntimeServices::from_proxy(&proxy),
            ),
            request,
        )
        .await
    {
        Ok(sent) => sent,
        Err(_) => panic!("fake packet-path send"),
    };

    assert_eq!(sent, payload.len());
    assert_eq!(calls.packet_descriptors(), 1);
    assert_eq!(calls.packet_sources(), 1);
    assert_eq!(calls.packet_builds(), 1);
    assert_eq!(calls.packet_sends(), 1);
}
