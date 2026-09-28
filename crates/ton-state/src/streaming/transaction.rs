//! Maps transaction cells to TON Center v3 fields. Callers supply account states
//! and trace relations when they have them; the cell converter never infers them.

use std::collections::HashMap;

use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, HashBytes, Lazy};
use rston::models::{
    AccountStatus, AccountStatusChange, ActionPhase, BouncePhase, ComputePhase,
    ComputePhaseSkipReason, ExtraCurrencyCollection, Message, MsgInfo, StoragePhase, TickTock,
    Transaction, TxInfo,
};
use serde_json::{Value, json};
use ton_indexer_core::BlockId;
use toncenter::v3::{StringOrNumber, responses as wire};

/// Converts one finalized transaction without fetching account state or following
/// messages. `block` is its actual shard block; `mc_seqno` is the committing anchor.
pub(super) fn convert(
    block: BlockId,
    mc_seqno: u32,
    lazy: &Lazy<Transaction>,
    tx: &Transaction,
) -> Result<wire::Transaction> {
    let mut result = convert_cell(block.workchain, lazy, tx)?;
    result.block_ref.shard = format!("{:016x}", block.shard);
    result.block_ref.seqno = block.seqno;
    result.mc_block_seqno = mc_seqno;
    result.emulated = false;
    result.finality = "finalized".into();
    Ok(result)
}

/// Converts an uncommitted transaction. Zero shard/seqno are placeholders, not
/// block inclusion evidence. The caller sets its simulation's masterchain anchor.
pub(crate) fn convert_cell(
    workchain: i32,
    lazy: &Lazy<Transaction>,
    tx: &Transaction,
) -> Result<wire::Transaction> {
    let hash = STANDARD.encode(lazy.inner().repr_hash());
    let update = tx.state_update.load()?;
    let info = tx.load_info()?;
    let mut description = wire::TransactionDescr {
        kind: String::new(),
        aborted: None,
        destroyed: None,
        credit_first: None,
        compute_ph: None,
        action: None,
        storage_ph: None,
        credit_ph: None,
        bounce: None,
        installed: None,
        is_tock: None,
        split_info: None,
    };

    match info {
        TxInfo::Ordinary(info) => {
            description.kind = "ord".into();
            description.aborted = Some(info.aborted);
            description.destroyed = Some(info.destroyed);
            description.credit_first = Some(info.credit_first);
            description.storage_ph = info.storage_phase.as_ref().map(storage_phase);
            description.compute_ph = Some(compute_phase(&info.compute_phase));
            description.action = info.action_phase.as_ref().map(action_phase);
            description.bounce = info.bounce_phase.as_ref().map(bounce_phase);
            description.credit_ph = info
                .credit_phase
                .map(|phase| {
                    Ok::<_, anyhow::Error>(wire::CreditPhase {
                        due_fees_collected: phase.due_fees_collected.map(|value| value.to_string()),
                        credit: Some(phase.credit.tokens.to_string()),
                        credit_extra_currencies: currencies(&phase.credit.other)?,
                    })
                })
                .transpose()?;
        }
        TxInfo::TickTock(info) => {
            description.kind = "tick_tock".into();
            description.is_tock = Some(info.kind == TickTock::Tock);
            description.aborted = Some(info.aborted);
            description.destroyed = Some(info.destroyed);
            description.storage_ph = Some(storage_phase(&info.storage_phase));
            description.compute_ph = Some(compute_phase(&info.compute_phase));
            description.action = info.action_phase.as_ref().map(action_phase);
        }
    }

    Ok(wire::Transaction {
        account: format!("{workchain}:{}", tx.account),
        hash: hash.clone(),
        lt: tx.lt.to_string(),
        block_ref: wire::BlockId {
            workchain,
            shard: "0000000000000000".into(),
            seqno: 0,
        },
        now: tx.now,
        mc_block_seqno: 0,
        emulated: true,
        finality: "pending".into(),
        prev_trans_hash: STANDARD.encode(tx.prev_trans_hash),
        prev_trans_lt: tx.prev_trans_lt.to_string(),
        orig_status: account_status(tx.orig_status).into(),
        end_status: account_status(tx.end_status).into(),
        total_fees: tx.total_fees.tokens.to_string(),
        total_fees_extra_currencies: currencies(&tx.total_fees.other)?,
        trace_external_hash: None,
        trace_id: None,
        child_transactions: Vec::new(),
        description,
        in_msg: tx
            .in_msg
            .as_ref()
            .map(|cell| message(cell, &hash, true))
            .transpose()?,
        out_msgs: tx
            .out_msgs
            .values()
            .map(|cell| message(&cell?, &hash, false))
            .collect::<Result<_>>()?,
        account_state_before: account_state(update.old, tx.orig_status),
        account_state_after: account_state(update.new, tx.end_status),
    })
}

