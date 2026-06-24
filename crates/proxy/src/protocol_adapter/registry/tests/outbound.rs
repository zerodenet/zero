use zero_engine::ResolvedLeafOutbound;

use crate::protocol_adapter::ProtocolRegistry;
use crate::runtime::orchestration::TcpPathCategory;

use super::fixtures::{compiled_in_outbound_leaves, outbound_leaf_name};

#[test]
fn compiled_in_outbound_leaf_variants_have_expected_adapter_claims() {
    let registry = ProtocolRegistry::build();

    for (leaf, expected_claims) in compiled_in_outbound_leaves() {
        let claim_count = registry
            .adapters
            .iter()
            .filter(|adapter| adapter.claims_outbound_leaf(&leaf))
            .count();
        assert_eq!(
            claim_count,
            expected_claims,
            "{} outbound leaf should have {expected_claims} adapter claim(s)",
            outbound_leaf_name(&leaf)
        );

        let resolved = registry.find_outbound_leaf(&leaf);
        assert_eq!(
            resolved.is_ok(),
            expected_claims == 1,
            "{} outbound leaf registry lookup result did not match claim policy",
            outbound_leaf_name(&leaf)
        );
    }
}

#[test]
fn block_outbound_leaf_is_kernel_fact_not_adapter_protocol() {
    let registry = ProtocolRegistry::build();
    let leaf = ResolvedLeafOutbound::Block {
        tag: Some("blocked"),
    };

    let claim_count = registry
        .adapters
        .iter()
        .filter(|adapter| adapter.claims_outbound_leaf(&leaf))
        .count();
    assert_eq!(claim_count, 0, "block should not be claimed by adapters");
    assert!(
        registry.find_outbound_leaf(&leaf).is_err(),
        "block should not resolve through adapter outbound dispatch"
    );

    let runtime = registry
        .outbound_leaf_runtime(&leaf)
        .expect("block should still expose neutral runtime facts");
    assert_eq!(runtime.tcp_path, TcpPathCategory::Block);
    assert_eq!(runtime.health_tag, None);
    assert_eq!(runtime.endpoint, None);
    assert_eq!(runtime.kernel_tag, Some("blocked"));
}
