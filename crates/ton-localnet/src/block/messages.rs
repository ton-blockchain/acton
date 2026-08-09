use crate::block::types::BlockTransaction;
use anyhow::Context;
use std::collections::BTreeMap;
use tycho_types::cell::Lazy;
use tycho_types::models::block::{InMsgDescr, OutMsgDescr};
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::models::message::{ImportFees, InMsg, InMsgExternal, OutMsg, OwnedMessage};
use tycho_types::models::transaction::Transaction;
use tycho_types::prelude::HashBytes;

/// Builds the inbound message descriptor for messages we can represent exactly.
///
/// External inbound messages have a direct TL-B descriptor that links the
/// message cell to the transaction cell, so localnet records them here. Internal
/// messages require routing envelopes and queue metadata that localnet does not
/// currently model; those transactions remain indexable through
/// `BlockExtra.account_blocks`.
pub(super) fn build_in_msg_descr(transactions: &[BlockTransaction]) -> anyhow::Result<InMsgDescr> {
    let mut entries = BTreeMap::new();

    for tx in transactions {
        let Some(msg_hash) = &tx.tx_meta.in_msg_hash else {
            continue;
        };
        let Some(in_msg_cell) = tx
            .tx_cell
            .parse::<Transaction>()
            .context("Failed to parse transaction for inbound message descriptor")?
            .in_msg
        else {
            continue;
        };

        let owned = in_msg_cell
            .parse::<OwnedMessage>()
            .context("Failed to parse inbound message for block descriptor")?;
        if !matches!(owned.info, tycho_types::models::MsgInfo::ExtIn(_)) {
            continue;
        }

        let tx_ref = Lazy::<Transaction>::from_raw(tx.tx_cell.clone())
            .context("Failed to wrap inbound transaction cell")?;
        let msg_ref = Lazy::<OwnedMessage>::from_raw(in_msg_cell)
            .context("Failed to wrap inbound message cell")?;
        let descriptor = InMsg::External(InMsgExternal {
            in_msg: msg_ref,
            transaction: tx_ref,
        });

        entries.insert(HashBytes(msg_hash.0), (ImportFees::default(), descriptor));
    }

    InMsgDescr::try_from_btree(&entries).context("Failed to build inbound message descriptor")
}

/// Builds the outbound message descriptor.
///
/// Localnet stores outgoing message cells and can schedule local internal
/// cascades. It does not store the outbound queue envelopes required by
/// `OutMsgDescr::New`/`Immediate`, so the descriptor is empty and transaction
/// discovery remains available through `AccountBlocks`.
pub(super) fn build_out_msg_descr() -> anyhow::Result<OutMsgDescr> {
    OutMsgDescr::try_from_btree(&BTreeMap::<HashBytes, (CurrencyCollection, OutMsg)>::new())
        .context("Failed to build outbound message descriptor")
}
