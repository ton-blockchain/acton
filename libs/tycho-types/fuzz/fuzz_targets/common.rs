use arbitrary::Arbitrary;
use tycho_types::dict::{self, AugDictExtra, StoreDictKey};
use tycho_types::prelude::*;

#[derive(Debug)]
pub struct AugInput<const MAX_N: usize, K, A, V> {
    pub initial_items: Vec<(K, A, V)>,
    pub operations: Vec<AugOperation<K, A, V>>,
}

impl<const MAX_N: usize, K, A, V> AugInput<MAX_N, K, A, V>
where
    K: StoreDictKey + Ord + Copy,
    for<'a> A: AugDictExtra + Store + Load<'a>,
    V: Store,
{
    pub fn compare_manual_vs_batched(self) {
        let ctx = Cell::empty_context();

        let initial =
            dict::build_aug_dict_from_sorted_iter(self.initial_items, A::comp_add, ctx).unwrap();

        // Build manually
        let mut target = initial.clone();
        for op in &self.operations {
            let mut b = CellDataBuilder::new();
            match op {
                AugOperation::Remove { key } => {
                    key.store_into_data(&mut b).unwrap();
                    dict::aug_dict_remove_owned(
                        &mut target,
                        &mut b.as_data_slice(),
                        K::BITS,
                        false,
                        A::comp_add,
                        ctx,
                    )
                    .unwrap();
                }
                AugOperation::Insert { key, extra, value } => {
                    key.store_into_data(&mut b).unwrap();
                    dict::aug_dict_insert(
                        &mut target,
                        &mut b.as_data_slice(),
                        K::BITS,
                        extra,
                        value,
                        dict::SetMode::Set,
                        A::comp_add,
                        ctx,
                    )
                    .unwrap();
                }
            }
        }

        // Build batched
        let mut result = initial.clone();
        dict::aug_dict_modify_from_sorted_iter(
            &mut result,
            self.operations,
            |op| *op.key(),
            |op| {
                Ok(match op {
                    AugOperation::Remove { .. } => None,
                    AugOperation::Insert { extra, value, .. } => Some((extra, value)),
                })
            },
            A::comp_add,
            ctx,
        )
        .unwrap();

        //
        assert_eq!(result, target);
    }
}

impl<'a, const MAX_N: usize, K, A, V> Arbitrary<'a> for AugInput<MAX_N, K, A, V>
where
    K: Arbitrary<'a> + PartialEq + Ord,
    A: Arbitrary<'a>,
    V: Arbitrary<'a>,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let item_count = u.int_in_range(0..=MAX_N)?;
        let mut initial_items = Vec::<(K, A, V)>::with_capacity(item_count);
        for _ in 0..item_count {
            initial_items.push(u.arbitrary()?);
        }
        initial_items.sort_unstable_by(|(a, ..), (b, ..)| a.cmp(b));
        initial_items.dedup_by(|(a, ..), (b, ..)| (*a).eq(b));

        let operation_count = u.int_in_range(0..=MAX_N)?;
        let mut operations = Vec::<AugOperation<K, A, V>>::with_capacity(operation_count);
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
pub enum AugOperation<K, A, V> {
    Remove { key: K },
    Insert { key: K, extra: A, value: V },
}

impl<K, A, V> AugOperation<K, A, V> {
    pub fn key(&self) -> &K {
        match self {
            Self::Remove { key } => key,
            Self::Insert { key, .. } => key,
        }
    }
}
