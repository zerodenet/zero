use zero_core::Address;
use zero_router::{RouteAction, Rule, RuleCondition, RuleSet};

#[test]
fn routes_domain_suffix_to_reject() {
    let rules = vec![Rule {
        condition: RuleCondition::Domain(vec!["blocked.example".to_owned()]),
        action: RouteAction::Reject,
    }];
    let ruleset = RuleSet::new(rules, RouteAction::Direct);

    let action = ruleset.decide(&Address::Domain("api.blocked.example".to_owned()));

    assert_eq!(action, RouteAction::Reject);
}

#[test]
fn borrowed_decision_reuses_ruleset_action() {
    let rules = vec![Rule {
        condition: RuleCondition::Domain(vec!["blocked.example".to_owned()]),
        action: RouteAction::Reject,
    }];
    let ruleset = RuleSet::new(rules, RouteAction::Direct);

    let action = ruleset.decide_ref(&Address::Domain("api.blocked.example".to_owned()));

    assert_eq!(action, &RouteAction::Reject);
}
