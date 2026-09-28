//! Response assembly shares transaction/message conversion with finalized SSE.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use rston::boc::Boc;
use rston::cell::{Cell, HashBytes, Lazy};
use rston::models::{AccountState, ShardAccount};
use serde::Serialize;
use toncenter::v3::responses as wire;

use super::Executed;
use crate::streaming::transaction::{convert_cell, currencies};

/// TON Center emulation result. Cell maps contain before/after code and data;
/// absent enrichment is omitted rather than represented as an empty analysis.
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct SimulationResponse {
    /// Applied masterchain checkpoint used for every state/config/library read
    mc_block_seqno: u32,
    trace: TraceNode,
    pub(super) transactions: BTreeMap<String, wire::Transaction>,
    /// Base64 seed passed to every transaction's native emulator
    rand_seed: String,
    /// Some internal messages were not executed because a resource limit was hit
    pub(super) is_incomplete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_cells: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_cells: Option<BTreeMap<String, String>>,
}

/// Links to completed transactions; external outputs have no child transaction.
#[derive(Serialize, utoipa::ToSchema)]
struct TraceNode {
    tx_hash: String,
    in_msg_hash: String,
    #[schema(no_recursion)]
    children: Vec<TraceNode>,
}

/// The VM did not produce a transaction. Aborted transactions are successful
/// emulation results instead and expose their exit codes in description.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub(crate) struct TransactionRejection {
    pub(super) error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) vm_exit_code: Option<i64>,
}

impl std::fmt::Display for TransactionRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.error)
    }
}

impl std::error::Error for TransactionRejection {}

/// Attaches local account states and causal message links to the shared wire
/// transactions. The completed-node list must contain its root at index zero.
pub(super) fn build(
    executed: Vec<Executed>,
    mc_seqno: u32,
    external_hash: HashBytes,
    seed: HashBytes,
    incomplete: bool,
    include_code_data: bool,
) -> Result<SimulationResponse> {
    let mut code_cells = include_code_data.then(BTreeMap::new);
    let mut data_cells = include_code_data.then(BTreeMap::new);
    let mut transactions: Vec<wire::Transaction> = Vec::with_capacity(executed.len());
    let mut children = vec![Vec::new(); executed.len()];

    for (index, node) in executed.into_iter().enumerate() {
        let result = node.result;
        let lazy = Lazy::from_raw(Boc::decode_base64(result.raw_transaction.as_ref())?)?;
        let mut tx = convert_cell(node.workchain, &lazy, &result.transaction)?;
        tx.mc_block_seqno = mc_seqno;
        tx.trace_external_hash = Some(STANDARD.encode(external_hash));
        tx.trace_id = Some(
            transactions
                .first()
                .map_or_else(|| tx.hash.clone(), |root| root.hash.clone()),
        );
        fill_account(
            &mut tx.account_state_before,
            &result.shard_account_before,
            &mut code_cells,
            &mut data_cells,
        )?;
        fill_account(
            &mut tx.account_state_after,
            &result.shard_account,
            &mut code_cells,
            &mut data_cells,
        )?;

        if let Some(parent) = node.parent {
            children[parent].push(index);
            let source = &mut transactions[parent];
            source.child_transactions.push(tx.hash.clone());
            let incoming = tx
                .in_msg
                .as_mut()
                .context("emulated transaction has no input message")?;
            incoming.out_msg_tx_hash = Some(source.hash.clone());
            let outgoing = source
                .out_msgs
                .iter_mut()
                .find(|msg| msg.hash == incoming.hash)
                .context("emulated parent has no matching output message")?;
            outgoing.in_msg_tx_hash = Some(tx.hash.clone());
        }
        transactions.push(tx);
    }
    let trace = trace_node(0, &transactions, &children)?;
    Ok(SimulationResponse {
        mc_block_seqno: mc_seqno,
        trace,
        transactions: transactions
            .into_iter()
            .map(|tx| (tx.hash.clone(), tx))
            .collect(),
        rand_seed: STANDARD.encode(seed),
        is_incomplete: incomplete,
        code_cells,
        data_cells,
    })
}

fn trace_node(
    index: usize,
    transactions: &[wire::Transaction],
    children: &[Vec<usize>],
) -> Result<TraceNode> {
    let tx = &transactions[index];
    Ok(TraceNode {
        tx_hash: tx.hash.clone(),
        in_msg_hash: tx
            .in_msg
            .as_ref()
            .context("emulated transaction has no input message")?
            .hash
            .clone(),
        children: children[index]
            .iter()
            .map(|&child| trace_node(child, transactions, children))
            .collect::<Result<_>>()?,
    })
}

fn fill_account(
    output: &mut wire::AccountState,
    shard: &ShardAccount,
    code_cells: &mut Option<BTreeMap<String, String>>,
    data_cells: &mut Option<BTreeMap<String, String>>,
) -> Result<()> {
    let Some(account) = shard.load_account()? else {
        output.balance = Some("0".into());
        output.extra_currencies = Some(Default::default());
        return Ok(());
    };
    output.balance = Some(account.balance.tokens.to_string());
    output.extra_currencies = Some(currencies(&account.balance.other)?);
    match account.state {
        AccountState::Active(state) => {
            output.code_hash = state.code.as_ref().map(|cell| cell_hash(cell, code_cells));
            output.data_hash = state.data.as_ref().map(|cell| cell_hash(cell, data_cells));
        }
        AccountState::Frozen(hash) => output.frozen_hash = Some(STANDARD.encode(hash)),
        AccountState::Uninit => {}
    }
    Ok(())
}

fn cell_hash(cell: &Cell, cells: &mut Option<BTreeMap<String, String>>) -> String {
    let hash = STANDARD.encode(cell.repr_hash());
    if let Some(cells) = cells {
        cells
            .entry(hash.clone())
            .or_insert_with(|| Boc::encode_base64(cell));
    }
    hash
}
