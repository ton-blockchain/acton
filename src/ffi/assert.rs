use super::SearchParamIndex;
use crate::context::{
    AssertBinFailure, AssertDecimalFailure, AssertFailure, Context, ExternalMessageNotFoundFailure,
    ExternalSendNotAcceptedFailure, FailAssertFailure, TransactionGenericAssertFailure,
    TransactionNotFoundParams, WalletNotFoundFailure,
};
use acton_debug::{RenderedValue, render_tuple_as_tolk_type};
use anyhow::{Context as ErrorContext, anyhow};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use ton_source_map::SourceLocation;
use tvm_ffi::from_stack::FromStack;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::models::{IntAddr, StdAddr, Transaction};

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

extension!(assume_reject in (Context) with (location: String, message: String) using assume_reject_impl);
fn assume_reject_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    location: String,
    message: String,
) -> anyhow::Result<()> {
    *ctx.asserts.assert_failure = Some(AssertFailure::Assume(FailAssertFailure {
        message: Some(if message.is_empty() {
            "assume(...) rejected this input".to_owned()
        } else {
            message
        }),
        location: SourceLocation::parse(&location)?,
    }));
    Ok(())
}

extension!(assert_bin in (Context) with (location: String, message: String, right: Tuple, right_ty_idx: BigInt, left: Tuple, left_ty_idx: BigInt, operator: String) using assert_bin_impl);
#[allow(clippy::too_many_arguments)]
fn assert_bin_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    location: String,
    message: String,
    right: Tuple,
    right_ty_idx: BigInt,
    left: Tuple,
    left_ty_idx: BigInt,
    operator: String,
) -> anyhow::Result<()> {
    let left = left.unwrap_single();
    let right = right.unwrap_single();
    let source_map = ctx.env.source_map.clone();
    let left_ty_idx = left_ty_idx
        .to_usize()
        .ok_or_else(|| anyhow!("ty_idx=`{left_ty_idx}` does not fit into usize"))?;
    let right_ty_idx = right_ty_idx
        .to_usize()
        .ok_or_else(|| anyhow!("ty_idx=`{right_ty_idx}` does not fit into usize"))?;

    if operator == "==" || operator == "!=" {
        let left_rendered = render_tuple_as_tolk_type(&source_map, &left, left_ty_idx);
        let right_rendered = render_tuple_as_tolk_type(&source_map, &right, right_ty_idx);
        let values_equal = rendered_values_equal(&left_rendered, &right_rendered);

        if operator == "==" && values_equal {
            stack.push_bool(true);
            return Ok(());
        }
        if operator == "!=" && !values_equal {
            stack.push_bool(true);
            return Ok(());
        }
    }

    if (operator == "<" || operator == ">" || operator == "<=" || operator == ">=")
        && let Some(TupleItem::Int(left_int)) = left.0.first()
        && let Some(TupleItem::Int(right_int)) = right.0.first()
        && (operator == "<" && left_int < right_int
            || operator == ">" && left_int > right_int
            || operator == "<=" && left_int <= right_int
            || operator == ">=" && left_int >= right_int)
    {
        stack.push_bool(true);
        return Ok(());
    }

    *ctx.asserts.assert_failure = Some(AssertFailure::Bin(AssertBinFailure {
        operator,
        left,
        right,
        left_ty_idx,
        right_ty_idx,
        source_map,
        message: Some(message),
        location: SourceLocation::parse(&location)?,
    }));
    stack.push_bool(false);
    Ok(())
}

