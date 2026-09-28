use super::*;
use crate::models::Block;
use crate::prelude::Boc;

fn check_master_state(cell: Cell) {
    let data = cell.parse::<ShardStateUnsplit>().unwrap();
    println!("data: {data:#?}");
    assert_eq!(CellBuilder::build_from(&data).unwrap(), cell);

    let shard_accounts = data.load_accounts().unwrap();
    assert_eq!(
        CellBuilder::build_from(&shard_accounts).unwrap(),
        data.accounts
    );

    for entry in shard_accounts.iter() {
        let (id, depth_balance, shard_state) = entry.unwrap();
        let account = shard_state.load_account().unwrap();
        println!("{id}: {depth_balance:?} {account:#?}");
    }

    for (i, entry) in data.libraries.iter().enumerate() {
        let (hash, descr) = entry.unwrap();
        println!("lib#{i} hash={hash}");
        for entry in descr.publishers.keys() {
            let address = entry.unwrap();
            println!("lib#{i} publisher={address}")
        }
    }

    let _elector = shard_accounts.get([0x33; 32]).unwrap().unwrap();
    assert!(shard_accounts.contains_key([0x55; 32]).unwrap());

    let custom = data.load_custom().unwrap().unwrap();
    println!("custom: {custom:#?}");
    assert_eq!(
        CellBuilder::build_from(&custom).unwrap(),
        data.custom.unwrap()
    );
}

#[test]
fn prod_zerostate() {
    const BOC: &[u8] = include_bytes!("zerostate.boc");
    check_master_state(Boc::decode(BOC).unwrap());
}

#[test]
fn new_zerostate() {
    const BOC: &[u8] = include_bytes!("new_zerostate.boc");
    let zerostate = Boc::decode(BOC).unwrap();
    check_master_state(zerostate.clone());

    let block = Boc::decode(include_bytes!("first_block.boc")).unwrap();
    let block = block.parse::<Block>().unwrap();
    let state_update = block.state_update.load().unwrap();

    let new_state = state_update.apply(&zerostate).unwrap();
    check_master_state(new_state);
}
