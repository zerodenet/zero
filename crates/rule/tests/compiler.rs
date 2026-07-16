use std::net::{Ipv4Addr, Ipv6Addr};

use ipnet::{Ipv4Net, Ipv6Net};
use zero_rule::{
    CompileError, Ipv4Range, Ipv6Range, Rule, RuleSet, RuleSetCompiler, DISPLAY_NAME_MAX_BYTES,
};

#[test]
fn normalizes_domains_and_eliminates_covered_entries() {
    let input = RuleSet::new(vec![
        Rule::DomainSuffix(" Example.COM. ".to_owned()),
        Rule::DomainSuffix("api.example.com".to_owned()),
        Rule::DomainExact("API.EXAMPLE.COM".to_owned()),
        Rule::DomainExact("Bücher.example".to_owned()),
        Rule::DomainExact("xn--bcher-kva.example".to_owned()),
    ]);

    let (compiled, report) = RuleSetCompiler.compile(input).expect("compile rule set");

    assert_eq!(compiled.domain_suffix(), ["example.com"]);
    assert_eq!(compiled.domain_exact(), ["xn--bcher-kva.example"]);
    assert_eq!(report.input_rules, 5);
    assert_eq!(report.duplicates_removed, 1);
    assert_eq!(report.covered_rules_removed, 2);
    assert_eq!(report.output_entries, 2);
}

#[test]
fn merges_overlapping_and_adjacent_ipv4_ranges() {
    let input = RuleSet::new(vec![
        Rule::Ipv4Cidr(Ipv4Net::new(Ipv4Addr::new(10, 0, 0, 0), 25).unwrap()),
        Rule::Ipv4Cidr(Ipv4Net::new(Ipv4Addr::new(10, 0, 0, 128), 25).unwrap()),
        Rule::Ipv4Cidr(Ipv4Net::new(Ipv4Addr::new(192, 168, 0, 0), 16).unwrap()),
    ]);

    let (compiled, report) = RuleSetCompiler.compile(input).expect("compile rule set");

    assert_eq!(
        compiled.ipv4_ranges(),
        vec![
            Ipv4Range {
                start: u32::from(Ipv4Addr::new(10, 0, 0, 0)),
                end: u32::from(Ipv4Addr::new(10, 0, 0, 255)),
            },
            Ipv4Range {
                start: u32::from(Ipv4Addr::new(192, 168, 0, 0)),
                end: u32::from(Ipv4Addr::new(192, 168, 255, 255)),
            },
        ]
    );
    assert_eq!(report.ranges_merged, 1);
}

#[test]
fn compiles_ipv6_ranges_without_host_enumeration() {
    let input = RuleSet::new(vec![
        Rule::Ipv6Cidr(Ipv6Net::new("2001:db8::".parse().unwrap(), 127).unwrap()),
        Rule::Ipv6Cidr(Ipv6Net::new("2001:db8::2".parse().unwrap(), 127).unwrap()),
    ]);

    let (compiled, report) = RuleSetCompiler.compile(input).expect("compile rule set");

    assert_eq!(
        compiled.ipv6_ranges(),
        vec![Ipv6Range {
            start: u128::from("2001:db8::".parse::<Ipv6Addr>().unwrap()),
            end: u128::from("2001:db8::3".parse::<Ipv6Addr>().unwrap()),
        }]
    );
    assert_eq!(report.ranges_merged, 1);
}

#[test]
fn validates_optional_display_name_for_fixed_header_field() {
    let valid = RuleSet::new(vec![Rule::DomainExact("example.com".to_owned())])
        .with_display_name("AI 服务规则");
    let (compiled, _) = RuleSetCompiler.compile(valid).expect("valid name");
    assert_eq!(compiled.display_name(), Some("AI 服务规则"));

    let too_long = RuleSet::new(vec![Rule::DomainExact("example.com".to_owned())])
        .with_display_name("a".repeat(DISPLAY_NAME_MAX_BYTES + 1));
    assert!(matches!(
        RuleSetCompiler.compile(too_long),
        Err(CompileError::DisplayNameTooLong { .. })
    ));
}

#[test]
fn rejects_empty_or_invalid_domains() {
    let empty = RuleSet::new(vec![Rule::DomainExact(" . ".to_owned())]);
    assert!(matches!(
        RuleSetCompiler.compile(empty),
        Err(CompileError::InvalidDomain { index: 0, .. })
    ));

    assert_eq!(
        RuleSetCompiler.compile(RuleSet::new(Vec::new())),
        Err(CompileError::EmptyRuleSet)
    );

    for keyword in ["含有中文", "has\0nul"] {
        assert!(matches!(
            RuleSetCompiler.compile(RuleSet::new(vec![Rule::DomainKeyword(keyword.to_owned())])),
            Err(CompileError::InvalidDomain { index: 0, .. })
        ));
    }
}