fn message(cell: &Cell, tx_hash: &str, incoming: bool) -> Result<wire::Message> {
    let message = cell.parse::<Message>()?;
    let body = CellBuilder::build_from(message.body)?;
    let mut result = wire::Message {
        hash: STANDARD.encode(cell.repr_hash()),
        hash_norm: None,
        source: None,
        destination: None,
        value: None,
        value_extra_currencies: None,
        fwd_fee: None,
        ihr_fee: None,
        created_lt: None,
        created_at: None,
        decoded_opcode: None,
        extra_flags: None,
        ihr_disabled: None,
        bounce: None,
        bounced: None,
        import_fee: None,
        in_msg_tx_hash: incoming.then(|| tx_hash.to_owned()),
        opcode: body
            .as_slice()?
            .load_u32()
            .ok()
            .map(|opcode| StringOrNumber::String(format!("0x{opcode:08x}"))),
        out_msg_tx_hash: (!incoming).then(|| tx_hash.to_owned()),
        message_content: Some(content(&body)),
        init_state: message
            .init
            .as_ref()
            .map(CellBuilder::build_from)
            .transpose()?
            .as_ref()
            .map(content),
    };

    match message.info {
        MsgInfo::Int(info) => {
            result.source = Some(info.src.to_string());
            result.destination = Some(info.dst.to_string());
            result.value = Some(info.value.tokens.to_string());
            result.value_extra_currencies = Some(currencies(&info.value.other)?);
            result.fwd_fee = Some(info.fwd_fee.to_string());
            result.ihr_fee = Some("0".into());
            result.extra_flags = Some(info.extra_flags.bits().to_string());
            result.created_lt = Some(info.created_lt.to_string());
            result.created_at = Some(info.created_at.to_string());
            result.ihr_disabled = Some(info.ihr_disabled);
            result.bounce = Some(info.bounce);
            result.bounced = Some(info.bounced);
        }
        MsgInfo::ExtIn(info) => {
            result.source = info.src.as_ref().map(ToString::to_string);
            result.destination = Some(info.dst.to_string());
            result.import_fee = Some(info.import_fee.to_string());
        }
        MsgInfo::ExtOut(info) => {
            result.source = Some(info.src.to_string());
            result.destination = info.dst.as_ref().map(ToString::to_string);
            result.created_lt = Some(info.created_lt.to_string());
            result.created_at = Some(info.created_at.to_string());
        }
    }
    Ok(result)
}

fn content(cell: &Cell) -> wire::MessageContent {
    wire::MessageContent {
        hash: Some(STANDARD.encode(cell.repr_hash())),
        body: Some(Boc::encode_base64(cell)),
        decoded: None,
    }
}

/// Encodes identifiers and 248-bit amounts as decimal strings without losing
/// precision in clients whose JSON numbers cannot represent the full range.
pub(crate) fn currencies(values: &ExtraCurrencyCollection) -> Result<HashMap<String, String>> {
    values
        .as_dict()
        .iter()
        .map(|entry| {
            let (id, value) = entry?;
            Ok((id.to_string(), value.to_string()))
        })
        .collect()
}

fn account_state(hash: HashBytes, status: AccountStatus) -> wire::AccountState {
    wire::AccountState {
        hash: STANDARD.encode(hash),
        account_status: Some(account_status(status).into()),
        balance: None,
        code_boc: None,
        code_hash: None,
        data_boc: None,
        data_hash: None,
        extra_currencies: None,
        frozen_hash: None,
    }
}

const fn account_status(status: AccountStatus) -> &'static str {
    match status {
        AccountStatus::Uninit => "uninit",
        AccountStatus::Frozen => "frozen",
        AccountStatus::Active => "active",
        AccountStatus::NotExists => "nonexist",
    }
}

