use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rand9::distr::{Distribution, StandardUniform};
use rand9::{Rng, SeedableRng};
use tycho_types::cell::*;
use tycho_types::dict::*;

fn build_dict_impl<K, V>(id: impl Into<String>, sizes: &[usize], c: &mut Criterion)
where
    StandardUniform: Distribution<K> + Distribution<V>,
    K: StoreDictKey + Ord,
    V: Store,
{
    let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);

    let mut group = c.benchmark_group(id);

    for size in sizes {
        let mut values = (0..*size)
            .map(|_| (rng.random::<K>(), rng.random::<V>()))
            .collect::<Vec<_>>();

        group.bench_with_input(BenchmarkId::new("inserts", size), &values, |b, values| {
            b.iter(|| {
                let mut result = Dict::<K, V>::new();
                for (key, value) in values {
                    result.add(key, value).unwrap();
                }
                black_box(result);
            });
        });

        values.sort_by(|(l, _), (r, _)| l.cmp(r));
        group.bench_with_input(BenchmarkId::new("leaves", size), &values, |b, values| {
            b.iter(|| {
                let result = Dict::<K, V>::try_from_sorted_slice(values).unwrap();
                black_box(result);
            });
        });
    }
}

fn build_dict_group(c: &mut Criterion) {
    macro_rules! decl_dict_benches {
        ($(($k:ty, $v:ident): [$($n:literal),+]),*$(,)?) => {
            $({
                let id = format!(
                    "build_dict({},{})",
                    stringify!($k), stringify!($v)
                );
                build_dict_impl::<$k, $v>(id, &[$($n),+], c);
            });*
        };
    }

    decl_dict_benches![
        (u8, u64): [10, 256],
        (u16, u64): [10, 100, 256, 1000, 10000],
        (u32, u64): [10, 100, 1000, 10000, 100000],
        (u64, u64): [10, 100, 1000, 5000, 10000, 20000, 25000, 50000, 75000, 100000],
    ];
}

criterion_group!(build_dict, build_dict_group);
criterion_main!(build_dict);
