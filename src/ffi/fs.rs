use crate::context::Context;
use ton_emulator::{extension, register_ext_methods};
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

extension!(read_bytes in (Context) with (path: String) using read_bytes_impl);
fn read_bytes_impl(_ctx: &mut Context, stack: &mut Tuple, path: String) -> anyhow::Result<()> {
    match std::fs::read(&path) {
        Ok(content) => stack.push_bytes(&content),
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(write_string in (Context) with (data: String, path: String) using write_string_impl);
fn write_string_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    data: String,
    path: String,
) -> anyhow::Result<()> {
    stack.push_bool(std::fs::write(&path, data).is_ok());
    Ok(())
}

extension!(write_bytes in (Context) with (data: TupleItem, path: String) using write_bytes_impl);
fn write_bytes_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    data: TupleItem,
    path: String,
) -> anyhow::Result<()> {
    let data = match data {
        TupleItem::Slice(cell) | TupleItem::Cell(cell) => Tuple::parse_snake_bytes(&cell),
        _ => None,
    };
    let success = data
        .map(|bytes| std::fs::write(&path, bytes).is_ok())
        .unwrap_or(false);
    stack.push_bool(success);
    Ok(())
}

extension!(path_exists in (Context) with (path: String) using path_exists_impl);
fn path_exists_impl(_ctx: &mut Context, stack: &mut Tuple, path: String) -> anyhow::Result<()> {
    stack.push_bool(std::fs::exists(&path).unwrap_or(false));
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