pub(crate) fn rendered_values_equal(left: &RenderedValue, right: &RenderedValue) -> bool {
    let left = visible_rendered_value(left);
    let right = visible_rendered_value(right);

    match (left, right) {
        (
            RenderedValue::UnionCase {
                variant_name: left_variant,
                fields: left_fields,
                ..
            },
            RenderedValue::UnionCase {
                variant_name: right_variant,
                fields: right_fields,
                ..
            },
        ) => {
            if left_variant != right_variant {
                return false;
            }

            match (
                union_case_payload(left_fields),
                union_case_payload(right_fields),
            ) {
                (Some(left), Some(right)) => rendered_values_equal(left, right),
                (None, None) => true,
                _ => false,
            }
        }
        (
            RenderedValue::UnionCase {
                variant_name,
                fields,
                ..
            },
            right,
        ) => union_case_equal_to_value(variant_name, fields, right),
        (
            left,
            RenderedValue::UnionCase {
                variant_name,
                fields,
                ..
            },
        ) => union_case_equal_to_value(variant_name, fields, left),
        (
            RenderedValue::Leaf {
                value: left_value, ..
            },
            RenderedValue::Leaf {
                value: right_value, ..
            },
        )
        | (
            RenderedValue::Address {
                value: left_value, ..
            },
            RenderedValue::Address {
                value: right_value, ..
            },
        ) => left_value == right_value,
        (
            RenderedValue::CellLike {
                type_name: left_type,
                value: left_value,
                ..
            },
            RenderedValue::CellLike {
                type_name: right_type,
                value: right_value,
                ..
            },
        )
        | (
            RenderedValue::CellOf {
                type_name: left_type,
                value: left_value,
                ..
            },
            RenderedValue::CellOf {
                type_name: right_type,
                value: right_value,
                ..
            },
        )
        | (
            RenderedValue::EnumValue {
                type_name: left_type,
                value: left_value,
                ..
            },
            RenderedValue::EnumValue {
                type_name: right_type,
                value: right_value,
                ..
            },
        ) => left_type == right_type && left_value == right_value,
        (
            RenderedValue::Struct {
                type_name: left_type,
                fields: left_fields,
            },
            RenderedValue::Struct {
                type_name: right_type,
                fields: right_fields,
            },
        ) => left_type == right_type && rendered_fields_equal(left_fields, right_fields),
        (
            RenderedValue::Tensor {
                items: left_items, ..
            },
            RenderedValue::Tensor {
                items: right_items, ..
            },
        )
        | (
            RenderedValue::ArrayOf {
                items: left_items, ..
            },
            RenderedValue::ArrayOf {
                items: right_items, ..
            },
        ) => rendered_items_equal(left_items, right_items),
        (RenderedValue::OptimizedOut, RenderedValue::OptimizedOut)
        | (RenderedValue::LazyCantParseSlice, RenderedValue::LazyCantParseSlice) => true,
        (
            RenderedValue::LazyUnresolved {
                type_name: left_type,
            },
            RenderedValue::LazyUnresolved {
                type_name: right_type,
            },
        ) => left_type == right_type,
        _ => false,
    }
}

fn visible_rendered_value(mut value: &RenderedValue) -> &RenderedValue {
    loop {
        match value {
            RenderedValue::LastSeen { inner } => value = inner,
            RenderedValue::LazyNotYetLoaded { preview } => value = preview,
            _ => return value,
        }
    }
}

pub(crate) fn union_case_payload(fields: &[(String, RenderedValue)]) -> Option<&RenderedValue> {
    fields
        .iter()
        .find_map(|(name, value)| (name == "value").then_some(value))
}

fn union_case_equal_to_value(
    variant_name: &str,
    fields: &[(String, RenderedValue)],
    value: &RenderedValue,
) -> bool {
    if let Some(payload) = union_case_payload(fields) {
        return rendered_values_equal(payload, value);
    }

    match value {
        RenderedValue::Leaf {
            value: leaf_value, ..
        } => leaf_value == variant_name,
        RenderedValue::Struct { type_name, fields } => {
            fields.is_empty() && type_name == variant_name
        }
        _ => false,
    }
}

fn rendered_items_equal(left: &[RenderedValue], right: &[RenderedValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| rendered_values_equal(left, right))
}

