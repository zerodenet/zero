#[cfg(not(feature = "http"))]
use zero_config::InboundConfig;
#[cfg(not(feature = "socks5"))]
use zero_config::OutboundConfig;
#[cfg(any(not(feature = "http"), not(feature = "socks5")))]
use zero_engine::EngineError;

#[cfg(not(feature = "http"))]
#[test]
fn uncompiled_inbound_protocol_fails_with_feature_metadata() {
    let inbound: InboundConfig = serde_json::from_value(serde_json::json!({
        "tag": "disabled-http",
        "listen": { "address": "127.0.0.1", "port": 18080 },
        "protocol": { "type": "http" }
    }))
    .expect("valid HTTP CONNECT inbound config");

    let error = crate::register::protocol_registry()
        .validate_inbounds(&[inbound])
        .expect_err("uncompiled HTTP CONNECT inbound should fail validation");

    assert!(matches!(
        error,
        EngineError::CompiledFeatureDisabled {
            kind: "inbound",
            tag,
            protocol: "http",
            feature: "http",
        } if tag == "disabled-http"
    ));
}

#[cfg(not(feature = "socks5"))]
#[test]
fn uncompiled_outbound_protocol_fails_with_feature_metadata() {
    let outbound: OutboundConfig = serde_json::from_value(serde_json::json!({
        "tag": "disabled-socks",
        "protocol": {
            "type": "socks5",
            "server": "127.0.0.1",
            "port": 1080
        }
    }))
    .expect("valid SOCKS5 outbound config");

    let error = crate::register::protocol_registry()
        .validate_outbounds(&[outbound])
        .expect_err("uncompiled SOCKS5 outbound should fail validation");

    assert!(matches!(
        error,
        EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag,
            protocol: "socks5",
            feature: "socks5",
        } if tag == "disabled-socks"
    ));
}
