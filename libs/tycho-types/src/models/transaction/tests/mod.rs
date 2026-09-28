use super::*;
use crate::prelude::{Boc, Cell, CellBuilder};

fn check_tx(boc: &[u8]) -> Cell {
    let boc = Boc::decode(boc).unwrap();
    let tx = boc.parse::<Transaction>().unwrap();

    #[cfg(feature = "serde")]
    {
        let json = serde_json::to_string_pretty(&tx).unwrap();
        println!("{json}");

        let parsed = serde_json::from_str::<Transaction>(&json).unwrap();
        let parsed_boc = CellBuilder::build_from(&parsed).unwrap();
        assert_eq!(boc.repr_hash(), parsed_boc.repr_hash());
    }

    println!("tx: {tx:#?}");

    let in_msg = tx.load_in_msg().unwrap();
    println!("In message: {in_msg:?}");

    for (i, entry) in tx.out_msgs.iter().enumerate() {
        let (number, cell) = entry.unwrap();
        let message = cell.parse::<Message>().unwrap();
        assert_eq!(number, i as u16);
        println!("Out message: {i}, message: {message:?}");
    }
    assert_eq!(
        tx.out_msg_count.into_inner() as usize,
        tx.out_msgs.raw_values().count()
    );

    let mut out_msg_count = 0;
    for msg in tx.iter_out_msgs() {
        msg.unwrap();
        out_msg_count += 1;
    }
    assert_eq!(out_msg_count, tx.out_msg_count);

    let info = tx.load_info().unwrap();
    println!("info: {info:#?}");
    assert_eq!(tx.info, CellBuilder::build_from(info).unwrap());

    let serialized = CellBuilder::build_from(tx).unwrap();
    assert_eq!(serialized, boc);
    serialized
}

#[test]
fn ordinary_tx_without_outgoing() {
    check_tx(include_bytes!("ordinary_tx_without_outgoing.boc"));
}

#[test]
fn ordinary_tx_with_outgoing() {
    check_tx(include_bytes!("ordinary_tx_with_outgoing.boc"));
}

#[test]
fn ordinary_tx_with_external() {
    check_tx(include_bytes!("ordinary_tx_with_external.boc"));
}

#[test]
fn ordinary_tx_recursive() {
    check_tx(include_bytes!("ordinary_tx_recursive.boc"));
}

#[test]
fn ordinary_bounce() {
    // Ok, no state
    check_tx(include_bytes!("ordinary_tx_bounce_no_state.boc"));

    // NoFunds
    check_tx(include_bytes!("ordinary_tx_bounce_no_funds.boc"));
}

#[test]
fn tick_tx() {
    // Tick
    check_tx(include_bytes!("tick_tx.boc"));
}

#[test]
fn tock_tx() {
    check_tx(include_bytes!("tock_tx.boc"));
}