fn rendered_fields_equal(
    left: &[(String, RenderedValue)],
    right: &[(String, RenderedValue)],
) -> bool {
    left.len() == right.len()
        && left.iter().all(|(left_name, left_value)| {
            right
                .iter()
                .find(|(right_name, _)| right_name == left_name)
                .is_some_and(|(_, right_value)| rendered_values_equal(left_value, right_value))
        })
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
        "expect(<actual>).toEqualDecimal(<expected>)".to_owned()
    } else {
        message
    };

    *ctx.asserts.assert_failure = Some(AssertFailure::Decimal(AssertDecimalFailure {
        left: left_str,
        right: right_str,
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
    let exit_code = i32::try_from(&code).context("Exit code value is too big for uint32")?;
    *ctx.asserts.expected_exit_code = Some(exit_code);
    Ok(())
}

extension!(fail_to_find_transaction_by_params in (Context) with (params: Tuple, txs: Vec<TupleItem>, message: String, location: String) using fail_to_find_transaction_by_params_impl);
fn fail_to_find_transaction_by_params_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    params: Tuple,
    txs: Vec<TupleItem>,
    message: String,
    location: String,
) -> anyhow::Result<()> {
    let Some((params, parsed_txs)) = process_txs_and_search_params(&txs, &params) else {
        return Ok(());
    };

    *ctx.asserts.assert_failure = Some(AssertFailure::TransactionNotFound(
        TransactionGenericAssertFailure {
            txs,
            parsed_txs,
            params,
            message: Some(message),
            location: SourceLocation::parse(&location)?,
        },
    ));
    Ok(())
}

extension!(fail_to_not_find_transaction_by_params in (Context) with (params: Tuple, txs: Vec<TupleItem>, message: String, location: String) using fail_to_not_find_transaction_by_params_impl);
fn fail_to_not_find_transaction_by_params_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    params: Tuple,
    txs: Vec<TupleItem>,
    message: String,
    location: String,
) -> anyhow::Result<()> {
    let Some((params, parsed_txs)) = process_txs_and_search_params(&txs, &params) else {
        return Ok(());
    };

    *ctx.asserts.assert_failure = Some(AssertFailure::TransactionIsFound(
        TransactionGenericAssertFailure {
            txs,
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

extension!(fail_to_find_external_message in (Context) with (opcode: BigInt, message_name: String, txs: Vec<TupleItem>, message: String, location: String) using fail_to_find_external_message_impl);
fn fail_to_find_external_message_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    opcode: BigInt,
    message_name: String,
    txs: Vec<TupleItem>,
    message: String,
    location: String,
) -> anyhow::Result<()> {
    *ctx.asserts.assert_failure = Some(AssertFailure::ExternalMessageNotFound(
        ExternalMessageNotFoundFailure {
            message: Some(message),
            location: SourceLocation::parse(&location)?,
            txs,
            message_name,
            opcode: opcode.to_u32(),
        },
    ));
    Ok(())
}

extension!(fail_external_send_not_accepted in (Context) with (error: Tuple, message: String, location: String) using fail_external_send_not_accepted_impl);
fn fail_external_send_not_accepted_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    error: Tuple,
    message: String,
    location: String,
) -> anyhow::Result<()> {
    let diagnostic_id = tuple_optional_int(&error, 3).and_then(|value| value.to_u64());
    let missing_libraries = diagnostic_id
        .and_then(|id| ctx.chain.emulations.find_failed_message(id))
        .map(|message| {
            let mut libraries = message
                .missing_libraries
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            libraries.sort_unstable();
            libraries
        })
        .unwrap_or_default();

    *ctx.asserts.assert_failure = Some(AssertFailure::ExternalSendNotAccepted(
        ExternalSendNotAcceptedFailure {
            message,
            reason: tuple_string(&error, 0)
                .unwrap_or_else(|| "External message not accepted by smart contract".to_owned()),
            external_not_accepted: tuple_bool(&error, 1).unwrap_or(false),
            vm_exit_code: tuple_optional_int(&error, 2).and_then(|value| value.to_i32()),
            diagnostic_id,
            missing_libraries,
            destination: tuple_optional_std_addr(&error, 4),
            location: SourceLocation::parse(&location)?,
        },
    ));
    Ok(())
}

fn tuple_item(tuple: &Tuple, index: usize) -> TupleItem {
    tuple.0.get(index).cloned().unwrap_or(TupleItem::Null)
}

fn tuple_string(tuple: &Tuple, index: usize) -> Option<String> {
    String::from_item(tuple_item(tuple, index)).ok()
}

fn tuple_bool(tuple: &Tuple, index: usize) -> Option<bool> {
    bool::from_item(tuple_item(tuple, index)).ok()
}

fn tuple_optional_int(tuple: &Tuple, index: usize) -> Option<BigInt> {
    Option::<BigInt>::from_item(tuple_item(tuple, index))
        .ok()
        .flatten()
}

fn tuple_optional_std_addr(tuple: &Tuple, index: usize) -> Option<StdAddr> {
    Option::<StdAddr>::from_item(tuple_item(tuple, index))
        .ok()
        .flatten()
}

#[must_use]
pub fn process_txs_and_search_params(
    txs: &[TupleItem],
    params: &Tuple,
) -> Option<(TransactionNotFoundParams, Vec<Transaction>)> {
    let params = parse_search_params(params)?;

    let parsed_txs = txs
        .iter()
        .filter_map(|el| match el {
            TupleItem::Tuple(tuple) => match tuple.first() {
                Some(TupleItem::Cell(cell)) => Some(cell),
                _ => None,
            },
            _ => None,
        })
        .filter_map(|x| x.parse::<Transaction>().ok())
        .collect::<Vec<_>>();

    Some((params, parsed_txs))
}

/// Extract tag, predicate, and optional original value from a sub-tuple.
/// Format: [0, null] = absent, [1, cont] = user predicate, [2, cont, `original_value`].
/// Returns None for tag 0. For tag 2, `original` holds the display value.
struct SubtupleData<'a> {
    tag: u8,
    original: Option<&'a TupleItem>,
}

fn read_subtuple(item: Option<&TupleItem>) -> Option<SubtupleData<'_>> {
    let TupleItem::Tuple(sub) = item? else {
        return None;
    };
    if sub.len() < 2 {
        return None;
    }
    let tag = match &sub[0] {
        TupleItem::Int(n) => n.to_u32().unwrap_or(0) as u8,
        _ => 0,
    };
    if tag == 0 {
        return None;
    }
    let original = if tag == 2 { sub.get(2) } else { None };
    Some(SubtupleData { tag, original })
}

