#![cfg(test)]
use crate::common::ExecutorVerbosity;
use crate::message::Executor;
use crate::message::types::{PrevBlockId, PrevBlocksInfo, RunTransactionArgs};
use crate::{DEFAULT_CONFIG, EXT_METHOD_STACK_ALL_ITEMS};
use num_bigint::BigInt;
use std::ffi::c_char;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;

const MESSAGE_B64: &str = "te6ccgEBAQEAXAAAs2gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ACW3KjJfr/ID5Nkj7xB33xCZD+wzKhEVCVM/gq78qkGEQF9eEAAAAAAAAAAAAAAAAAAAAAAAAwA==";
const SHARD_ACCOUNT_B64: &str = "te6ccgEBAgEAZQABUEIAo/QUie4HOlbbq3s8tbZIXLyq3iMgXy2Ih0e2fuJ7AAAAAAAtxsABAG/AAltyoyX6/yA+TZI+8Qd98QmQ/sMyoRFQlTP4Ku/KpBhCAl3DSqAZUAAAAAAAtxsFgEC6F1wABA==";

#[test]
fn prev_blocks_info_serializes_real_c7_tuple_shape() -> anyhow::Result<()> {
    let latest = PrevBlockId {
        workchain: 0,
        shard: i64::MIN,
        seqno: 42,
        root_hash: [0x11; 32],
        file_hash: [0x12; 32],
    };
    let key_block = PrevBlockId {
        workchain: -1,
        shard: -1,
        seqno: 40,
        root_hash: [0x21; 32],
        file_hash: [0x22; 32],
    };
    let sparse = PrevBlockId {
        workchain: 0,
        shard: i64::MIN,
        seqno: 0,
        root_hash: [0; 32],
        file_hash: [0; 32],
    };

    let encoded =
        PrevBlocksInfo::new(vec![latest], key_block, vec![sparse]).to_stack_entry_boc_base64()?;
    let cell = Boc::decode_base64(&encoded)?;
    let mut parser = cell.as_slice_allow_exotic();
    let TupleItem::Tuple(Tuple(fields)) = tvm_ffi::serde::parse_tuple_item(&mut parser)? else {
        panic!("prev_blocks_info must serialize as a tuple item");
    };

    assert_eq!(fields.len(), 3);
    let TupleItem::Tuple(last_mc_blocks) = &fields[0] else {
        panic!("last_mc_blocks must be a tuple");
    };
    let TupleItem::Tuple(first_block) = &last_mc_blocks[0] else {
        panic!("block id must be a tuple");
    };

    assert_eq!(first_block.len(), 5);
    assert_eq!(
        first_block[1],
        TupleItem::Int(BigInt::from(i64::MIN as u64))
    );
    Ok(())
}

#[test]
fn test_executor() -> anyhow::Result<()> {
    let exec = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;

    let result = exec.run_transaction(
        MESSAGE_B64,
        &RunTransactionArgs {
            shard_account: SHARD_ACCOUNT_B64.to_owned(),
            ..Default::default()
        },
    );

    println!("{result:?}");
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn test_executor_with_bad_libs() -> anyhow::Result<()> {
    let exec = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;

    let result = exec.run_transaction(
        MESSAGE_B64,
        &RunTransactionArgs {
            libs: Some(String::new()), // not a valid cell
            shard_account: SHARD_ACCOUNT_B64.to_owned(),
            ..Default::default()
        },
    );

    println!("{result:?}");

    assert!(result.is_err());
    assert_eq!(
        format!("{}", result.expect_err("should be an error")),
        "Cannot run transaction: Can't set params"
    );
    Ok(())
}

#[test]
fn test_executor_fail_with_tick_tock() -> anyhow::Result<()> {
    let exec = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;

    let result = exec.run_transaction(
        MESSAGE_B64,
        &RunTransactionArgs {
            shard_account: SHARD_ACCOUNT_B64.to_owned(),
            is_tick_tock: Some(false),
            is_tock: Some(true),
            ..Default::default()
        },
    );

    println!("{result:?}");

    assert!(result.is_err());
    assert_eq!(
        format!("{}", result.expect_err("should be an error")),
        "Cannot run transaction: Can't decode other params"
    );
    Ok(())
}

#[test]
fn test_executor_with_random_seed() -> anyhow::Result<()> {
    let exec = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;

    let result = exec.run_transaction(
        MESSAGE_B64,
        &RunTransactionArgs {
            shard_account: SHARD_ACCOUNT_B64.to_owned(),
            random_seed: Some(*&[
                1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4,
                5, 6, 7, 8,
            ]),
            ..Default::default()
        },
    );

    println!("{result:?}");
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn test_executor_with_ext_method() -> anyhow::Result<()> {
    struct MyContext {
        called_count: u32,
    }

    unsafe extern "C" fn my_callback(ctx: *mut MyContext, _arg: *const c_char) -> *const c_char {
        // SAFETY: `called_count` is valid non-null pointer
        unsafe {
            (*ctx).called_count += 1;
        }
        std::ptr::null()
    }

    let mut my_ctx = MyContext { called_count: 0 };

    let mut exec = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;

    exec.register_ext_method(100, &mut my_ctx, EXT_METHOD_STACK_ALL_ITEMS, my_callback)?;

    let result = exec.run_transaction(
        MESSAGE_B64,
        &RunTransactionArgs {
            shard_account: SHARD_ACCOUNT_B64.to_owned(),
            ..Default::default()
        },
    );

    println!("{result:?}");
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn test_executor_set_config() -> anyhow::Result<()> {
    let exec = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;

    let config_base64 = DEFAULT_CONFIG;

    let correct_result = exec.set_config(config_base64);
    assert!(correct_result.is_ok_and(std::convert::identity));

    let bad_config = DEFAULT_CONFIG.to_string() + "invalid_part";
    let bad_config_result = exec.set_config(&bad_config);
    assert!(bad_config_result.is_ok_and(|x| !x));

    Ok(())
}

// #[test]
// fn test_step_executor_run() -> anyhow::Result<()> {
//     let msg = "te6ccgEBAQEAXAAAs2gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ACW3KjJfr/ID5Nkj7xB33xCZD+wzKhEVCVM/gq78qkGEQF9eEAAAAAAAAAAAAAAAAAAAAAAAAwA==";
//     let shard_account = "te6ccgEBAgEAZQABUEIAo/QUie4HOlbbq3s8tbZIXLyq3iMgXy2Ih0e2fuJ7AAAAAAAtxsABAG/AAltyoyX6/yA+TZI+8Qd98QmQ/sMyoRFQlTP4Ku/KpBhCAl3DSqAZUAAAAAAAtxsFgEC6F1wABA==";
//
//     let exec = StepExecutor::new(None)?;
//
//     let prepare_result = exec.prepare_transaction(
//         msg,
//         RunTransactionArgs {
//             shard_account: shard_account.to_owned(),
//             ..Default::default()
//         },
//     )?;
//
//     assert!(prepare_result.success);
//
//     let mut steps = 0;
//     while !exec.step() {
//         steps += 1;
//         let _pos = exec.get_code_pos();
//         let _stack = exec.get_stack();
//     }
//     assert!(steps > 0);
//
//     let res = exec.finish_transaction()?;
//     assert!(matches!(res, EmulationResult::Success(_)));
//
//     Ok(())
// }
