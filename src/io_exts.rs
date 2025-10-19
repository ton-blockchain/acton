use crate::context::Context;
use emulator::executor::Executor;
use emulator::get_executor::GetExecutor;
use emulator::tuple::stack::{Tuple, TupleItem};
use emulator::{extension, pop_args, register_ext_methods};

extension!(print in (Context) with (s: TupleItem, type_name: String) using print_impl);
fn print_impl(ctx: &mut Context, _stack: &mut Tuple, s: TupleItem, type_name: String) {
    let typed_tuple = if let TupleItem::Tuple(tuple) = &s {
        TupleItem::TypedTuple {
            abi: ctx.abi.find_type(&type_name),
            items: tuple.clone(),
            type_name,
        }
    } else {
        s
    };

    if ctx.capture_test_output {
        ctx.stdout_buffer.push_str(&format!("{}\n", typed_tuple));
    } else {
        println!("{}", typed_tuple);
    }
}

extension!(eprint in (Context) with (s: String) using eprint_impl);
fn eprint_impl(ctx: &mut Context, _stack: &mut Tuple, s: String) {
    if ctx.capture_test_output {
        ctx.stderr_buffer.push_str(&format!("{}\n", s));
    } else {
        eprintln!("{}", s);
    }
}

pub fn register_extensions(executor: &mut Executor, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        1 => print,
        2 => eprint,
    });
}

pub fn register_get_extensions(executor: &mut GetExecutor, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        1 => print,
        2 => eprint,
    });
}
