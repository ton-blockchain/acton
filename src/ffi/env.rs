use crate::context::Context;
use num_bigint::BigInt;
use std::env;
use std::str::FromStr;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::models::{StdAddr, StdAddrFormat};

extension!(env_int in (Context) with (name: String) using env_int_impl);
fn env_int_impl(_ctx: &mut Context, stack: &mut Tuple, name: String) -> anyhow::Result<()> {
    match env::var(&name) {
        Ok(val) => {
            if let Ok(num) = BigInt::from_str(&val) {
                stack.push(TupleItem::Int(num));
            } else if let Some(val) = val.strip_prefix("0x") {
                if let Some(num) = BigInt::parse_bytes(val.as_bytes(), 16) {
                    stack.push(TupleItem::Int(num));
                } else {
                    stack.push(TupleItem::Null);
                }
            } else {
                stack.push(TupleItem::Null);
            }
        }
        Err(_) => stack.push(TupleItem::Null),
    }

    Ok(())
}

extension!(env_bool in (Context) with (name: String) using env_bool_impl);
fn env_bool_impl(_ctx: &mut Context, stack: &mut Tuple, name: String) -> anyhow::Result<()> {
    match env::var(&name) {
        Ok(val) => {
            stack.push_bool(val == "1" || val.eq_ignore_ascii_case("true"));
        }
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(env_string in (Context) with (name: String) using env_string_impl);
fn env_string_impl(_ctx: &mut Context, stack: &mut Tuple, name: String) -> anyhow::Result<()> {
    match env::var(&name) {
        Ok(val) => {
            stack.push_string(&val);
        }
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(env_slice in (Context) with (name: String) using env_slice_impl);
fn env_slice_impl(_ctx: &mut Context, stack: &mut Tuple, name: String) -> anyhow::Result<()> {
    match env::var(&name) {
        Ok(val) => {
            stack.push_string(&val);
        }
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(env_address in (Context) with (name: String) using env_address_impl);
fn env_address_impl(_ctx: &mut Context, stack: &mut Tuple, name: String) -> anyhow::Result<()> {
    match env::var(&name) {
        Ok(val) => {
            if let Ok((addr, _)) = StdAddr::from_str_ext(&val, StdAddrFormat::any()) {
                let mut builder = CellBuilder::new();
                if addr.store_into(&mut builder, Cell::empty_context()).is_ok()
                    && let Ok(cell) = builder.build()
                {
                    stack.push(TupleItem::Slice(cell));
                    return Ok(());
                }
            }
            stack.push(TupleItem::Null);
        }
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(env_cell in (Context) with (name: String) using env_cell_impl);
fn env_cell_impl(_ctx: &mut Context, stack: &mut Tuple, name: String) -> anyhow::Result<()> {
    match env::var(&name) {
        Ok(val) => {
            let cell = if let Ok(b) = Boc::decode_base64(&val) {
                Some(b)
            } else {
                Boc::decode_hex(&val).ok()
            };

            if let Some(cell) = cell {
                stack.push(TupleItem::Cell(cell));
                return Ok(());
            }
            stack.push(TupleItem::Null);
        }
        Err(_) => stack.push(TupleItem::Null),
    }
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        50 => env_int : 1,
        51 => env_bool : 1,
        52 => env_string : 1,
        53 => env_address : 1,
        54 => env_cell : 1,
        55 => env_slice : 1,
    });
}
