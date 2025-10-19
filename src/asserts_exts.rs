use crate::context::{AssertBinFailure, AssertFailure, Context, FailAssertFailure};
use emulator::executor::Executor;
use emulator::get_executor::GetExecutor;
use emulator::tuple::stack::Tuple;
use emulator::{extension, pop_args, register_ext_methods};

extension!(assert_fail in (Context) with (location: String, message: String) using assert_fail_impl);
fn assert_fail_impl(ctx: &mut Context, _stack: &mut Tuple, location: String, message: String) {
    *ctx.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
        message: Some(message),
        location: Some(location),
    }));
}

extension!(assert_equal in (Context) with (location: String, message: String, right: Tuple, right_name: String, left: Tuple, left_name: String) using assert_equal_impl);
fn assert_equal_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    location: String,
    message: String,
    right: Tuple,
    right_name: String,
    left: Tuple,
    left_name: String,
) {
    if left == right {
        stack.push_bool(true);
    } else {
        *ctx.assert_failure = Some(AssertFailure::Bin(AssertBinFailure {
            left,
            right,
            left_type: left_name,
            right_type: right_name,
            message: Some(message),
            location: Some(location),
        }));
        stack.push_bool(false);
    }
}

pub fn register_extensions(executor: &mut Executor, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        4 => assert_equal,
        100 => assert_fail,
    });
}

pub fn register_get_extensions(executor: &mut GetExecutor, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        4 => assert_equal,
        100 => assert_fail,
    });
}
