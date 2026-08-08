use crate::LocalnetError;
use crate::executor::ExecResult;
use crate::node::Node;
use crate::storage::JettonMasterMeta;
use crate::types::{Addr, BocBytes, Hash256};
use anyhow::Context;
use serde_json::Value;
use tycho_types::boc::BocRepr;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::models::{
    AnyAddr, CurrencyCollection, IntMsgInfo, Message, MsgInfo, OwnedMessage, StdAddr,
};
use tycho_types::num::Tokens;

const LEGACY_MINT_OPCODE: u32 = 0x0000_0015;
const CURRENT_MINT_OPCODE: u32 = 0x642b_7d07;
const INTERNAL_TRANSFER_STEP_OPCODE: u32 = 0x178d_4519;
const DEFAULT_JETTON_DECIMALS: u32 = 9;
const MAX_JETTON_DECIMALS: u32 = 30;
const MINT_MESSAGE_VALUE: u128 = 100_000_000;
const MINT_TON_AMOUNT: u128 = 50_000_000;
const FORWARD_TON_AMOUNT: u128 = 20_000_000;

#[derive(Clone, Copy, Debug)]
enum MintLayout {
    LegacyV1,
    Current,
}

impl MintLayout {
    const ALL: [Self; 2] = [Self::LegacyV1, Self::Current];

    const fn opcode(self) -> u32 {
        match self {
            Self::LegacyV1 => LEGACY_MINT_OPCODE,
            Self::Current => CURRENT_MINT_OPCODE,
        }
    }
}

pub(crate) fn mint(
    node: &mut Node,
    master_address: &Addr,
    recipient: &Addr,
    amount: &str,
) -> anyhow::Result<Hash256> {
    node.ensure_detected_assets_for_address(master_address)?;
    let master = node
        .iter_jetton_masters()
        .find(|master| master.address == *master_address)
        .cloned()
        .ok_or_else(|| LocalnetError::invalid_request("This address is not a jetton master"))?;
    if !master.mintable {
        return Err(LocalnetError::invalid_request("This jetton cannot be minted").into());
    }
    let admin = master.admin_address.ok_or_else(|| {
        LocalnetError::invalid_request(
            "This jetton master has no admin address, so the faucet cannot mint it",
        )
    })?;
    let amount = parse_amount(amount, decimals(&master.jetton_content))?;
    let mut accepted = Vec::new();
    let mut outcomes = Vec::new();

    for layout in MintLayout::ALL {
        let boc = build_message(&master, admin, recipient, amount, layout)?;
        let result = node
            .preflight_internal_boc(&boc)
            .with_context(|| format!("Failed to preflight {layout:?} jetton mint message"))?;
        let creates_transfer = creates_internal_transfer(&result);
        let aborted = result.is_aborted();
        let compute_exit_code = result.compute_exit_code();
        let action_result_code = result.action_result_code();
        let is_accepted = !aborted
            && compute_exit_code == Some(0)
            && action_result_code == Some(0)
            && creates_transfer;
        outcomes.push(format!(
            "{layout:?}: aborted={aborted}, compute_exit_code={compute_exit_code:?}, action_result_code={action_result_code:?}, internal_transfer={creates_transfer}",
        ));
        if is_accepted {
            accepted.push(boc);
        }
    }

    if accepted.len() != 1 {
        return Err(LocalnetError::invalid_request(format!(
            "Could not uniquely detect the jetton mint layout ({})",
            outcomes.join("; ")
        ))
        .into());
    }

    node.send_internal_boc(accepted.pop().expect("one accepted mint message"))
}

fn build_message(
    master: &JettonMasterMeta,
    admin: Addr,
    recipient: &Addr,
    amount: u128,
    layout: MintLayout,
) -> anyhow::Result<BocBytes> {
    let context = Cell::empty_context();
    let mut internal_transfer = CellBuilder::new();
    internal_transfer.store_u32(INTERNAL_TRANSFER_STEP_OPCODE)?;
    internal_transfer.store_u64(0)?;
    Tokens::new(amount).store_into(&mut internal_transfer, context)?;
    AnyAddr::None.store_into(&mut internal_transfer, context)?;
    AnyAddr::None.store_into(&mut internal_transfer, context)?;
    Tokens::new(FORWARD_TON_AMOUNT).store_into(&mut internal_transfer, context)?;
    internal_transfer.store_bit_zero()?;

    let mut body = CellBuilder::new();
    body.store_u32(layout.opcode())?;
    body.store_u64(0)?;
    StdAddr::from(recipient).store_into(&mut body, context)?;
    Tokens::new(MINT_TON_AMOUNT).store_into(&mut body, context)?;
    body.store_reference(internal_transfer.build()?)?;

    let message = OwnedMessage {
        info: MsgInfo::Int(IntMsgInfo {
            ihr_disabled: true,
            bounce: false,
            bounced: false,
            src: admin.into(),
            dst: master.address.into(),
            ihr_fee: Tokens::ZERO,
            value: CurrencyCollection::new(MINT_MESSAGE_VALUE),
            fwd_fee: Tokens::ZERO,
            created_at: 0,
            created_lt: 0,
        }),
        init: None,
        body: body.build()?.into(),
        layout: None,
    };
    Ok(BocRepr::encode(message)?.into())
}

fn creates_internal_transfer(result: &ExecResult) -> bool {
    result.out_msg_cells.iter().any(|cell| {
        let Ok(mut message) = cell.parse::<Message<'_>>() else {
            return false;
        };
        matches!(message.info, MsgInfo::Int(_))
            && message.body.load_u32().ok() == Some(INTERNAL_TRANSFER_STEP_OPCODE)
    })
}

fn decimals(content: &Value) -> u32 {
    content
        .get("decimals")
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .filter(|decimals| *decimals <= MAX_JETTON_DECIMALS)
        .unwrap_or(DEFAULT_JETTON_DECIMALS)
}

fn parse_amount(value: &str, decimals: u32) -> anyhow::Result<u128> {
    let invalid_amount = || {
        LocalnetError::invalid_request(format!(
            "Invalid jetton amount: expected a value greater than zero with up to {decimals} decimal places"
        ))
    };
    let value = value.trim();
    let (whole, fraction) = match value.split_once('.') {
        Some((whole, fraction)) if decimals > 0 && !fraction.contains('.') => (whole, fraction),
        Some(_) => return Err(invalid_amount().into()),
        None => (value, ""),
    };
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > decimals as usize
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid_amount().into());
    }

    let scale = 10u128.checked_pow(decimals).ok_or_else(invalid_amount)?;
    let whole = whole.parse::<u128>().map_err(|_| invalid_amount())?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u128>().map_err(|_| invalid_amount())?
            * 10u128
                .checked_pow(decimals - fraction.len() as u32)
                .ok_or_else(invalid_amount)?
    };
    let amount = whole
        .checked_mul(scale)
        .and_then(|whole| whole.checked_add(fraction))
        .filter(|amount| *amount > 0)
        .ok_or_else(invalid_amount)?;
    if !Tokens::new(amount).is_valid() {
        return Err(invalid_amount().into());
    }
    Ok(amount)
}
