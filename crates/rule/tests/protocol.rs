use zero_rule::protocol::{decode_json, encode_json, RuleProtocolError, ZERO_RULE_IR_VERSION};
use zero_rule::{Rule, RuleSet};

#[test]
fn zero_rule_ir_v1_round_trips_every_rule_type() {
    let source = br#"{
  "version": 1,
  "name": "AI services",
  "rules": [
    {"type":"domain_exact","value":"api.example.com"},
    {"type":"domain_suffix","value":"example.org"},
    {"type":"domain_keyword","value":"special"},
    {"type":"ipv4_cidr","value":"10.0.0.0/8"},
    {"type":"ipv6_cidr","value":"fd00::/8"}
  ]
}"#;

    let decoded = decode_json(source).expect("decode Zero Rule IR");
    assert_eq!(decoded.display_name.as_deref(), Some("AI services"));
    assert_eq!(decoded.rules.len(), 5);
    assert!(matches!(decoded.rules[0], Rule::DomainExact(_)));

    let encoded = encode_json(&decoded).expect("encode Zero Rule IR");
    assert_eq!(decode_json(&encoded).unwrap(), decoded);
}

#[test]
fn rejects_unknown_versions_fields_and_rule_types() {
    let wrong_version = br#"{"version":2,"rules":[]}"#;
    assert!(matches!(
        decode_json(wrong_version),
        Err(RuleProtocolError::UnsupportedVersion {
            actual: 2,
            expected: ZERO_RULE_IR_VERSION
        })
    ));

    for input in [
        br#"{"version":1,"unknown":true,"rules":[]}"#.as_slice(),
        br#"{"version":1,"rules":[{"type":"process_name","value":"app"}]}"#.as_slice(),
    ] {
        assert!(matches!(
            decode_json(input),
            Err(RuleProtocolError::Json(_))
        ));
    }
}

#[test]
fn rejects_invalid_cidr_and_oversized_values() {
    let invalid = br#"{"version":1,"rules":[{"type":"ipv4_cidr","value":"not-a-cidr"}]}"#;
    assert!(matches!(
        decode_json(invalid),
        Err(RuleProtocolError::InvalidIpv4 { index: 0, .. })
    ));

    let rule_set = RuleSet::new(vec![Rule::DomainExact("a".repeat(4_097))]);
    assert!(matches!(
        encode_json(&rule_set),
        Err(RuleProtocolError::RuleValueTooLong { index: 0, .. })
    ));
}
