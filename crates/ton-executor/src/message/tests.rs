#![cfg(test)]
use crate::common::ExecutorVerbosity;
use crate::message::Executor;
use crate::message::types::RunTransactionArgs;
use crate::{DEFAULT_CONFIG, EXT_METHOD_STACK_ALL_ITEMS};
use std::ffi::c_char;

#[test]
fn test_executor() -> anyhow::Result<()> {
    let exec = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;

    let msg = "te6ccgEBAQEAXAAAs2gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ACW3KjJfr/ID5Nkj7xB33xCZD+wzKhEVCVM/gq78qkGEQF9eEAAAAAAAAAAAAAAAAAAAAAAAAwA==";
    let shard_account = "te6ccgEBAgEAZQABUEIAo/QUie4HOlbbq3s8tbZIXLyq3iMgXy2Ih0e2fuJ7AAAAAAAtxsABAG/AAltyoyX6/yA+TZI+8Qd98QmQ/sMyoRFQlTP4Ku/KpBhCAl3DSqAZUAAAAAAAtxsFgEC6F1wABA==";

    let result = exec.run_transaction(
        msg,
        &RunTransactionArgs {
            shard_account: shard_account.to_owned(),
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

    let msg = "te6ccgEBAQEAXAAAs2gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ACW3KjJfr/ID5Nkj7xB33xCZD+wzKhEVCVM/gq78qkGEQF9eEAAAAAAAAAAAAAAAAAAAAAAAAwA==";
    let shard_account = "te6ccgEBAgEAZQABUEIAo/QUie4HOlbbq3s8tbZIXLyq3iMgXy2Ih0e2fuJ7AAAAAAAtxsABAG/AAltyoyX6/yA+TZI+8Qd98QmQ/sMyoRFQlTP4Ku/KpBhCAl3DSqAZUAAAAAAAtxsFgEC6F1wABA==";

    let result = exec.run_transaction(
        msg,
        &RunTransactionArgs {
            libs: Some(String::new()), // not a valid cell
            shard_account: shard_account.to_owned(),
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

    let msg = "te6ccgEBAQEAXAAAs2gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ACW3KjJfr/ID5Nkj7xB33xCZD+wzKhEVCVM/gq78qkGEQF9eEAAAAAAAAAAAAAAAAAAAAAAAAwA==";
    let shard_account = "te6ccgEBAgEAZQABUEIAo/QUie4HOlbbq3s8tbZIXLyq3iMgXy2Ih0e2fuJ7AAAAAAAtxsABAG/AAltyoyX6/yA+TZI+8Qd98QmQ/sMyoRFQlTP4Ku/KpBhCAl3DSqAZUAAAAAAAtxsFgEC6F1wABA==";

    let result = exec.run_transaction(
        msg,
        &RunTransactionArgs {
            shard_account: shard_account.to_owned(),
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

    let msg = "te6ccgEBAQEAXAAAs2gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ACW3KjJfr/ID5Nkj7xB33xCZD+wzKhEVCVM/gq78qkGEQF9eEAAAAAAAAAAAAAAAAAAAAAAAAwA==";
    let shard_account = "te6ccgEBAgEAZQABUEIAo/QUie4HOlbbq3s8tbZIXLyq3iMgXy2Ih0e2fuJ7AAAAAAAtxsABAG/AAltyoyX6/yA+TZI+8Qd98QmQ/sMyoRFQlTP4Ku/KpBhCAl3DSqAZUAAAAAAAtxsFgEC6F1wABA==";

    let result = exec.run_transaction(
        msg,
        &RunTransactionArgs {
            shard_account: shard_account.to_owned(),
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

    let msg = "te6ccgEBAQEAXAAAs2gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ACW3KjJfr/ID5Nkj7xB33xCZD+wzKhEVCVM/gq78qkGEQF9eEAAAAAAAAAAAAAAAAAAAAAAAAwA==";
    let shard_account = "te6ccgEBAgEAZQABUEIAo/QUie4HOlbbq3s8tbZIXLyq3iMgXy2Ih0e2fuJ7AAAAAAAtxsABAG/AAltyoyX6/yA+TZI+8Qd98QmQ/sMyoRFQlTP4Ku/KpBhCAl3DSqAZUAAAAAAAtxsFgEC6F1wABA==";

    let result = exec.run_transaction(
        msg,
        &RunTransactionArgs {
            shard_account: shard_account.to_owned(),
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
//     let exec = StepExecutor::new()?;
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
