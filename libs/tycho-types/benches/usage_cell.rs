use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use tycho_types::cell::RefsIter;
use tycho_types::prelude::*;

const BOC: &str = "te6ccgECCAEAAWQAAnPP9noJKCEBL3oZerOiIcNghuL96V3wIcuYOWQdvNC+2fqCEIJDQAAAAAAAAAAAAAAAAZa8xB6QABNAAgEAUO3QlUyMI4dEepUMw3Ou6oSqq8+1lyHkjOGFK6DAn6TXAAAAAAAAAAABFP8A9KQT9LzyyAsDAgEgBwQC5vJx1wEBwADyeoMI1xjtRNCDB9cB1ws/yPgozxYjzxbJ+QADcdcBAcMAmoMH1wFRE7ry4GTegEDXAYAg1wGAINcBVBZ1+RDyqPgju/J5Zr74I4EHCKCBA+ioUiC8sfJ0AiCCEEzuZGy64w8ByMv/yz/J7VQGBQA+ghAWnj4Ruo4R+AACkyDXSpd41wHUAvsA6NGTMvI84gCYMALXTND6QIMG1wFx1wF41wHXTPgAcIAQBKoCFLHIywVQBc8WUAP6AstpItAhzzEh10mghAm5mDNwAcsAWM8WlzBxAcsAEsziyQH7AAAE0jA=";

fn traverse_cell_ordinary(c: &mut Criterion) {
    let cell = Boc::decode_base64(BOC).unwrap();

    c.bench_function("traverse cell ordinary", |b| {
        b.iter(|| {
            let mut visitor = Visitor::default();
            black_box(visitor.add_cell(cell.as_ref()));
        })
    });
}

fn traverse_cell_storage_cell(c: &mut Criterion) {
    let cell = Boc::decode_base64(BOC).unwrap();
    let usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess);
    let cell = usage_tree.track(&cell);

    c.bench_function("traverse cell usage tree", |b| {
        b.iter(|| {
            let mut visitor = Visitor::default();
            black_box(visitor.add_cell(cell.as_ref()));
        })
    });
}

fn traverse_cell_storage_cell_with_capacity(c: &mut Criterion) {
    let cell = Boc::decode_base64(BOC).unwrap();
    let usage_tree = UsageTree::with_mode_and_capacity(UsageTreeMode::OnDataAccess, 100);
    let cell = usage_tree.track(&cell);

    c.bench_function("traverse cell usage tree with capacity", |b| {
        b.iter(|| {
            let mut visitor = Visitor::default();
            black_box(visitor.add_cell(cell.as_ref()));
        })
    });
}

#[derive(Default)]
struct Visitor<'a> {
    visited: ahash::HashSet<&'a HashBytes>,
    stack: Vec<RefsIter<'a>>,
}

impl<'a> Visitor<'a> {
    fn add_cell(&mut self, cell: &'a DynCell) -> bool {
        if !self.visited.insert(cell.repr_hash()) {
            return true;
        }

        self.stack.clear();
        self.stack.push(cell.references());
        self.reduce_stack()
    }

    fn reduce_stack(&mut self) -> bool {
        'outer: while let Some(item) = self.stack.last_mut() {
            for cell in item.by_ref() {
                if !self.visited.insert(cell.repr_hash()) {
                    continue;
                }

                let mut slice = cell.as_slice().unwrap();
                slice.load_bit().ok();
                slice.load_u32().ok();
                slice.load_small_uint(5).ok();
                slice.load_reference().ok();

                let next = cell.references();
                if next.peek().is_some() {
                    self.stack.push(next);
                    continue 'outer;
                }
            }

            self.stack.pop();
        }

        true
    }
}

criterion_group!(
    benches,
    traverse_cell_ordinary,
    traverse_cell_storage_cell,
    traverse_cell_storage_cell_with_capacity
);
criterion_main!(benches);
