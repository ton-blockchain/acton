use crate::cell::*;
use crate::error::Error;

/// Creates a pruned branch cell with the specified merkle depth.
pub fn make_pruned_branch(
    cell: &DynCell,
    merkle_depth: u8,
    context: &dyn CellContext,
) -> Result<Cell, Error> {
    let descriptor = cell.descriptor();
    let mut cell_level_mask = descriptor.level_mask();

    let mut builder = CellBuilder::new();

    let new_level = 1 << merkle_depth;
    let level_mask = LevelMask::new(cell_level_mask.to_byte() | new_level);

    builder.set_exotic(true);

    _ = builder.store_u16(u16::from_be_bytes([
        CellType::PrunedBranch.to_byte(),
        level_mask.to_byte(),
    ]));

    // Only write levels lower than the new level.
    cell_level_mask = LevelMask::new(cell_level_mask.to_byte() & (new_level - 1));

    for level in cell_level_mask {
        _ = builder.store_u256(cell.hash(level));
    }

    for level in cell_level_mask {
        _ = builder.store_u16(cell.depth(level));
    }

    builder.build_ext(context)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::boc::Boc;
    use crate::merkle::MerkleProof;

    #[test]
    fn correct_pruned_branch() {
        let cell = {
            let mut builder = CellBuilder::new();
            builder.store_u128(0xdeafbeaf123123).unwrap();
            builder.store_reference(Cell::empty_cell()).unwrap();
            builder.build().unwrap()
        };

        let pruned_branch = make_pruned_branch(cell.as_ref(), 0, Cell::empty_context()).unwrap();
        assert_eq!(cell.repr_hash(), pruned_branch.hash(0));
        assert_eq!(cell.depth(0), pruned_branch.depth(0));

        let virtual_cell = cell.virtualize();
        assert_eq!(cell.repr_hash(), virtual_cell.repr_hash());
        assert_eq!(cell.depth(3), virtual_cell.depth(3));

        let virtual_pruned_branch =
            make_pruned_branch(virtual_cell, 0, Cell::empty_context()).unwrap();
        assert_eq!(pruned_branch.as_ref(), virtual_pruned_branch.as_ref());
    }

    #[test]
    fn partial_pruned() -> anyhow::Result<()> {
        let cell = CellBuilder::build_from((
            CellBuilder::build_from((0xaa_u8, Cell::empty_cell()))?,
            CellBuilder::build_from((0xbb_u8, Cell::empty_cell()))?,
        ))?;
        println!("Original: {}", Boc::encode_base64(&cell));

        // Prune left cell.
        let with_left_pruned = {
            let usage_tree = UsageTree::new(UsageTreeMode::OnLoad);
            let tracked = usage_tree.track(&cell);
            tracked.reference(1).unwrap().touch_recursive();

            MerkleProof::create(cell.as_ref(), usage_tree).build_raw_ext(Cell::empty_context())?
        };
        println!("Left pruned: {}", Boc::encode_base64(&with_left_pruned));

        // Full pruned.
        let pruned = make_pruned_branch(with_left_pruned.as_ref(), 0, Cell::empty_context())?;
        println!("Full pruned: {}", Boc::encode_base64(&pruned));

        Ok(())
    }
}
