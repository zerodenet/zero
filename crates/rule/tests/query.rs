use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ipnet::{Ipv4Net, Ipv6Net};
use zero_rule::{
    PreparedRuleQuery, QueryError, Rule, RuleMatch, RuleMatcher, RuleSet, RuleSetCompiler,
};

fn matcher() -> zero_rule::CompiledRuleSet {
    RuleSetCompiler
        .compile(RuleSet::new(vec![
            Rule::DomainExact("exact.example".to_owned()),
            Rule::DomainSuffix("service.example".to_owned()),
            Rule::DomainKeyword("special-keyword".to_owned()),
            Rule::Ipv4Cidr(Ipv4Net::new(Ipv4Addr::new(10, 0, 0, 0), 8).unwrap()),
            Rule::Ipv6Cidr(Ipv6Net::new("fd00::".parse().unwrap(), 8).unwrap()),
        ]))
        .expect("compile matcher")
        .0
}

#[test]
fn uses_one_query_entry_point_for_every_rule_type() {
    let matcher = matcher();

    for domain in [
        "EXACT.EXAMPLE.",
        "api.service.example",
        "prefix-special-keyword.example",
    ] {
        let query = PreparedRuleQuery::new(Some(domain), None).expect("prepare domain");
        assert!(
            matcher.matches(&query),
            "expected domain match for {domain}"
        );
    }

    for address in [
        IpAddr::V4(Ipv4Addr::new(10, 20, 30, 40)),
        IpAddr::V6("fd12::1".parse::<Ipv6Addr>().unwrap()),
    ] {
        let query = PreparedRuleQuery::new(None, Some(address)).expect("prepare address");
        assert!(matcher.matches(&query), "expected IP match for {address}");
    }
}

#[test]
fn treats_domain_and_destination_ip_as_alternative_facts() {
    let matcher = matcher();
    let query = PreparedRuleQuery::new(
        Some("unmatched.example"),
        Some(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))),
    )
    .expect("prepare query");

    assert!(matcher.matches(&query));
}

#[test]
fn respects_domain_label_boundaries_and_ip_range_edges() {
    let matcher = matcher();
    for domain in ["notservice.example", "service.example.invalid"] {
        let query = PreparedRuleQuery::new(Some(domain), None).expect("prepare domain");
        assert!(!matcher.matches(&query));
    }

    let outside = PreparedRuleQuery::new(None, Some(IpAddr::V4(Ipv4Addr::new(11, 0, 0, 0))))
        .expect("prepare address");
    assert!(!matcher.matches(&outside));
}

#[test]
fn rejects_queries_without_matchable_facts() {
    assert_eq!(PreparedRuleQuery::new(None, None), Err(QueryError::Empty));
    assert!(matches!(
        PreparedRuleQuery::new(Some("."), None),
        Err(QueryError::InvalidDomain { .. })
    ));
}

#[test]
fn reports_match_category_with_stable_precedence() {
    let matcher = matcher();
    let cases = [
        ("exact.example", RuleMatch::DomainExact),
        ("api.service.example", RuleMatch::DomainSuffix),
        ("prefix-special-keyword.example", RuleMatch::DomainKeyword),
    ];
    for (domain, expected) in cases {
        let query = PreparedRuleQuery::new(Some(domain), None).unwrap();
        assert_eq!(matcher.lookup(&query), Some(expected));
    }

    let both = PreparedRuleQuery::new(
        Some("exact.example"),
        Some(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))),
    )
    .unwrap();
    assert_eq!(
        RuleMatcher::lookup(&matcher, &both),
        Some(RuleMatch::DomainExact)
    );
}
