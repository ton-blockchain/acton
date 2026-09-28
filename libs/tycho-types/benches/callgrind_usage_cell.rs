use std::collections::HashSet;
use std::hint::black_box;

use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use tycho_types::cell::RefsIter;
use tycho_types::prelude::*;

const BOC: &str = "te6ccgECCAEAAWQAAnPP9noJKCEBL3oZerOiIcNghuL96V3wIcuYOWQdvNC+2fqCEIJDQAAAAAAAAAAAAAAAAZa8xB6QABNAAgEAUO3QlUyMI4dEepUMw3Ou6oSqq8+1lyHkjOGFK6DAn6TXAAAAAAAAAAABFP8A9KQT9LzyyAsDAgEgBwQC5vJx1wEBwADyeoMI1xjtRNCDB9cB1ws/yPgozxYjzxbJ+QADcdcBAcMAmoMH1wFRE7ry4GTegEDXAYAg1wGAINcBVBZ1+RDyqPgju/J5Zr74I4EHCKCBA+ioUiC8sfJ0AiCCEEzuZGy64w8ByMv/yz/J7VQGBQA+ghAWnj4Ruo4R+AACkyDXSpd41wHUAvsA6NGTMvI84gCYMALXTND6QIMG1wFx1wF41wHXTPgAcIAQBKoCFLHIywVQBc8WUAP6AstpItAhzzEh10mghAm5mDNwAcsAWM8WlzBxAcsAEsziyQH7AAAE0jA=";

#[library_benchmark]
fn traverse_cell_ordinary() {
    let cell = Boc::decode_base64(BOC).unwrap();

    let mut visitor = Visitor::new();
    black_box(visitor.add_cell(cell.as_ref()));
}

#[library_benchmark]
fn traverse_cell_storage_cell() {
    let cell = Boc::decode_base64(BOC).unwrap();
    let usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess);
    let cell = usage_tree.track(&cell);

    let mut visitor = Visitor::new();
    black_box(visitor.add_cell(cell.as_ref()));
}

#[library_benchmark]
fn traverse_cell_storage_cell_with_capacity() {
    let cell = Boc::decode_base64(BOC).unwrap();
    let usage_tree = UsageTree::with_mode_and_capacity(UsageTreeMode::OnDataAccess, 100);
    let cell = usage_tree.track(&cell);

    let mut visitor = Visitor::new();
    black_box(visitor.add_cell(cell.as_ref()));
}

struct Visitor<'a> {
    visited: ahash::HashSet<&'a HashBytes>,
    stack: Vec<RefsIter<'a>>,
}

impl<'a> Visitor<'a> {
    fn new() -> Self {
        Self {
            visited: HashSet::with_hasher(ahash::RandomState::with_seed(0)),
            stack: Vec::new(),
        }
    }

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

library_benchmark_group!(
    name = traverse_cell;
    benchmarks = traverse_cell_ordinary, traverse_cell_storage_cell, traverse_cell_storage_cell_with_capacity
);

main!(library_benchmark_groups = traverse_cell);
