use crate::context::{
    AssertBinFailure, AssertFailure, Context, FailAssertFailure, TransactionGenericAssertFailure,
    TransactionNotFoundParams, WalletNotFoundFailure,
};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use ton_source_map::SourceLocation;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::Load;
use tycho_types::models::{IntAddr, Transaction};

extension!(assert_fail in (Context) with (location: String, message: String) using assert_fail_impl);
fn assert_fail_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    location: String,
    message: String,
) -> anyhow::Result<()> {
    *ctx.asserts.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
        message: Some(message),
        location: SourceLocation::parse(&location)?,
    }));
    Ok(())
}

extension!(assert_bin in (Context) with (location: String, message: String, right: Tuple, right_name: String, left: Tuple, left_name: String, operator: String) using assert_bin_impl);
#[allow(clippy::too_many_arguments)]
fn assert_bin_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    location: String,
    message: String,
    right: Tuple,
    right_name: String,
    left: Tuple,
    left_name: String,
    operator: String,
) -> anyhow::Result<()> {
    let left = left.unwrap_single();
    let right = right.unwrap_single();

    if operator == "==" && left.equal_to(&right) {
        stack.push_bool(true);
        return Ok(());
    }
    if operator == "!=" && !left.equal_to(&right) {
        stack.push_bool(true);
        return Ok(());
    }

    if (operator == "<" || operator == ">" || operator == "<=" || operator == ">=")
        && let Some(TupleItem::Int(left_int)) = left.0.first()
        && let Some(TupleItem::Int(right_int)) = right.0.first()
    {
        if operator == "<" && left_int < right_int
            || operator == ">" && left_int > right_int
            || operator == "<=" && left_int <= right_int
            || operator == ">=" && left_int >= right_int
        {
            stack.push_bool(true);
            return Ok(());
        }

        *ctx.asserts.assert_failure = Some(AssertFailure::Bin(AssertBinFailure {
            operator,
            left,
            right,
            left_type: left_name,
            right_type: right_name,
            message: Some(message),
            location: SourceLocation::parse(&location)?,
        }));
        stack.push_bool(false);
        return Ok(());
    }

    *ctx.asserts.assert_failure = Some(AssertFailure::Bin(AssertBinFailure {
        operator,
        left,
        right,
        left_type: left_name,
        right_type: right_name,
        message: Some(message),
        location: SourceLocation::parse(&location)?,
    }));
    stack.push_bool(false);
    Ok(())
}

fn format_decimal(value: &BigInt, decimals: u32) -> String {
    let s = value.to_string();
    let is_negative = s.starts_with('-');
    let abs_s = if is_negative { &s[1..] } else { &s };
    let decimals = decimals as usize;

    let mut result = if abs_s.len() <= decimals {
        let mut res = "0.".to_string();
        res.push_str(&"0".repeat(decimals - abs_s.len()));
        res.push_str(abs_s);
        res
    } else {
        let mut res = abs_s.to_string();
        res.insert(abs_s.len() - decimals, '.');
        res
    };

    if result.contains('.') {
        result = result.trim_end_matches('0').to_string();
        if result.ends_with('.') {
            result.push('0');
        }
    }

    if is_negative {
        format!("-{result}")
    } else {
        result
    }
}

extension!(assert_decimal in (Context) with (left: BigInt, right: BigInt, decimals: BigInt, message: String, location: String) using assert_decimal_impl);
fn assert_decimal_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    left: BigInt,
    right: BigInt,
    decimals: BigInt,
    message: String,
    location: String,
) -> anyhow::Result<()> {
    if left == right {
        stack.push_bool(true);
        return Ok(());
    }

    let decimals_u32 = decimals.to_u32().unwrap_or(0);
    let left_str = format_decimal(&left, decimals_u32);
    let right_str = format_decimal(&right, decimals_u32);

    let message = if message.is_empty() {
        format!(
            "expect(<actual>).toEqualDecimal(<expected>)\n       Actual:   {left_str}\n       Expected: {right_str}"
        )
    } else {
        message
    };

    *ctx.asserts.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
        message: Some(message),
        location: SourceLocation::parse(&location)?,
    }));

    stack.push_bool(false);
    Ok(())
}

extension!(expect_to_end_with_exit_code in (Context) with (code: BigInt) using expect_to_end_with_exit_code_impl);
fn expect_to_end_with_exit_code_impl(
    ctx: &mut Context,
    _: &mut Tuple,
    code: BigInt,
) -> anyhow::Result<()> {
    *ctx.asserts.expected_exit_code = Some(code);
    Ok(())
}

