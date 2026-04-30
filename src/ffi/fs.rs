use crate::context::Context;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvm_ffi::stack::{Tuple, TupleItem};

extension!(read_file in (Context) with (path: String) using read_file_impl);
fn read_file_impl(ctx: &mut Context, stack: &mut Tuple, path: String) -> anyhow::Result<()> {
    let Some(path) = ctx.resolve_project_read_path(&path) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    match std::fs::read_to_string(path) {
        Ok(content) => stack.push_string(&content),
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(read_bytes in (Context) with (path: String) using read_bytes_impl);
fn read_bytes_impl(ctx: &mut Context, stack: &mut Tuple, path: String) -> anyhow::Result<()> {
    let Some(path) = ctx.resolve_project_read_path(&path) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    match std::fs::read(path) {
        Ok(content) => stack.push_bytes(&content),
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(write_string in (Context) with (data: String, path: String) using write_string_impl);
fn write_string_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    data: String,
    path: String,
) -> anyhow::Result<()> {
    let success = ctx
        .resolve_project_write_path(&path)
        .is_some_and(|path| std::fs::write(path, data).is_ok());
    stack.push_bool(success);
    Ok(())
}

extension!(write_bytes in (Context) with (data: TupleItem, path: String) using write_bytes_impl);
fn write_bytes_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    data: TupleItem,
    path: String,
) -> anyhow::Result<()> {
    let Some(path) = ctx.resolve_project_write_path(&path) else {
        stack.push_bool(false);
        return Ok(());
    };

    let data = match data {
        TupleItem::Slice(cell) | TupleItem::Cell(cell) => Tuple::parse_snake_bytes(&cell),
        _ => None,
    };
    let success = data.is_some_and(|bytes| std::fs::write(path, bytes).is_ok());
    stack.push_bool(success);
    Ok(())
}

extension!(path_exists in (Context) with (path: String) using path_exists_impl);
fn path_exists_impl(ctx: &mut Context, stack: &mut Tuple, path: String) -> anyhow::Result<()> {
    stack.push_bool(
        ctx.resolve_project_read_path(&path)
            .is_some_and(|path| path.exists()),
    );
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        3 => read_file : 1,
        4 => read_bytes : 1,
        5 => write_string : 2,
        7 => write_bytes : 2,
        22 => path_exists : 1,
    });
}
