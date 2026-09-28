use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rand9::distr::{Distribution, StandardUniform};
use rand9::{Rng, SeedableRng};
use tycho_types::cell::*;
use tycho_types::dict::*;

fn build_dict_impl<K, V>(id: impl Into<String>, sizes: &[usize], c: &mut Criterion)
where
    StandardUniform: Distribution<K> + Distribution<V>,
    K: Copy + StoreDictKey + Ord,
    V: Copy + Store,
{
    let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);

    let mut group = c.benchmark_group(id);

    for size in sizes {
        let mut values = (0..*size)
            .map(|_| (rng.random::<K>(), rng.random::<V>()))
            .collect::<Vec<_>>();
        values.sort_by(|(l, _), (r, _)| l.cmp(r));
        let initial = Dict::try_from_sorted_slice(&values).unwrap();

        let mut operations = (0..*size)
            .map(|_| {
                let key = rng.random::<K>();
                let value = rng.random::<bool>().then(|| rng.random::<V>());
                (key, value)
            })
            .collect::<Vec<_>>();
        operations.sort_by(|(l, _), (r, _)| l.cmp(r));
        operations.dedup_by(|(l, _), (r, _)| (*l).eq(r));

        group.bench_with_input(
            BenchmarkId::new("manual", size),
            &operations,
            |b, operations| {
                b.iter(|| {
                    let mut result = initial.clone();
                    for (key, value) in operations {
                        match value {
                            Some(value) => {
                                result.add(key, value).unwrap();
                            }
                            None => {
                                result.remove_raw(key).unwrap();
                            }
                        }
                    }
                    black_box(result);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batched", size),
            &operations,
            |b, operations| {
                b.iter(|| {
                    let mut result = initial.clone();
                    result
                        .modify_with_sorted_iter(operations.iter().copied())
                        .unwrap();
                    black_box(result);
                });
            },
        );
    }
}

fn modify_dict_group(c: &mut Criterion) {
    macro_rules! decl_dict_benches {
        ($(($k:ty, $v:ident): [$($n:literal),+]),*$(,)?) => {
            $({
                let id = format!(
                    "modify_dict({},{})",
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

criterion_group!(modify_dict, modify_dict_group);
criterion_main!(modify_dict);
