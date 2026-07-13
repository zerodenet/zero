use super::fixtures::{compiled_in_inbound_configs, inbound_protocol_name};

#[test]
fn compiled_in_inbound_variants_have_exactly_one_registered_adapter() {
    let registry = crate::register::protocol_registry();

    for config in compiled_in_inbound_configs() {
        let claim_count = registry
            .entries
            .iter()
            .filter(|entry| entry.support.supports_inbound(&config))
            .count();
        assert_eq!(
            claim_count,
            1,
            "{} inbound config should be claimed by exactly one adapter",
            inbound_protocol_name(&config)
        );
        assert!(
            registry.find_inbound(&config).is_ok(),
            "{} inbound config should resolve through ProtocolRegistry::find_inbound",
            inbound_protocol_name(&config)
        );
    }
}
