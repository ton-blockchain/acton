use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use tycho_types::prelude::*;

fn test_uniform(c: &mut Criterion) {
    let cells = (0..=32)
        .chain([
            40, 60, 64, 80, 96, 127, 128, 160, 196, 200, 255, 256, 300, 400, 500, 600, 700, 800,
            900, 1000, 1023,
        ])
        .map(|bits| {
            let mut builder = CellBuilder::new();
            builder.store_zeros(bits).unwrap();
            builder.build().unwrap()
        })
        .collect::<Vec<_>>();

    for cell in cells {
        let slice = cell.as_slice().unwrap();
        c.bench_with_input(
            BenchmarkId::new("test slice uniform", slice.size_bits()),
            &slice,
            |b, slice| b.iter(|| black_box(slice.test_uniform())),
        );
    }
}

criterion_group!(benches, test_uniform);
criterion_main!(benches);
