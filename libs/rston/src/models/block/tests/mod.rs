use std::collections::HashMap;

use super::*;
use crate::prelude::*;

fn serialize_any<T: Store>(data: T) -> Cell {
    CellBuilder::build_from(data).unwrap()
}

fn check_block(boc: &[u8], expected_shards: Option<Vec<ShardIdent>>) -> Cell {
    let boc = Boc::decode(boc).unwrap();
    let block = boc.parse::<Block>().unwrap();
    println!("block: {block:#?}");

    let info = block.load_info().unwrap();

    #[cfg(feature = "serde")]
    {
        // JSON represents known capability names; the binary round-trip below retains all bits.
        let mut info = info.clone();
        info.gen_software.capabilities = info.gen_software.capabilities.iter().collect();

        let json = serde_json::to_string_pretty(&info).unwrap();
        let parsed: BlockInfo = serde_json::from_str(&json).unwrap();
        let parsed_boc = CellBuilder::build_from(&parsed).unwrap();
        assert_eq!(serialize_any(info), parsed_boc);
    }

    println!("info: {info:#?}");
    let prev_ref = info.load_prev_ref().unwrap();
    println!("prev_ref: {prev_ref:#?}");
    assert_eq!(serialize_any(info), block.info);

    let value_flow = block.load_value_flow().unwrap();
    println!("value_flow: {value_flow:#?}");
    assert_eq!(serialize_any(value_flow), block.value_flow);

    let state_update = block.load_state_update().unwrap();
    println!("state_update: {state_update:#?}");
    assert_eq!(serialize_any(state_update), block.state_update);

    let extra = block.load_extra().unwrap();
    println!("extra: {extra:#?}");
    let account_blocks = extra.account_blocks.load().unwrap();
    println!("account_blocks: {account_blocks:#?}");

    for entry in account_blocks.iter() {
        let (account, _, account_block) = entry.unwrap();
        assert_eq!(account, account_block.account);

        for entry in account_block.transactions.iter() {
            let (_lt, _, cell) = entry.unwrap();
            let tx = cell.load().unwrap();
            assert_eq!(account, tx.account);
        }
    }
    assert_eq!(serialize_any(account_blocks), extra.account_blocks);

    let custom = extra.load_custom().unwrap();
    assert_eq!(expected_shards.is_some(), custom.is_some());
    if let Some(custom) = &custom {
        println!("custom: {custom:#?}");

        let expected_shards = expected_shards.unwrap();

        let shards = custom.shards.get_workchain_shards(0).unwrap().unwrap();
        let mut shard_ids = Vec::new();
        for entry in shards.raw_iter() {
            let (shard, _value) = entry.unwrap();
            println!("shard: {shard:?}");
            shard_ids.push(shard);
        }
        assert_eq!(expected_shards, shard_ids);

        for entry in custom.shards.iter() {
            let (shard, value) = entry.unwrap();
            println!("shard {shard:?}: {value:#?}");
        }
        println!(
            "origin encoded {}",
            BocRepr::encode_base64(&custom.shards).unwrap()
        );
        let shards = custom
            .shards
            .iter()
            .map(|x| x.unwrap())
            .collect::<HashMap<_, _>>();
        let encoded = ShardHashes::from_shards(&shards).unwrap();
        println!("encoded {}", BocRepr::encode_base64(&encoded).unwrap());

        let parsed = encoded.iter().collect::<Result<Vec<_>, _>>().unwrap();
        for (a, b) in shard_ids.iter().zip(parsed.iter()) {
            assert_eq!(*a, b.0);
        }

        for item in custom.shards.latest_blocks() {
            let block_id = item.unwrap();
            println!("block_id: {block_id}");
        }

        assert_eq!(serialize_any(custom), extra.custom.as_ref().unwrap())
    }

    assert_eq!(serialize_any(extra), block.extra);

    let serialized = serialize_any(block);
    assert_eq!(serialized, boc);

    boc
}

