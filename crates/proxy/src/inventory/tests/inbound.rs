use std::sync::Arc;

use zero_config::{InboundConfig, RuntimeConfig};

use super::fixtures::TcpCapabilityCalls;
use super::tcp::proxy_with_fake_tcp;

fn fake_inbound() -> InboundConfig {
    RuntimeConfig::parse(
        r#"{
            "inbounds": [{
                "tag": "fake-inbound",
                "listen": { "address": "127.0.0.1", "port": 0 },
                "protocol": { "type": "direct" }
            }],
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("fake inbound config")
    .inbounds
    .into_iter()
    .next()
    .expect("fake inbound")
}

#[tokio::test]
async fn inventory_binds_before_preparing_the_same_inbound_capability() {
    let calls = Arc::new(TcpCapabilityCalls::default());
    let proxy = proxy_with_fake_tcp(calls.clone());
    let inbound = fake_inbound();

    calls.set_fail_bind(true);
    let error = match proxy.protocols.bind_inbound(&inbound, None).await {
        Ok(_) => panic!("fake inbound bind unexpectedly succeeded"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("fake inbound bind failure"));
    assert_eq!(calls.inbound_binds(), 1);
    assert_eq!(calls.inbound_spawns(), 0);

    calls.set_fail_bind(false);
    let bound = proxy
        .protocols
        .bind_inbound(&inbound, None)
        .await
        .expect("fake inbound bind");
    let _operation = proxy
        .protocols
        .prepare_inbound_listener(inbound, proxy.config.source_dir())
        .expect("fake inbound preparation");
    drop(bound);

    assert_eq!(calls.inbound_binds(), 2);
    assert_eq!(calls.inbound_spawns(), 1);
}
