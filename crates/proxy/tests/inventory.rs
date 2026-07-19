use zero_config::RuntimeConfig;
use zero_proxy::{compiled_protocol_features, ProtocolInventory, Proxy};

#[test]
fn inventory_exposes_a_consistent_public_capability_matrix() {
    let inventory = ProtocolInventory::default();
    let inbound_names = inventory.supported_inbounds();
    let outbound_names = inventory.supported_outbounds();
    let capabilities = inventory.protocol_capabilities();

    assert!(inbound_names.contains(&"direct"));
    assert!(outbound_names.contains(&"direct"));
    assert!(outbound_names.contains(&"block"));
    assert!(capabilities.iter().all(|capability| capability.compiled));

    let capability_names = capabilities
        .iter()
        .map(|capability| capability.protocol.as_str())
        .collect::<Vec<_>>();
    assert!(capability_names.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(capability_names.contains(&"direct"));
    assert!(capability_names.contains(&"block"));

    for inbound in inbound_names {
        assert!(
            capabilities
                .iter()
                .any(|capability| capability.protocol == inbound),
            "inbound `{inbound}` must have a public capability descriptor"
        );
    }
    for outbound in outbound_names {
        assert!(
            capabilities
                .iter()
                .any(|capability| capability.protocol == outbound),
            "outbound `{outbound}` must have a public capability descriptor"
        );
    }

    let features = compiled_protocol_features();
    assert!(features.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn inventory_validates_the_same_config_accepted_by_proxy_construction() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [{
                "tag": "direct-in",
                "listen": { "address": "127.0.0.1", "port": 0 },
                "protocol": { "type": "direct" }
            }],
            "outbounds": [{
                "tag": "direct-out",
                "protocol": { "type": "direct" }
            }],
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("parse direct config");
    let inventory = ProtocolInventory::default();

    assert!(inventory.supports_inbound_protocol(&config.inbounds[0].protocol));
    assert!(inventory.supports_outbound_protocol(&config.outbounds[0].protocol));
    inventory
        .validate_config(&config)
        .expect("inventory validation");
    Proxy::new(config).expect("proxy construction");
}

#[cfg(not(feature = "hysteria2"))]
#[test]
fn proxy_construction_rejects_an_uncompiled_inventory_protocol() {
    use zero_engine::EngineError;

    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [{
                "tag": "uncompiled-hysteria2",
                "protocol": {
                    "type": "hysteria2",
                    "server": "127.0.0.1",
                    "port": 443,
                    "password": "password"
                }
            }],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "uncompiled-hysteria2" }
            }
        }"#,
    )
    .expect("parse config for an uncompiled protocol");

    let error = Proxy::new(config).expect_err("uncompiled protocol must fail early");
    assert!(matches!(
        error,
        EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            protocol: "hysteria2",
            feature: "hysteria2",
            ..
        }
    ));
}
