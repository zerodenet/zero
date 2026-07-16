use std::hint::black_box;
use std::time::{Duration, Instant};

use fst::Set;

const ENTRY_COUNT: usize = 100_000;
const QUERY_ROUNDS: usize = 20;

fn main() {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut keys = (0..ENTRY_COUNT)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            format!("{state:016x}.service.example.net")
        })
        .collect::<Vec<_>>();
    keys.sort_unstable();
    let fst = Set::from_iter(keys.iter()).expect("build FST");
    let queries = (0..ENTRY_COUNT)
        .step_by(10)
        .flat_map(|index| {
            [
                keys[index].clone(),
                format!("missing-{index:06}.service.example.net"),
            ]
        })
        .collect::<Vec<_>>();

    let flat_bytes = 4 + (keys.len() + 1) * 8 + keys.iter().map(String::len).sum::<usize>();
    let flat_time = measure(|| {
        for query in &queries {
            black_box(
                keys.binary_search_by(|candidate| candidate.as_str().cmp(query.as_str()))
                    .is_ok(),
            );
        }
    });
    let fst_time = measure(|| {
        for query in &queries {
            black_box(fst.contains(query));
        }
    });
    let operations = queries.len() * QUERY_ROUNDS;

    println!("entries={ENTRY_COUNT} queries={operations}");
    println!(
        "flat bytes={} ns/query={:.1}",
        flat_bytes,
        nanos_per_query(flat_time, operations)
    );
    println!(
        "fst  bytes={} ns/query={:.1}",
        fst.as_fst().as_bytes().len(),
        nanos_per_query(fst_time, operations)
    );
}

fn measure(mut operation: impl FnMut()) -> Duration {
    operation();
    let start = Instant::now();
    for _ in 0..QUERY_ROUNDS {
        operation();
    }
    start.elapsed()
}

fn nanos_per_query(duration: Duration, operations: usize) -> f64 {
    duration.as_nanos() as f64 / operations as f64
}
