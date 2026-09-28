use std::hint::black_box;

use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use tycho_types::prelude::*;

#[library_benchmark]
#[bench::small(2)]
#[bench::medium(4)]
#[bench::large(8)]
#[bench::xlarge(10)]
fn test(bits: u32) {
    let mut builder = CellBuilder::new();
    builder.store_zeros(2u16.pow(bits) - 1u16).unwrap();
    let cell = builder.build().unwrap();

    let slice = cell.as_slice().unwrap();
    black_box(slice.test_uniform());
}

library_benchmark_group!(
    name = test_uniform;
    benchmarks = test
);

main!(library_benchmark_groups = test_uniform);
