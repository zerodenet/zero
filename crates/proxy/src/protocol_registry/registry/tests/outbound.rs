use zero_engine::ResolvedLeafOutbound;

use crate::runtime::path::TcpPathCategory;

use super::fixtures::{compiled_in_outbound_leaves, outbound_leaf_name};

#[test]
fn compiled_in_outbound_leaf_variants_have_expected_adapter_claims() {
    let registry = crate::register::protocol_registry();

    for (leaf, expected_claims) in compiled_in_outbound_leaves() {
        let claim_count = registry
            .entries
            .iter()
            .filter(|entry| entry.tcp.claims_outbound_leaf(&leaf))
            .count();
        assert_eq!(
            claim_count,
            expected_claims,
            "{} outbound leaf should have {expected_claims} adapter claim(s)",
            outbound_leaf_name(&leaf)
        );

        let claimed = registry.claim_outbound_leaf(&leaf);
        assert_eq!(
            claimed.as_ref().map(|claim| claim.tcp.is_some()).ok(),
            Some(expected_claims == 1),
            "{} claimed outbound lookup should expose runtime facts and optional adapter with the same claim policy",
            outbound_leaf_name(&leaf)
        );
    }
}

#[test]
fn block_outbound_leaf_is_kernel_fact_not_adapter_protocol() {
    let registry = crate::register::protocol_registry();
    let leaf = ResolvedLeafOutbound::Block {
        tag: Some("blocked"),
    };

    let claim_count = registry
        .entries
        .iter()
        .filter(|entry| entry.tcp.claims_outbound_leaf(&leaf))
        .count();
    assert_eq!(claim_count, 0, "block should not be claimed by adapters");

    let claimed = registry
        .claim_outbound_leaf(&leaf)
        .expect("block should still expose claimed runtime facts");
    assert!(
        claimed.tcp.is_none(),
        "block should not expose an outbound adapter"
    );

    let runtime = registry
        .outbound_leaf_runtime(&leaf)
        .expect("block should still expose neutral runtime facts");
    assert_eq!(runtime.tcp_path, TcpPathCategory::Block);
    assert_eq!(runtime.health_tag, None);
    assert_eq!(runtime.endpoint, None);
    assert_eq!(runtime.kernel_tag, Some("blocked"));
    assert_eq!(claimed.runtime.tcp_path, TcpPathCategory::Block);
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
#[test]
fn udp_outbound_leaf_lookup_matches_tcp_claim_policy() {
    let registry = crate::register::protocol_registry();

    for (leaf, expected_claims) in compiled_in_outbound_leaves() {
        let claimed = registry.claim_outbound_leaf(&leaf);
        assert_eq!(
            claimed.as_ref().map(|claim| claim.udp.is_some()).ok(),
            Some(expected_claims == 1),
            "{} claimed udp-flow lookup should follow the same claim policy as tcp outbound lookup",
            outbound_leaf_name(&leaf)
        );
    }
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
#[test]
fn packet_path_leaf_lookup_matches_tcp_claim_policy() {
    let registry = crate::register::protocol_registry();

    for (leaf, expected_claims) in compiled_in_outbound_leaves() {
        let claimed = registry.claim_outbound_leaf(&leaf);
        assert_eq!(
            claimed.as_ref().map(|claim| claim.packet_path.is_some()).ok(),
            Some(expected_claims == 1),
            "{} claimed packet-path lookup should follow the same claim policy as tcp outbound lookup",
            outbound_leaf_name(&leaf)
        );
    }
}
