use std::env;
use std::hint::black_box;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Instant;

use zero_rule::protocol::decode_json;
use zero_rule::zrs::{encode, MappedRuleSet, PrewarmPolicy, VerifiedRuleSet, VerifyMode};
use zero_rule::{PreparedRuleQuery, Rule, RuleSet, RuleSetCompiler};

fn main() {
    let source = env::var_os("ZERO_RULE_IR");
    let started = Instant::now();
    let rules = match &source {
        Some(path) => decode_json(&std::fs::read(path).expect("read ZERO_RULE_IR"))
            .expect("decode Zero Rule IR"),
        None => synthetic_rules(100_000),
    };
    let decode_time = started.elapsed();
    let started = Instant::now();
    let (compiled, report) = RuleSetCompiler.compile(rules).expect("compile rules");
    let compile_time = started.elapsed();
    let queries = sample_queries(&compiled, 10_000);
    let started = Instant::now();
    let bytes = encode(&compiled).expect("encode ZRS");
    let encode_time = started.elapsed();
    let started = Instant::now();
    let mapped = VerifiedRuleSet::from_bytes(&bytes, VerifyMode::FullChecksum).expect("verify ZRS");
    let verify_time = started.elapsed();
    let path = env::temp_dir().join(format!(
        "zero-rule-bench-{}-{}.zrs",
        std::process::id(),
        bytes.len()
    ));
    std::fs::write(&path, &bytes).expect("write temporary ZRS");
    let started = Instant::now();
    let mmap = MappedRuleSet::open(&path, VerifyMode::Structure).expect("map ZRS");
    let map_time = started.elapsed();
    let prewarm = mmap.prewarm(PrewarmPolicy::Roots);

    for query in &queries {
        assert_eq!(compiled.matches(query), mapped.matches(query));
        assert_eq!(compiled.lookup(query), mmap.lookup(query));
    }
    let memory_time = measure(&queries, |query| compiled.matches(query));
    let borrowed_zrs_time = measure(&queries, |query| mapped.matches(query));
    let mmap_zrs_time = measure(&queries, |query| mmap.matches(query));
    let metadata = mmap.metadata();
    println!(
        "source={}",
        source
            .as_ref()
            .map_or_else(|| "synthetic".into(), |path| path.to_string_lossy())
    );
    println!(
        "input_rules={} output_entries={} zrs_bytes={} index_bytes={}",
        report.input_rules,
        report.output_entries,
        bytes.len(),
        metadata.index_bytes()
    );
    println!(
        "decode_ms={:.3} compile_ms={:.3} encode_ms={:.3} verify_ms={:.3} map_ms={:.3}",
        millis(decode_time),
        millis(compile_time),
        millis(encode_time),
        millis(verify_time),
        millis(map_time)
    );
    println!(
        "queries={} memory_ns/query={:.1} borrowed_zrs_ns/query={:.1} mmap_zrs_ns/query={:.1}",
        queries.len(),
        nanos(memory_time, queries.len()),
        nanos(borrowed_zrs_time, queries.len()),
        nanos(mmap_zrs_time, queries.len())
    );
    println!(
        "prewarm=roots touched_pages={} page_size={}",
        prewarm.touched_pages, prewarm.page_size
    );
    drop(mmap);
    std::fs::remove_file(path).expect("remove temporary ZRS");
}

fn synthetic_rules(count: usize) -> RuleSet {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    RuleSet::new(
        (0..count)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                Rule::DomainExact(format!("{state:016x}.service.example.net"))
            })
            .collect(),
    )
}

fn sample_queries(compiled: &zero_rule::CompiledRuleSet, maximum: usize) -> Vec<PreparedRuleQuery> {
    let mut queries = Vec::new();
    for (index, value) in compiled.domain_exact().iter().take(maximum / 2).enumerate() {
        queries.push(PreparedRuleQuery::new(Some(value), None).unwrap());
        queries
            .push(PreparedRuleQuery::new(Some(&format!("missing-{index}.invalid")), None).unwrap());
    }
    for value in compiled
        .domain_suffix()
        .iter()
        .take(maximum.saturating_sub(queries.len()))
    {
        queries.push(PreparedRuleQuery::new(Some(&format!("probe.{value}")), None).unwrap());
    }
    for value in compiled
        .domain_keyword()
        .iter()
        .take(maximum.saturating_sub(queries.len()))
    {
        queries
            .push(PreparedRuleQuery::new(Some(&format!("probe-{value}.example")), None).unwrap());
    }
    for range in compiled
        .ipv4_ranges()
        .iter()
        .take(maximum.saturating_sub(queries.len()))
    {
        queries.push(
            PreparedRuleQuery::new(None, Some(IpAddr::V4(Ipv4Addr::from(range.start)))).unwrap(),
        );
    }
    for range in compiled
        .ipv6_ranges()
        .iter()
        .take(maximum.saturating_sub(queries.len()))
    {
        queries.push(
            PreparedRuleQuery::new(None, Some(IpAddr::V6(Ipv6Addr::from(range.start)))).unwrap(),
        );
    }
    queries
}

fn measure(
    queries: &[PreparedRuleQuery],
    matcher: impl Fn(&PreparedRuleQuery) -> bool,
) -> std::time::Duration {
    let started = Instant::now();
    for query in queries {
        black_box(matcher(query));
    }
    started.elapsed()
}

fn millis(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
fn nanos(duration: std::time::Duration, count: usize) -> f64 {
    duration.as_nanos() as f64 / count.max(1) as f64
}