#[test]
fn masterchain_block() {
    check_block(
        include_bytes!("mc_simple_block.boc"),
        Some(vec![ShardIdent::new(0, ShardIdent::PREFIX_FULL).unwrap()]),
    );
}

#[test]
fn masterchain_key_block() {
    check_block(
        include_bytes!("mc_key_block.boc"),
        Some(vec![ShardIdent::new(0, ShardIdent::PREFIX_FULL).unwrap()]),
    );
}

#[test]
fn masterchain_block_with_shards() {
    check_block(
        include_bytes!("mc_block_with_shards.boc"),
        Some(vec![
            ShardIdent::new(0, 0x4000000000000000).unwrap(),
            ShardIdent::new(0, 0xa000000000000000).unwrap(),
            ShardIdent::new(0, 0xe000000000000000).unwrap(),
        ]),
    );
}

#[test]
fn shard_block_empty() {
    check_block(include_bytes!("empty_shard_block.boc"), None);
}

#[test]
fn shard_block_with_messages() {
    check_block(include_bytes!("simple_shard_block.boc"), None);
}

#[test]
fn parse_block_id() {
    let block_id = BlockId {
        shard: ShardIdent::MASTERCHAIN,
        seqno: 123321,
        root_hash: HashBytes([123; 32]),
        file_hash: HashBytes([234; 32]),
    };

    let s = block_id.to_string();
    println!("S: {s}");
    assert_eq!(s.parse::<BlockId>().unwrap(), block_id);
}

#[test]
fn shard_ident_operations() {
    let shard = ShardIdent::BASECHAIN;
    assert!(shard.is_left_child());
    assert_eq!(shard.prefix_len(), 0);
    assert!(shard.merge().is_none());
    assert_eq!(shard.opposite(), None);

    let (left, right) = shard.split().unwrap();
    assert!(left.is_left_child() && !left.is_right_child());
    assert!(right.is_right_child() && !right.is_left_child());

    assert_eq!(left.opposite(), Some(right));
    assert_eq!(right.opposite(), Some(left));

    assert!(!left.is_child_of(&ShardIdent::MASTERCHAIN));
    assert!(!right.is_child_of(&ShardIdent::MASTERCHAIN));

    assert!(left.is_child_of(&shard));
    assert!(!shard.is_child_of(&left));
    assert!(right.is_child_of(&shard));
    assert!(!shard.is_child_of(&right));

    assert!(!left.is_parent_of(&right));
    assert!(!right.is_parent_of(&left));
    assert!(!left.is_ancestor_of(&right));
    assert!(!right.is_ancestor_of(&left));

    assert!(shard.is_parent_of(&left));
    assert!(!left.is_parent_of(&shard));
    assert!(shard.is_parent_of(&right));
    assert!(!right.is_parent_of(&shard));

    assert!(shard.is_ancestor_of(&left));
    assert!(!left.is_ancestor_of(&shard));
    assert!(shard.is_ancestor_of(&right));
    assert!(!right.is_ancestor_of(&shard));

    assert_eq!(left.merge().unwrap(), shard);
    assert_eq!(right.merge().unwrap(), shard);

    let children = {
        let (ll, lr) = left.split().unwrap();
        let (rl, rr) = right.split().unwrap();

        assert_eq!(ll.opposite(), Some(lr));
        assert_eq!(lr.opposite(), Some(ll));
        assert_eq!(rl.opposite(), Some(rr));
        assert_eq!(rr.opposite(), Some(rl));

        assert!(ll.intersects(&left));
        assert!(left.intersects(&ll));
        assert!(lr.intersects(&left));
        assert!(left.intersects(&lr));

        assert!(rl.intersects(&right));
        assert!(right.intersects(&rl));
        assert!(rr.intersects(&right));
        assert!(right.intersects(&rr));

        assert!(!rl.intersects(&left));
        assert!(!left.intersects(&rl));
        assert!(!rr.intersects(&left));
        assert!(!left.intersects(&rr));

        assert!(!ll.intersects(&right));
        assert!(!right.intersects(&ll));
        assert!(!lr.intersects(&right));
        assert!(!right.intersects(&lr));

        [ll, lr, rl, rr]
    };

    for child in children {
        assert_eq!(child.prefix_len(), 2);
        assert!(child.is_left_child() != child.is_right_child());
        assert!(shard.is_ancestor_of(&child));

        assert!(!child.is_ancestor_of(&shard));
        assert!(!child.is_parent_of(&shard));
        assert!(!shard.is_parent_of(&child));

        assert!(shard.intersects(&child));
        assert!(child.intersects(&shard));

        assert!(!child.intersects(&ShardIdent::MASTERCHAIN));
    }
}

