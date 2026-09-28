use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rand9::distr::{Distribution, StandardUniform};
use rand9::{Rng, SeedableRng};
use tycho_types::cell::*;
use tycho_types::dict::*;

fn build_dict_impl<K, V>(id: BenchmarkId, num_elements: usize, c: &mut Criterion)
where
    StandardUniform: Distribution<K> + Distribution<V>,
    K: StoreDictKey,
    V: Store,
{
    let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);

    let values = (0..num_elements)
        .map(|_| (rng.random::<K>(), rng.random::<V>()))
        .collect::<Vec<_>>();

    c.bench_with_input(id, &values, |b, values| {
        b.iter(|| {
            let mut result = Dict::<K, V>::new();
            for (key, value) in values {
                result.set(key, value).unwrap();
            }
            black_box(result);
        });
    });
}

fn build_dict_group(c: &mut Criterion) {
    macro_rules! decl_dict_benches {
        ($({ $n:literal, $k:ty, $v:ident }),*$(,)?) => {
            $({
                let id = BenchmarkId::new(
                    "build_dict",
                    format!(
                        "size={}; key={}; value={}",
                        $n, stringify!($k), stringify!($v)
                    )
                );
                build_dict_impl::<$k, $v>(id, $n, c);
            });*
        };
    }

    decl_dict_benches![
        { 10, u8, u64 },
        { 256, u8, u64 },

        { 10, u16, u64 },
        { 100, u16, u64 },
        { 256, u16, u64 },
        { 10000, u16, u64 },

        { 10, u32, u64 },
        { 100, u32, u64 },
        { 1000, u32, u64 },
        { 100000, u32, u64 },

        { 10, u64, u64 },
        { 100, u64, u64 },
        { 1000, u64, u64 },
        { 100000, u64, u64 },
    ];
}

criterion_group!(build_dict, build_dict_group);
criterion_main!(build_dict);
