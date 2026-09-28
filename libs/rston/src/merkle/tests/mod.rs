use super::*;
use crate::cell::{CellTreeStats, EMPTY_CELL_HASH};
use crate::error::Error;
use crate::prelude::*;

#[test]
fn correct_proof_store_load() {
    let proof = MerkleProof::default();
    let cell = CellBuilder::build_from(&proof).unwrap();

    let parsed = cell.as_ref().parse_exotic::<MerkleProof>().unwrap();
    assert_eq!(parsed, proof);

    let parsed = cell.as_ref().parse_exotic::<MerkleProofRef<'_>>().unwrap();
    assert_eq!(parsed, proof);
}

#[test]
fn test_proof() {
    let root = Boc::decode(include_bytes!("simple_proof.boc")).unwrap();
    let target_hash = root.as_ref().reference(1).unwrap().repr_hash();

    let merkle_proof = MerkleProof::create_for_cell(root.as_ref(), target_hash)
        .build()
        .unwrap();

    let virtual_root = merkle_proof.cell.as_ref().virtualize();
    println!("{}", virtual_root.display_tree());

    assert_eq!(root.as_ref().repr_hash(), virtual_root.repr_hash());
    assert_eq!(root.as_ref().repr_depth(), virtual_root.repr_depth());
}

#[test]
#[cfg_attr(miri, ignore)] // takes too long to execute on miri
fn create_proof_for_deep_cell() {
    let mut cell = Cell::empty_cell();
    for i in 0..65000 {
        let mut builder = CellBuilder::new();
        builder.store_u32(i).unwrap();
        builder.store_reference(cell).unwrap();
        cell = builder.build().unwrap();
    }

    let stats = cell.compute_unique_stats(1 << 22).unwrap();
    assert_eq!(
        stats,
        CellTreeStats {
            bit_count: 65000 * 32,
            cell_count: 65001
        }
    );

    {
        let encoded = Boc::encode_base64(cell.as_ref());
        let decoded = Boc::decode_base64(encoded).unwrap();
        assert_eq!(decoded.as_ref(), cell.as_ref());
    }

    let cell = MerkleProof::create_for_cell(cell.as_ref(), EMPTY_CELL_HASH)
        .build()
        .unwrap();

    let encoded = BocRepr::encode_base64(&cell).unwrap();
    let decoded = Boc::decode_base64(encoded)
        .unwrap()
        .as_ref()
        .parse_exotic::<MerkleProof>()
        .unwrap();

    assert_eq!(cell, decoded);
}

#[test]
fn create_proof_for_dict() {
    // Create dict with keys 0..10
    let mut dict = Dict::<u32, u32>::new();

    for i in 0..10 {
        dict.add(i, i * 10).unwrap();
    }

    // Create a usage tree for accessing an element with keys 0 and 9
    let usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess);
    let tracked_cell = usage_tree.track(&CellBuilder::build_from(dict).unwrap());
    let tracked_dict = tracked_cell.as_ref().parse::<Dict<u32, u32>>().unwrap();
    tracked_dict.get(0).unwrap().unwrap();
    tracked_dict.get(9).unwrap().unwrap();

    // Create proof from the usage tree
    let merkle_proof = MerkleProof::create(tracked_cell.as_ref(), usage_tree)
        .build()
        .unwrap();

    // Try to read some keys
    let dict = merkle_proof.cell.as_ref().virtualize();
    let dict = dict.parse::<Dict<u32, u32>>().unwrap();
    dict.get(0).unwrap().unwrap();
    dict.get(9).unwrap().unwrap();

    assert!(matches!(dict.get(5), Err(Error::UnexpectedExoticCell)));
}