use crate::context::DisplayParam;

#[must_use]
pub fn parse_search_params(params: &Tuple) -> Option<TransactionNotFoundParams> {
    let item_at = |idx: SearchParamIndex| params.0.get(idx.as_usize());

    let mut result = TransactionNotFoundParams {
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
        send_mode: None,
        compute_phase_skipped: None,
        body: None,
        state_init: None,
    };

    // Helper: parse a sub-tuple field as DisplayParam.
    // For tag=1 (user predicate) → Function.
    // For tag=2 (value-as-predicate) → extract original value from sub[2] for display.
    macro_rules! parse_field {
        (addr $field:ident, $idx:expr) => {
            if let Some(data) = read_subtuple(item_at($idx)) {
                result.$field = if data.tag == 1 {
                    Some(DisplayParam::Function)
                } else if let Some(orig) = data.original.and_then(read_optional_address_value) {
                    Some(DisplayParam::Value(orig))
                } else {
                    Some(DisplayParam::Function)
                };
            }
        };
        (bigint $field:ident, $idx:expr) => {
            if let Some(data) = read_subtuple(item_at($idx)) {
                result.$field = if data.tag == 1 {
                    Some(DisplayParam::Function)
                } else if let Some(num) = data.original.and_then(read_int_like_param) {
                    Some(DisplayParam::Value(num.clone()))
                } else {
                    Some(DisplayParam::Function)
                };
            }
        };
        (u32 $field:ident, $idx:expr) => {
            if let Some(data) = read_subtuple(item_at($idx)) {
                result.$field = if data.tag == 1 {
                    Some(DisplayParam::Function)
                } else {
                    data.original
                        .and_then(read_int_like_param)
                        .and_then(|n| n.to_u32())
                        .map(DisplayParam::Value)
                        .or(Some(DisplayParam::Function))
                };
            }
        };
        (i32 $field:ident, $idx:expr) => {
            if let Some(data) = read_subtuple(item_at($idx)) {
                result.$field = if data.tag == 1 {
                    Some(DisplayParam::Function)
                } else if let Some(num) = data.original.and_then(read_int_like_param) {
                    Some(DisplayParam::Value(num.to_i32().unwrap_or(0)))
                } else {
                    Some(DisplayParam::Function)
                };
            }
        };
        (bool $field:ident, $idx:expr) => {
            if let Some(data) = read_subtuple(item_at($idx)) {
                result.$field = if data.tag == 1 {
                    Some(DisplayParam::Function)
                } else if let Some(b) = data.original.and_then(read_bool_like_param) {
                    Some(DisplayParam::Value(b))
                } else {
                    Some(DisplayParam::Function)
                };
            }
        };
        (cell $field:ident, $idx:expr) => {
            if let Some(data) = read_subtuple(item_at($idx)) {
                result.$field = if data.tag == 1 {
                    Some(DisplayParam::Function)
                } else if let Some(TupleItem::Cell(cell)) = data.original {
                    Some(DisplayParam::Value(cell.clone()))
                } else {
                    Some(DisplayParam::Function)
                };
            }
        };
    }

    parse_field!(addr to, SearchParamIndex::To);
    parse_field!(addr from, SearchParamIndex::From);
    parse_field!(bigint value, SearchParamIndex::Value);
    parse_field!(u32 exit_code, SearchParamIndex::ExitCode);
    parse_field!(bool success, SearchParamIndex::Success);
    parse_field!(bool aborted, SearchParamIndex::Aborted);
    parse_field!(bool deploy, SearchParamIndex::Deploy);
    parse_field!(bool bounce, SearchParamIndex::Bounce);
    parse_field!(bool bounced, SearchParamIndex::Bounced);
    parse_field!(u32 opcode, SearchParamIndex::Opcode);
    parse_field!(i32 action_exit_code, SearchParamIndex::ActionExitCode);
    parse_field!(
        bool compute_phase_skipped,
        SearchParamIndex::ComputePhaseSkipped
    );
    parse_field!(cell body, SearchParamIndex::Body);
    parse_field!(cell state_init, SearchParamIndex::StateInit);
    parse_field!(u32 send_mode, SearchParamIndex::SendMode);

    Some(result)
}

fn read_optional_address_value(item: &TupleItem) -> Option<IntAddr> {
    match item {
        TupleItem::Slice(cell) | TupleItem::Cell(cell) => cell.parse::<IntAddr>().ok(),
        _ => None,
    }
}

fn read_int_like_param(item: &TupleItem) -> Option<&BigInt> {
    match item {
        TupleItem::Int(num) => Some(num),
        TupleItem::Tuple(items) => items.first().and_then(read_int_like_param),
        _ => None,
    }
}

fn read_bool_like_param(item: &TupleItem) -> Option<bool> {
    match item {
        TupleItem::Int(num) => Some(num.to_i64() == Some(-1)),
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
        107 => assume_reject : 2,
        108 => fail_to_find_external_message : 5,
        109 => fail_external_send_not_accepted : 3,
    });
}
