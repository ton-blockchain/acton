use crate::context::{
    AssertBinFailure, AssertFailure, Context, FailAssertFailure, TransactionGenericAssertFailure,
    TransactionNotFoundParams,
};
use emulator::traits::BaseExecutor;
use emulator::{extension, pop_args, register_ext_methods};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::Load;
use tycho_types::models::{IntAddr, Transaction};

extension!(assert_fail in (Context) with (location: String, message: String) using assert_fail_impl);
fn assert_fail_impl(ctx: &mut Context, _stack: &mut Tuple, location: String, message: String) {
    *ctx.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
        message: Some(message),
        location: Some(location),
    }));
}

extension!(assert_bin in (Context) with (location: String, message: String, right: Tuple, right_name: String, left: Tuple, left_name: String, operator: String) using assert_bin_impl);
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
) {
    let left = left.unwrap_single();
    let right = right.unwrap_single();

    if operator == "==" && left == right {
        stack.push_bool(true);
        return;
    }
    if operator == "!=" && left != right {
        stack.push_bool(true);
        return;
    }

    if operator == "<" || operator == ">" || operator == "<=" || operator == ">=" {
        if let Some(TupleItem::Int(left_int)) = left.0.first()
            && let Some(TupleItem::Int(right_int)) = right.0.first()
        {
            if operator == "<" && left_int < right_int
                || operator == ">" && left_int > right_int
                || operator == "<=" && left_int <= right_int
                || operator == ">=" && left_int >= right_int
            {
                stack.push_bool(true);
                return;
            }

            *ctx.assert_failure = Some(AssertFailure::Bin(AssertBinFailure {
                operator,
                left,
                right,
                left_type: left_name,
                right_type: right_name,
                message: Some(message),
                location: Some(location),
            }));
            stack.push_bool(false);
            return;
        }
    }

    *ctx.assert_failure = Some(AssertFailure::Bin(AssertBinFailure {
        operator,
        left,
        right,
        left_type: left_name,
        right_type: right_name,
        message: Some(message),
        location: Some(location),
    }));
    stack.push_bool(false);
}

extension!(expect_to_end_with_exit_code in (Context) with (code: BigInt) using expect_to_end_with_exit_code_impl);
fn expect_to_end_with_exit_code_impl(ctx: &mut Context, _stack: &mut Tuple, code: BigInt) {
    *ctx.expected_exit_code = Some(code);
}

extension!(fail_to_find_transaction_by_params in (Context) with (params: Tuple, txs: Tuple, message: String, location: String) using fail_to_find_transaction_by_params_impl);
fn fail_to_find_transaction_by_params_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    params: Tuple,
    txs: Tuple,
    message: String,
    location: String,
) {
    // struct SearchParams {
    //     to: address,
    //     from: address? = null,
    //     exit_code: int32? = null,
    //     deploy: bool? = null,
    // }

    let (params, parsed_txs) = match process_txs_and_search_params(&txs, params) {
        Some(value) => value,
        None => return,
    };

    *ctx.assert_failure = Some(AssertFailure::TransactionNotFound(
        TransactionGenericAssertFailure {
            txs: txs.to_typed(&"TransactionList".to_string()),
            parsed_txs,
            params,
            message: Some(message),
            location: Some(location),
        },
    ));
}

extension!(fail_to_not_find_transaction_by_params in (Context) with (params: Tuple, txs: Tuple, message: String, location: String) using fail_to_not_find_transaction_by_params_impl);
fn fail_to_not_find_transaction_by_params_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    params: Tuple,
    txs: Tuple,
    message: String,
    location: String,
) {
    // struct SearchParams {
    //     to: address,
    //     from: address? = null,
    //     exit_code: int32? = null,
    //     deploy: bool? = null,
    // }

    let (params, parsed_txs) = match process_txs_and_search_params(&txs, params) {
        Some(value) => value,
        None => return,
    };

    *ctx.assert_failure = Some(AssertFailure::TransactionIsFound(
        TransactionGenericAssertFailure {
            txs: txs.to_typed(&"TransactionList".to_string()),
            parsed_txs,
            params,
            message: if message.is_empty() {
                None
            } else {
                Some(message)
            },
            location: if location.is_empty() {
                None
            } else {
                Some(location)
            },
        },
    ));
}

pub fn process_txs_and_search_params(
    txs: &Tuple,
    params: Tuple,
) -> Option<(TransactionNotFoundParams, Vec<Transaction>)> {
    let mut params_reader = params.clone().0;
    let raw_bounced = params_reader.pop();
    let raw_deploy = params_reader.pop();
    let raw_exit_code = params_reader.pop();
    let raw_from = params_reader.pop();
    let raw_to = params_reader.pop();

    let mut params = TransactionNotFoundParams {
        to: Default::default(),
        from: None,
        exit_code: None,
        deploy: None,
        bounced: None,
    };

    if let Some(raw_bounced) = raw_bounced {
        if let TupleItem::Null = raw_bounced {
            params.bounced = None
        } else if let TupleItem::Int(num) = raw_bounced {
            params.bounced = Some(num == BigInt::from(18446744073709551615u64))
        }
    }
    if let Some(raw_deploy) = raw_deploy {
        if let TupleItem::Null = raw_deploy {
            params.deploy = None
        } else if let TupleItem::Int(num) = raw_deploy {
            params.deploy = Some(num == BigInt::from(18446744073709551615u64))
        }
    }
    if let Some(raw_exit_code) = raw_exit_code {
        if let TupleItem::Null = raw_exit_code {
            params.exit_code = None
        } else if let TupleItem::Int(num) = raw_exit_code {
            params.exit_code = num.to_u32()
        }
    }
    if let Some(raw_from) = raw_from {
        if let TupleItem::Null = raw_from {
            params.from = None
        } else if let TupleItem::Tuple(raw_from) = &raw_from
            && let TupleItem::Slice(cell) = &raw_from[0]
        {
            let cell = Boc::decode_base64(cell.to_boc_b64(false).unwrap()).unwrap();
            let mut slice = cell.as_slice().unwrap();
            if let Ok(address) = IntAddr::load_from(&mut slice) {
                params.from = Some(address);
            }
        } else if let TupleItem::Slice(cell) = raw_from {
            let cell = Boc::decode_base64(cell.to_boc_b64(false).unwrap()).unwrap();
            let mut slice = cell.as_slice().unwrap();
            if let Ok(address) = IntAddr::load_from(&mut slice) {
                params.from = Some(address);
            }
        }
    }
    if let Some(raw_to) = raw_to {
        if let TupleItem::Slice(cell) = raw_to {
            let cell = Boc::decode_base64(cell.to_boc_b64(false).unwrap()).unwrap();
            let mut slice = cell.as_slice().unwrap();
            if let Ok(address) = IntAddr::load_from(&mut slice) {
                params.to = address;
            }
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
        .map(|x| {
            let result = x.to_boc_b64(false).unwrap();
            let tx_cell: tycho_types::cell::Cell = Boc::decode_base64(&result).unwrap();
            let mut tx_slice = tx_cell.as_slice().unwrap();
            Transaction::load_from(&mut tx_slice).unwrap()
        })
        .collect::<Vec<_>>();

    Some((params, parsed_txs))
}

pub fn register_extensions(executor: &mut dyn BaseExecutor, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        100 => assert_fail,
        101 => assert_bin,
        102 => expect_to_end_with_exit_code,
        103 => fail_to_find_transaction_by_params,
        104 => fail_to_not_find_transaction_by_params,
    });
}
