use std::cell::RefCell;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use tycho_types::boc::Boc;
use tycho_types::boc::ser::BocHeaderCache;
use tycho_types::cell::{Cell, CellBuilder};

thread_local! {
    static CACHE: RefCell<BocHeaderCache<ahash::RandomState>> = {
        RefCell::new(BocHeaderCache::with_capacity(300000))
    };
}

fn make_big_tree(depth: u8, count: &mut u32, target: u32) -> Cell {
    *count += 1;

    if depth == 0 {
        CellBuilder::build_from(*count).unwrap()
    } else {
        let mut b = CellBuilder::new();
        for _ in 0..4 {
            if *count < target {
                b.store_reference(make_big_tree(depth - 1, count, target))
                    .unwrap();
            }
        }
        b.build().unwrap()
    }
}

fn encode(c: &mut Criterion) {
    let cell = make_big_tree(11, &mut 0, 300000);
    c.bench_function("encode", |b| {
        b.iter(|| {
            CACHE.with_borrow_mut(|c| {
                let bytes = Boc::encode_with_cache(cell.as_ref(), c);
                black_box(bytes);
            });
        })
    });
}

criterion_group!(benches, encode);
criterion_main!(benches);
