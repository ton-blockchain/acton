use crate::context::Context;
use emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvmffi::stack::{Tuple, TupleItem};

extension!(read_file in (Context) with (path: String) using read_file_impl);
fn read_file_impl(_ctx: &mut Context, stack: &mut Tuple, path: String) -> anyhow::Result<()> {
    match std::fs::read_to_string(&path) {
        Ok(content) => stack.push_string(&content),
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        3 => read_file,
    });
}
