use std::collections::{BTreeSet, HashSet};

use crate::error::CompileError;
use crate::model::{
    CompileReport, CompiledRuleSet, Ipv4Range, Ipv6Range, Rule, RuleSet, DISPLAY_NAME_MAX_BYTES,
};
use crate::normalize;

#[derive(Debug, Clone, Copy, Default)]
pub struct RuleSetCompiler;

impl RuleSetCompiler {
    pub fn compile(
        &self,
        input: RuleSet,
    ) -> Result<(CompiledRuleSet, CompileReport), CompileError> {
        validate_display_name(input.display_name.as_deref())?;
        if input.rules.len() > crate::MAX_RULES {
            return Err(CompileError::TooManyRules {
                actual: input.rules.len(),
                maximum: crate::MAX_RULES,
            });
        }
        if input.rules.is_empty() {
            return Err(CompileError::EmptyRuleSet);
        }

        let input_rules = input.rules.len();
        let mut exact = BTreeSet::new();
        let mut suffix = BTreeSet::new();
        let mut keyword = BTreeSet::new();
        let mut ipv4 = BTreeSet::new();
        let mut ipv6 = BTreeSet::new();
        let mut duplicates_removed = 0;

        for (index, rule) in input.rules.into_iter().enumerate() {
            let inserted = match rule {
                Rule::DomainExact(value) => {
                    let value = normalize::domain(&value).map_err(|reason| {
                        CompileError::InvalidDomain {
                            index,
                            value,
                            reason,
                        }
                    })?;
                    exact.insert(value)
                }
                Rule::DomainSuffix(value) => {
                    let value = normalize::domain(&value).map_err(|reason| {
                        CompileError::InvalidDomain {
                            index,
                            value,
                            reason,
                        }
                    })?;
                    suffix.insert(value)
                }
                Rule::DomainKeyword(value) => {
                    let value = normalize::keyword(&value).map_err(|reason| {
                        CompileError::InvalidDomain {
                            index,
                            value,
                            reason,
                        }
                    })?;
                    keyword.insert(value)
                }
                Rule::Ipv4Cidr(network) => ipv4.insert(Ipv4Range {
                    start: u32::from(network.network()),
                    end: u32::from(network.broadcast()),
                }),
                Rule::Ipv6Cidr(network) => ipv6.insert(ipv6_range(network)),
            };
            if !inserted {
                duplicates_removed += 1;
            }
        }

        let (domain_suffix, suffix_set, suffix_covered) = eliminate_covered_suffixes(suffix);
        let mut covered_rules_removed = suffix_covered;
        let domain_exact = exact
            .into_iter()
            .filter(|domain| {
                let covered = normalize::is_covered_by_suffix(domain, &suffix_set);
                covered_rules_removed += usize::from(covered);
                !covered
            })
            .collect::<Vec<_>>();

        let (ipv4_ranges, ipv4_merged) = merge_ranges(
            ipv4.into_iter().collect(),
            |range: &Ipv4Range| range.start,
            |range| range.end,
            |range, end| range.end = end,
        );
        let (ipv6_ranges, ipv6_merged) = merge_ranges(
            ipv6.into_iter().collect(),
            |range: &Ipv6Range| range.start,
            |range| range.end,
            |range, end| range.end = end,
        );

        let compiled = CompiledRuleSet {
            display_name: input.display_name,
            domain_exact,
            domain_suffix,
            domain_keyword: keyword.into_iter().collect(),
            ipv4_ranges,
            ipv6_ranges,
        };
        let report = CompileReport {
            input_rules,
            output_entries: compiled.entry_count(),
            duplicates_removed,
            covered_rules_removed,
            ranges_merged: ipv4_merged + ipv6_merged,
        };

        Ok((compiled, report))
    }
}

fn validate_display_name(name: Option<&str>) -> Result<(), CompileError> {
    let Some(name) = name else {
        return Ok(());
    };
    if name.is_empty() {
        return Err(CompileError::EmptyDisplayName);
    }
    if name.as_bytes().contains(&0) {
        return Err(CompileError::DisplayNameContainsNul);
    }
    if name.len() > DISPLAY_NAME_MAX_BYTES {
        return Err(CompileError::DisplayNameTooLong {
            actual: name.len(),
            maximum: DISPLAY_NAME_MAX_BYTES,
        });
    }
    Ok(())
}

fn eliminate_covered_suffixes(suffixes: BTreeSet<String>) -> (Vec<String>, HashSet<String>, usize) {
    let mut ordered = suffixes.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        normalize::label_count(left)
            .cmp(&normalize::label_count(right))
            .then_with(|| left.cmp(right))
    });

    let mut accepted = HashSet::with_capacity(ordered.len());
    let mut covered = 0;
    for suffix in ordered {
        if normalize::is_covered_by_suffix(&suffix, &accepted) {
            covered += 1;
        } else {
            accepted.insert(suffix);
        }
    }
    let mut result = accepted.iter().cloned().collect::<Vec<_>>();
    result.sort();
    (result, accepted, covered)
}

fn ipv6_range(network: ipnet::Ipv6Net) -> Ipv6Range {
    let start = u128::from(network.network());
    let host_bits = 128 - network.prefix_len();
    let host_mask = if host_bits == 128 {
        u128::MAX
    } else if host_bits == 0 {
        0
    } else {
        (1_u128 << host_bits) - 1
    };
    Ipv6Range {
        start,
        end: start | host_mask,
    }
}

fn merge_ranges<T, V>(
    ranges: Vec<T>,
    start: impl Fn(&T) -> V,
    end: impl Fn(&T) -> V,
    set_end: impl Fn(&mut T, V),
) -> (Vec<T>, usize)
where
    V: Copy + Ord + SaturatingAddOne,
{
    let mut merged: Vec<T> = Vec::with_capacity(ranges.len());
    let mut merge_count = 0;
    for range in ranges {
        if let Some(previous) = merged.last_mut() {
            if start(&range) <= end(previous).saturating_add_one() {
                if end(&range) > end(previous) {
                    set_end(previous, end(&range));
                }
                merge_count += 1;
                continue;
            }
        }
        merged.push(range);
    }
    (merged, merge_count)
}

trait SaturatingAddOne {
    fn saturating_add_one(self) -> Self;
}

impl SaturatingAddOne for u32 {
    fn saturating_add_one(self) -> Self {
        self.saturating_add(1)
    }
}

impl SaturatingAddOne for u128 {
    fn saturating_add_one(self) -> Self {
        self.saturating_add(1)
    }
}
