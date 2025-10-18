use crate::context::{AssertFailure, Context};
use emulator::executor::Executor;
use emulator::get_executor::GetExecutor;
use emulator::tuple::stack::Tuple;
use emulator::{extension, pop_args, register_ext_methods};

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
        stack.push_bool_as_int(true);
    } else {
        *ctx.assert_failure = Some(AssertFailure {
            left,
            right,
            left_type: left_name,
            right_type: right_name,
            message: Some(message),
            location: Some(location),
        });
        stack.push_bool_as_int(false);
    }
}

pub fn register_extensions(executor: &mut Executor, ctx: *mut std::ffi::c_void) {
    register_ext_methods!(executor, ctx, {
        4 => assert_equal,
    });
}

pub fn register_get_extensions(executor: &mut GetExecutor, ctx: *mut std::ffi::c_void) {
    register_ext_methods!(executor, ctx, {
        4 => assert_equal,
    });
}