#[test]
fn proof_with_subtree() -> anyhow::Result<()> {
    let mut dict = Dict::<u32, u32>::new();
    for i in 0..10 {
        dict.add(i, i * 10)?;
    }
    let dict = CellBuilder::build_from(dict)?;

    let some_other_cell = {
        let mut builder = CellBuilder::new();
        builder.store_u128(123123)?;
        builder.store_reference(Cell::empty_cell())?;
        builder.store_reference(Cell::empty_cell())?;
        builder.build()?
    };

    let root_cell = {
        let mut builder = CellBuilder::new();
        builder.store_u128(321321)?;
        builder.store_reference(some_other_cell)?;
        builder.store_reference(dict.clone())?;
        builder.build()?
    };

    let mut usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess).with_subtrees();
    let root_cell = usage_tree.track(&root_cell);

    {
        let mut root_cell = root_cell.as_ref().as_slice()?;
        root_cell.load_u32().unwrap();

        assert!(usage_tree.add_subtree(dict.as_ref()));
    }

    let proof = MerkleProof::create(root_cell.as_ref(), usage_tree).build()?;
    let mut virtual_cell = proof.cell.as_ref().virtualize().as_slice()?;

    assert_eq!(virtual_cell.load_u128(), Ok(321321));

    let first_ref = virtual_cell.load_reference()?;
    assert_eq!(first_ref.cell_type(), CellType::PrunedBranch);

    let second_ref = virtual_cell.load_reference()?;
    assert_eq!(second_ref.cell_type(), CellType::Ordinary);
    assert!(second_ref.descriptor().level_mask().is_empty());

    let dict = second_ref.parse::<Dict<u32, u32>>()?;
    for (i, entry) in dict.iter().enumerate() {
        let (key, value) = entry?;
        assert_eq!(i, key as usize);
        assert_eq!(key * 10, value);
    }
    assert_eq!(dict.values().count(), 10);

    Ok(())
}

fn create_tree(depth: u32, num: u32) -> CellBuilder {
    let mut builder = CellBuilder::new();
    builder.store_u32(num).unwrap();
    if depth > 0 {
        let cell = create_tree(depth - 1, num);
        builder.store_reference(cell.build().unwrap()).unwrap();
    }
    builder
}

#[test]
fn test_merkle_update() {
    let build_cell = |num: u32| {
        let mut builder = CellBuilder::new();
        builder.store_u32(num).unwrap();
        builder.build().unwrap()
    };

    // Create two trees with overlapping structure
    let mut old_tree = create_tree(5, 1);
    old_tree.store_reference(build_cell(1)).unwrap();
    let old_tree = old_tree.build().unwrap();
    let usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess);
    let old_tree = usage_tree.track(&old_tree);

    let mut new_tree = create_tree(5, 1);
    new_tree.store_reference(build_cell(2)).unwrap();
    let new_tree = new_tree.build().unwrap();

    // Create a set of visited cells for the old tree
    let mut old_cells = HashSet::new();
    let mut stack = vec![old_tree.as_ref()];
    while let Some(cell) = stack.pop() {
        // trigger usage tracking)
        let _ = cell.data();
        old_cells.insert(*cell.repr_hash());
        for child in cell.references() {
            stack.push(child);
        }
    }

    // Create the Merkle update using HashSet
    let merkle_update = MerkleUpdate::create(old_tree.as_ref(), new_tree.as_ref(), old_cells)
        .build()
        .unwrap();

    // Create the Merkle update using UsageTree
    let merkle_update_with_usage_tree =
        MerkleUpdate::create(old_tree.as_ref(), new_tree.as_ref(), usage_tree)
            .build()
            .unwrap();

    // Print sizes
    println!(
        "Old tree size: {} bytes",
        BocRepr::encode(&old_tree).unwrap().len()
    );
    println!(
        "New tree size: {} bytes",
        BocRepr::encode(&new_tree).unwrap().len()
    );
    println!(
        "Update size (HashSet): {} bytes",
        BocRepr::encode(&merkle_update).unwrap().len()
    );
    println!(
        "Update size (UsageTree): {} bytes",
        BocRepr::encode(&merkle_update_with_usage_tree)
            .unwrap()
            .len()
    );

    // Verify the Merkle updates
    assert_eq!(merkle_update.old_hash, *old_tree.repr_hash());
    assert_eq!(merkle_update.new_hash, *new_tree.repr_hash());
    assert_eq!(
        merkle_update_with_usage_tree.old_hash,
        *old_tree.repr_hash()
    );
    assert_eq!(
        merkle_update_with_usage_tree.new_hash,
        *new_tree.repr_hash()
    );

    // Apply the Merkle updates to the old tree
    let result_hashset = merkle_update.apply(&old_tree).unwrap();
    let result_usage_tree = merkle_update_with_usage_tree.apply(&old_tree).unwrap();

    // Verify that the results match the new tree
    assert_eq!(result_hashset.as_ref(), new_tree.as_ref());
    assert_eq!(result_usage_tree.as_ref(), new_tree.as_ref());

    // Check that both Merkle updates are the same
    assert_eq!(merkle_update, merkle_update_with_usage_tree);
}
