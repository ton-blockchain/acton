use crate::context::Context;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::{Boc, ser::BocHeader};

extension!(encode in (Context) with (crc32: bool, value: TupleItem) using encode_impl);
fn encode_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    crc32: bool,
    value: TupleItem,
) -> anyhow::Result<()> {
    let (TupleItem::Cell(cell) | TupleItem::Slice(cell)) = value else {
        anyhow::bail!("boc.encode expects a cell or slice")
    };

    let mut bytes = Vec::new();
    BocHeader::<std::collections::hash_map::RandomState>::with_root(cell.as_ref())
        .with_crc(crc32)
        .encode(&mut bytes);
    stack.push_bytes(&bytes);
    Ok(())
}

extension!(decode in (Context) with (data: TupleItem) using decode_impl);
fn decode_impl(_ctx: &mut Context, stack: &mut Tuple, data: TupleItem) -> anyhow::Result<()> {
    let bytes = match data {
        TupleItem::Cell(cell) | TupleItem::Slice(cell) => Tuple::parse_snake_bytes(&cell),
        _ => None,
    };

    match bytes.and_then(|bytes| Boc::decode(bytes).ok()) {
        Some(cell) => stack.push(TupleItem::Cell(cell)),
        None => stack.push(TupleItem::Null),
    }
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        62 => encode : 2,
        63 => decode : 1,
    });
}