#[test]
fn shard_ident_max_split() {
    let mut shards = vec![];

    let mut shard = ShardIdent::BASECHAIN;
    shards.push(shard);

    for i in 0..ShardIdent::MAX_SPLIT_DEPTH {
        assert_eq!(shard.prefix_len(), i as u16);

        let (left, _) = shard.split().unwrap();
        shard = left;
        shards.push(shard);
    }
    assert!(shard.split().is_none());

    let mut rev_shard = ShardIdent::new(0, 1 << (63 - ShardIdent::MAX_SPLIT_DEPTH)).unwrap();
    while let Some(shard) = shards.pop() {
        assert_eq!(rev_shard, shard);

        if !shards.is_empty() {
            rev_shard = shard.merge().unwrap();
        }
    }
    assert!(rev_shard.merge().is_none());
}

#[test]
fn shard_ident_store_load() {
    fn check_store_load(shard: ShardIdent) {
        let mut builder = CellBuilder::new();
        shard
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let cell = builder.build().unwrap();
        assert_eq!(cell.bit_len(), ShardIdent::BITS);

        let parsed_shard = cell.parse::<ShardIdent>().unwrap();
        assert_eq!(shard, parsed_shard);
    }

    let mut shard = ShardIdent::BASECHAIN;
    for _ in 0..ShardIdent::MAX_SPLIT_DEPTH {
        let (left, right) = shard.split().unwrap();
        check_store_load(left);
        check_store_load(right);
        shard = left;
    }
    assert!(shard.split().is_none());

    // Try loading from invalid cells
    fn check_invalid<F: FnOnce(&mut CellBuilder) -> Result<(), Error>>(f: F) {
        let mut builder = CellBuilder::new();
        f(&mut builder).unwrap();
        let cell = builder.build().unwrap();
        assert!(cell.parse::<ShardIdent>().is_err())
    }

    check_invalid(|b| b.store_bit_one());
    check_invalid(|b| b.store_u8(0));
    check_invalid(|b| {
        b.store_u8(ShardIdent::MAX_SPLIT_DEPTH + 1)?;
        b.store_u32(0)?;
        b.store_u64(0)
    });
}

#[test]
fn proof_for_masterchain_block() {
    let boc = Boc::decode(include_bytes!("mc_block_proof.boc")).unwrap();
    let proof = boc.parse::<BlockProof>().unwrap();

    assert_eq!(proof.proof_for.shard, ShardIdent::MASTERCHAIN);
    assert_eq!(proof.proof_for.seqno, 13121100);
    assert!(proof.signatures.is_some());

    assert_eq!(serialize_any(proof).as_ref(), boc.as_ref());
}

#[test]
fn proof_for_shardchain_block() {
    let boc = Boc::decode(include_bytes!("shard_block_proof.boc")).unwrap();
    let proof = boc.parse::<BlockProof>().unwrap();

    assert_eq!(
        proof.proof_for.shard,
        ShardIdent::new(0, 0xf800000000000000).unwrap()
    );
    assert_eq!(proof.proof_for.seqno, 19363091);
    assert!(proof.signatures.is_none());

    assert_eq!(serialize_any(proof).as_ref(), boc.as_ref());
}
