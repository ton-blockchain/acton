#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use tycho_types::cell::Lazy;
use tycho_types::dict::AugDictExtra;
use tycho_types::error::Error;
use tycho_types::prelude::*;

use self::common::AugInput;

mod common;

fuzz_target!(|input: AugInput<1000, u32, AugWithCell, u64>| input.compare_manual_vs_batched());

#[derive(Default, Debug, Store, Load, Clone, Arbitrary)]
struct AugWithCell(u32, Option<Lazy<u32>>);

impl AugDictExtra for AugWithCell {
    fn comp_add(
        left: &mut CellSlice,
        right: &mut CellSlice,
        b: &mut CellBuilder,
        ctx: &dyn CellContext,
    ) -> Result<(), Error> {
        let left = Self::load_from(left)?;
        let right = Self::load_from(right)?;

        let sum = left.0.saturating_add(right.0);
        let cell = match (left.1, right.1) {
            (None, None) => Some(Lazy::new(&!sum)?),
            (Some(cell), None) | (None, Some(cell)) => {
                let cell = cell.load()?;
                Some(Lazy::new(&(cell & sum))?)
            }
            (Some(left), Some(right)) => {
                let left = left.load()?;
                let right = right.load()?;
                let cell_sum = left.saturating_add(right);
                (sum & 1 != cell_sum & 1)
                    .then(|| Lazy::new(&cell_sum))
                    .transpose()?
            }
        };

        AugWithCell(sum, cell).store_into(b, ctx)
    }
}