extension!(fail_to_find_transaction_by_params in (Context) with (params: Tuple, txs: Tuple, message: String, location: String) using fail_to_find_transaction_by_params_impl);
fn fail_to_find_transaction_by_params_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    params: Tuple,
    txs: Tuple,
    message: String,
    location: String,
) -> anyhow::Result<()> {
    // struct SearchParams {
    //     to: address,
    //     from: address? = null,
    //     exit_code: int32? = null,
    //     deploy: bool? = null,
    //     bounced: bool? = null,
    //     opcode: int32? = null,
    //     action_exit_code: int32? = null,
    //     compute_phase_skipped: bool? = null,
    //     body: cell? = null,
    // }

    let (params, parsed_txs) = match process_txs_and_search_params(&txs, params) {
        Some(value) => value,
        None => return Ok(()),
    };

    *ctx.asserts.assert_failure = Some(AssertFailure::TransactionNotFound(
        TransactionGenericAssertFailure {
            txs: txs.to_typed("SendResultList"),
            parsed_txs,
            params,
            message: Some(message),
            location: SourceLocation::parse(&location)?,
        },
    ));
    Ok(())
}

extension!(fail_to_not_find_transaction_by_params in (Context) with (params: Tuple, txs: Tuple, message: String, location: String) using fail_to_not_find_transaction_by_params_impl);
fn fail_to_not_find_transaction_by_params_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    params: Tuple,
    txs: Tuple,
    message: String,
    location: String,
) -> anyhow::Result<()> {
    // struct SearchParams {
    //     to: address,
    //     from: address? = null,
    //     exit_code: int32? = null,
    //     deploy: bool? = null,
    //     bounced: bool? = null,
    //     opcode: int32? = null,
    //     action_exit_code: int32? = null,
    //     compute_phase_skipped: bool? = null,
    //     body: cell? = null,
    // }

    let (params, parsed_txs) = match process_txs_and_search_params(&txs, params) {
        Some(value) => value,
        None => return Ok(()),
    };

    *ctx.asserts.assert_failure = Some(AssertFailure::TransactionIsFound(
        TransactionGenericAssertFailure {
            txs: txs.to_typed("SendResultList"),
            parsed_txs,
            params,
            message: if message.is_empty() {
                None
            } else {
                Some(message)
            },
            location: SourceLocation::parse(&location)?,
        },
    ));
    Ok(())
}

extension!(fail_wallet_not_found in (Context) with (location: String, wallet_name: String) using fail_wallet_not_found_impl);
fn fail_wallet_not_found_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    location: String,
    wallet_name: String,
) -> anyhow::Result<()> {
    *ctx.asserts.assert_failure = Some(AssertFailure::WalletNotFound(WalletNotFoundFailure {
        wallet_name,
        location: SourceLocation::parse(&location)?,
    }));
    Ok(())
}