fn storage_phase(phase: &StoragePhase) -> wire::StoragePhase {
    wire::StoragePhase {
        storage_fees_collected: Some(phase.storage_fees_collected.to_string()),
        storage_fees_due: phase.storage_fees_due.map(|value| value.to_string()),
        status_change: Some(status_change(phase.status_change).into()),
    }
}

const fn status_change(change: AccountStatusChange) -> &'static str {
    match change {
        AccountStatusChange::Unchanged => "unchanged",
        AccountStatusChange::Frozen => "frozen",
        AccountStatusChange::Deleted => "deleted",
    }
}

fn compute_phase(phase: &ComputePhase) -> wire::ComputePhase {
    match phase {
        ComputePhase::Skipped(phase) => wire::ComputePhase {
            skipped: Some(true),
            reason: Some(
                match phase.reason {
                    ComputePhaseSkipReason::NoState => "no_state",
                    ComputePhaseSkipReason::BadState => "bad_state",
                    ComputePhaseSkipReason::NoGas => "no_gas",
                    ComputePhaseSkipReason::Suspended => "suspended",
                }
                .into(),
            ),
            success: None,
            msg_state_used: None,
            account_activated: None,
            gas_fees: None,
            gas_used: None,
            gas_limit: None,
            gas_credit: None,
            mode: None,
            exit_code: None,
            exit_arg: None,
            vm_steps: None,
            vm_init_state_hash: None,
            vm_final_state_hash: None,
        },
        ComputePhase::Executed(phase) => wire::ComputePhase {
            skipped: Some(false),
            success: Some(phase.success),
            msg_state_used: Some(phase.msg_state_used),
            account_activated: Some(phase.account_activated),
            gas_fees: Some(phase.gas_fees.to_string()),
            gas_used: Some(phase.gas_used.to_string()),
            gas_limit: Some(phase.gas_limit.to_string()),
            gas_credit: phase.gas_credit.map(|value| value.to_string()),
            mode: Some(phase.mode),
            exit_code: Some(phase.exit_code),
            exit_arg: phase.exit_arg,
            vm_steps: Some(phase.vm_steps),
            vm_init_state_hash: Some(STANDARD.encode(phase.vm_init_state_hash)),
            vm_final_state_hash: Some(STANDARD.encode(phase.vm_final_state_hash)),
            reason: None,
        },
    }
}

fn action_phase(phase: &ActionPhase) -> wire::ActionPhase {
    wire::ActionPhase {
        success: Some(phase.success),
        valid: Some(phase.valid),
        no_funds: Some(phase.no_funds),
        status_change: Some(status_change(phase.status_change).into()),
        result_code: Some(phase.result_code),
        result_arg: phase.result_arg,
        tot_actions: Some(phase.total_actions.into()),
        spec_actions: Some(phase.special_actions.into()),
        skipped_actions: Some(phase.skipped_actions.into()),
        msgs_created: Some(phase.messages_created.into()),
        total_fwd_fees: phase.total_fwd_fees.map(|value| value.to_string()),
        total_action_fees: phase.total_action_fees.map(|value| value.to_string()),
        action_list_hash: Some(STANDARD.encode(phase.action_list_hash)),
        tot_msg_size: Some(wire::MsgSize {
            cells: Some(phase.total_message_size.cells.to_string()),
            bits: Some(phase.total_message_size.bits.to_string()),
        }),
    }
}

fn bounce_phase(phase: &BouncePhase) -> Value {
    match phase {
        BouncePhase::NegativeFunds => json!({"type": "negfunds"}),
        BouncePhase::NoFunds(phase) => json!({
            "type": "nofunds",
            "msg_size": {"cells": phase.msg_size.cells.to_string(), "bits": phase.msg_size.bits.to_string()},
            "req_fwd_fees": phase.req_fwd_fees.to_string(),
        }),
        BouncePhase::Executed(phase) => json!({
            "type": "ok",
            "msg_size": {"cells": phase.msg_size.cells.to_string(), "bits": phase.msg_size.bits.to_string()},
            "msg_fees": phase.msg_fees.to_string(),
            "fwd_fees": phase.fwd_fees.to_string(),
        }),
    }
}
