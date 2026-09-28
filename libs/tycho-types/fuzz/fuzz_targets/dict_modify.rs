#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use tycho_types::dict;
use tycho_types::prelude::*;

fuzz_target!(|input: Input<1000, u32, u64>| compare_manual_vs_batched(input));

fn compare_manual_vs_batched<const MAX_N: usize, K, V>(input: Input<MAX_N, K, V>)
where
    K: StoreDictKey + Ord + Copy,
    V: Store,
{
    let ctx = Cell::empty_context();

    let initial = dict::build_dict_from_sorted_iter(input.initial_items, ctx).unwrap();

    // Build manually
    let mut target = initial.clone();
    for op in &input.operations {
        let mut b = CellDataBuilder::new();
        match op {
            Operation::Remove { key } => {
                key.store_into_data(&mut b).unwrap();
                dict::dict_remove_owned(&mut target, &mut b.as_data_slice(), K::BITS, false, ctx)
                    .unwrap();
            }
            Operation::Insert { key, value } => {
                key.store_into_data(&mut b).unwrap();
                dict::dict_insert(
                    &mut target,
                    &mut b.as_data_slice(),
                    K::BITS,
                    value,
                    dict::SetMode::Set,
                    ctx,
                )
                .unwrap();
            }
        }
    }

    // Build batched
    let mut result = initial.clone();
    dict::dict_modify_from_sorted_iter(
        &mut result,
        input.operations,
        |op| *op.key(),
        |op| {
            Ok(match op {
                Operation::Remove { .. } => None,
                Operation::Insert { value, .. } => Some(value),
            })
        },
        ctx,
    )
    .unwrap();

    //
    assert_eq!(result, target);
}

#[derive(Debug)]
struct Input<const MAX_N: usize, K, V> {
    initial_items: Vec<(K, V)>,
    operations: Vec<Operation<K, V>>,
}

impl<'a, const MAX_N: usize, K, V> Arbitrary<'a> for Input<MAX_N, K, V>
where
    K: Arbitrary<'a> + PartialEq + Ord,
    V: Arbitrary<'a>,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let item_count = u.int_in_range(0..=MAX_N)?;
        let mut initial_items = Vec::<(K, V)>::with_capacity(item_count);
        for _ in 0..item_count {
            initial_items.push(u.arbitrary()?);
        }
        initial_items.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
        initial_items.dedup_by(|(a, _), (b, _)| (*a).eq(b));

        let operation_count = u.int_in_range(0..=MAX_N)?;
        let mut operations = Vec::<Operation<K, V>>::with_capacity(operation_count);
        for _ in 0..operation_count {
            operations.push(u.arbitrary()?);
        }
        operations.sort_unstable_by(|a, b| a.key().cmp(b.key()));
        operations.dedup_by(|a, b| a.key().eq(b.key()));

        Ok(Self {
            initial_items,
            operations,
        })
    }
}

#[derive(Debug, Arbitrary)]
enum Operation<K, V> {
    Remove { key: K },
    Insert { key: K, value: V },
}

impl<K, V> Operation<K, V> {
    fn key(&self) -> &K {
        match self {
            Self::Remove { key } => key,
            Self::Insert { key, .. } => key,
        }
    }
}