#[must_use]
pub fn process_txs_and_search_params(
    txs: &Tuple,
    params: Tuple,
) -> Option<(TransactionNotFoundParams, Vec<Transaction>)> {
    let mut params_reader = params.0;
    let raw_body = params_reader.pop();
    let raw_compute_phase_skipped = params_reader.pop();
    let raw_action_exit_code = params_reader.pop();
    let raw_opcode = params_reader.pop();
    let raw_bounced = params_reader.pop();
    let raw_bounce = params_reader.pop();
    let raw_deploy = params_reader.pop();
    let raw_aborted = params_reader.pop();
    let raw_success = params_reader.pop();
    let raw_exit_code = params_reader.pop();
    let raw_msg_value = params_reader.pop();
    let raw_from = params_reader.pop();
    let raw_to = params_reader.pop();

    let mut params = TransactionNotFoundParams {
        to: Default::default(),
        from: None,
        value: None,
        exit_code: None,
        success: None,
        aborted: None,
        deploy: None,
        bounce: None,
        bounced: None,
        opcode: None,
        action_exit_code: None,
        compute_phase_skipped: None,
        body: None,
    };

    if let Some(raw_opcode) = raw_opcode {
        if raw_opcode == TupleItem::Null {
            params.opcode = None;
        } else if let Some(num) = read_int_like_param(&raw_opcode) {
            params.opcode = num.to_u32();
        }
    }
    if let Some(raw_bounced) = raw_bounced {
        if raw_bounced == TupleItem::Null {
            params.bounced = None;
        } else if let TupleItem::Int(num) = raw_bounced {
            params.bounced = Some(num == BigInt::from(-1));
        }
    }
    if let Some(raw_bounce) = raw_bounce {
        if raw_bounce == TupleItem::Null {
            params.bounce = None;
        } else if let TupleItem::Int(num) = raw_bounce {
            params.bounce = Some(num == BigInt::from(-1));
        }
    }
    if let Some(raw_deploy) = raw_deploy {
        if raw_deploy == TupleItem::Null {
            params.deploy = None;
        } else if let TupleItem::Int(num) = raw_deploy {
            params.deploy = Some(num == BigInt::from(-1));
        }
    }
    if let Some(raw_exit_code) = raw_exit_code {
        if raw_exit_code == TupleItem::Null {
            params.exit_code = None;
        } else if let Some(num) = read_int_like_param(&raw_exit_code) {
            params.exit_code = num.to_u32();
        }
    }
    if let Some(raw_success) = raw_success {
        if raw_success == TupleItem::Null {
            params.success = None;
        } else if let TupleItem::Int(num) = raw_success {
            params.success = Some(num == BigInt::from(-1));
        }
    }
    if let Some(raw_aborted) = raw_aborted {
        if raw_aborted == TupleItem::Null {
            params.aborted = None;
        } else if let TupleItem::Int(num) = raw_aborted {
            params.aborted = Some(num == BigInt::from(-1));
        }
    }
    if let Some(raw_msg_value) = raw_msg_value {
        if raw_msg_value == TupleItem::Null {
            params.value = None;
        } else if let TupleItem::Int(num) = raw_msg_value {
            params.value = Some(num);
        }
    }
    if let Some(raw_from) = raw_from {
        if raw_from == TupleItem::Null {
            params.from = None;
        } else if let TupleItem::Tuple(raw_from) = &raw_from
            && let TupleItem::Slice(cell) = &raw_from[0]
        {
            let cell = Boc::decode(cell.to_boc(false).ok()?).ok()?;
            let mut slice = cell.as_slice().ok()?;
            if let Ok(address) = IntAddr::load_from(&mut slice) {
                params.from = Some(address);
            }
        } else if let TupleItem::Slice(cell) = raw_from {
            let cell = Boc::decode(cell.to_boc(false).ok()?).ok()?;
            let mut slice = cell.as_slice().ok()?;
            if let Ok(address) = IntAddr::load_from(&mut slice) {
                params.from = Some(address);
            }
        }
    }
    if let Some(raw_to) = raw_to {
        if raw_to == TupleItem::Null {
            params.to = None;
        } else if let TupleItem::Tuple(raw_to) = &raw_to
            && let TupleItem::Slice(cell) = &raw_to[0]
        {
            let cell = Boc::decode(cell.to_boc(false).ok()?).ok()?;
            let mut slice = cell.as_slice().ok()?;
            if let Ok(address) = IntAddr::load_from(&mut slice) {
                params.to = Some(address);
            }
        } else if let TupleItem::Slice(cell) = raw_to {
            let cell = Boc::decode(cell.to_boc(false).ok()?).ok()?;
            let mut slice = cell.as_slice().ok()?;
            if let Ok(address) = IntAddr::load_from(&mut slice) {
                params.to = Some(address);
            }
        }
    }
    if let Some(raw_action_exit_code) = raw_action_exit_code {
        if raw_action_exit_code == TupleItem::Null {
            params.action_exit_code = None;
        } else if let Some(num) = read_int_like_param(&raw_action_exit_code) {
            params.action_exit_code = Some(num.to_i32().unwrap_or(0));
        }
    }
    if let Some(raw_compute_phase_skipped) = raw_compute_phase_skipped {
        if raw_compute_phase_skipped == TupleItem::Null {
            params.compute_phase_skipped = None;
        } else if let TupleItem::Int(num) = raw_compute_phase_skipped {
            params.compute_phase_skipped = Some(num == BigInt::from(-1));
        }
    }
    if let Some(raw_body) = raw_body {
        if raw_body == TupleItem::Null {
            params.body = None;
        } else if let TupleItem::Cell(cell) = raw_body {
            let boc = cell.to_boc(false).ok()?;
            let decoded_cell = Boc::decode(&boc).ok()?;
            params.body = Some(decoded_cell);
        }
    }

    let parsed_txs = txs
        .0
        .iter()
        .filter_map(|el| match el {
            TupleItem::Tuple(tuple) => match &tuple[0] {
                TupleItem::Cell(cell) => Some(cell),
                _ => None,
            },
            _ => None,
        })
        .filter_map(|x| {
            let result = x.to_boc(false).ok()?;
            let tx_cell = Boc::decode(&result).ok()?;
            tx_cell.parse::<Transaction>().ok()
        })
        .collect::<Vec<_>>();

    Some((params, parsed_txs))
}

fn read_int_like_param(item: &TupleItem) -> Option<BigInt> {
    match item {
        TupleItem::Int(num) => Some(num.clone()),
        TupleItem::Tuple(items) => items.first().and_then(read_int_like_param),
        TupleItem::TypedTuple { inner, .. } => inner.0.first().and_then(read_int_like_param),
        _ => None,
    }
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        100 => assert_fail : 2,
        101 => assert_bin : 7,
        102 => expect_to_end_with_exit_code : 1,
        103 => fail_to_find_transaction_by_params : 4,
        104 => fail_to_not_find_transaction_by_params : 4,
        105 => fail_wallet_not_found : 2,
        106 => assert_decimal : 5,
    });
}
