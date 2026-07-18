use std::sync::Arc;

use zero_core::Address;
use zero_router::{RouteAction, Rule, RuleCondition, RuleSet, RuleSetMatcher};
use zero_rule::{Rule as MatcherRule, RuleSet as MatcherRuleSet, RuleSetCompiler};

#[test]
fn routes_domain_suffix_to_reject() {
    let rules = vec![Rule {
        condition: RuleCondition::Domain(vec!["blocked.example".to_owned()]),
        action: RouteAction::Reject,
    }];
    let ruleset = RuleSet::new(rules, RouteAction::Direct);

    let action = ruleset.decide(&Address::Domain("api.blocked.example".to_owned()), None);

    assert_eq!(action, RouteAction::Reject);
}

#[test]
fn borrowed_decision_reuses_ruleset_action() {
    let rules = vec![Rule {
        condition: RuleCondition::Domain(vec!["blocked.example".to_owned()]),
        action: RouteAction::Reject,
    }];
    let ruleset = RuleSet::new(rules, RouteAction::Direct);

    let action = ruleset.decide_ref(&Address::Domain("api.blocked.example".to_owned()), None);

    assert_eq!(action, &RouteAction::Reject);
}

#[test]
fn route_condition_uses_zero_rule_matcher_and_reports_its_tag() {
    let (compiled, _) = RuleSetCompiler
        .compile(MatcherRuleSet::new(vec![
            MatcherRule::DomainSuffix("example.com".to_owned()),
            MatcherRule::Ipv4Cidr("10.0.0.0/8".parse().unwrap()),
        ]))
        .expect("compile matcher set");
    let ruleset = RuleSet::new(
        vec![Rule {
            condition: RuleCondition::RuleSet(RuleSetMatcher::new("private", Arc::new(compiled))),
            action: RouteAction::Reject,
        }],
        RouteAction::Direct,
    );

    assert_eq!(
        ruleset.decide(&Address::Domain("api.Example.COM".to_owned()), None),
        RouteAction::Reject
    );
    assert_eq!(
        ruleset.decide(&Address::Ipv4([10, 2, 3, 4]), None),
        RouteAction::Reject
    );

    let trace = ruleset.decide_trace(&Address::Domain("api.example.com".to_owned()), None);
    assert_eq!(
        trace.matched_rule.expect("matched rule").condition,
        "rule_set: private"
    );
}
