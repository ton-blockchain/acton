use std::hint::black_box;

use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use rand9::distr::{Distribution, StandardUniform};
use rand9::{Rng, SeedableRng};
use tycho_types::cell::*;
use tycho_types::dict::*;

fn build_dict_inserts<K, V>(num_elements: usize) -> Dict<K, V>
where
    StandardUniform: Distribution<K> + Distribution<V>,
    K: StoreDictKey,
    V: Store,
{
    let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);

    let mut result = Dict::<K, V>::new();
    for _ in 0..num_elements {
        let key = rng.random::<K>();
        let value = rng.random::<V>();
        result.add(key, value).unwrap();
    }
    result
}

fn build_dict_leaves<K, V>(num_elements: usize) -> Dict<K, V>
where
    StandardUniform: Distribution<K> + Distribution<V>,
    K: StoreDictKey + Ord,
    V: Store,
{
    let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);

    let mut values = (0..num_elements)
        .map(|_| (rng.random::<K>(), rng.random::<V>()))
        .collect::<Vec<_>>();
    values.sort_by(|(l, _), (r, _)| l.cmp(r));

    Dict::<K, V>::try_from_sorted_slice(&values).unwrap()
}

#[library_benchmark]
#[bench::small(10)]
#[bench::medium(100)]
#[bench::large(1000)]
#[bench::xlarge(10000)]
fn bench_build_dict_u64_u64_inserts(num_elements: usize) -> Dict<u64, u64> {
    black_box(build_dict_inserts(num_elements))
}

#[library_benchmark]
#[bench::small(10)]
#[bench::medium(100)]
#[bench::large(1000)]
#[bench::xlarge(10000)]
fn bench_build_dict_u64_u64_leaves(num_elements: usize) -> Dict<u64, u64> {
    black_box(build_dict_leaves(num_elements))
}

library_benchmark_group!(
    name = build_dict;
    benchmarks = bench_build_dict_u64_u64_inserts, bench_build_dict_u64_u64_leaves
);

main!(library_benchmark_groups = build_dict);
