use crate::context::Context;
use num_bigint::BigInt;
use std::env;
use std::str::FromStr;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::{Tuple, TupleItem};

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
            let v = val.to_lowercase();
            if v == "true" || v == "1" {
                stack.push_bool(true);
            } else {
                stack.push_bool(false);
            }
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
            if let Ok(addr) = TonAddress::from_str(&val) {
                let mut builder = CellBuilder::new();
                if builder.store_address(&addr).is_ok()
                    && let Ok(cell) = builder.build()
                {
                    stack.push(TupleItem::Slice(cell.to_arc()));
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
            let cell = if let Ok(b) = ArcCell::from_boc_b64(&val) {
                Some(b)
            } else {
                ArcCell::from_boc_hex(&val).ok()
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
        50 => env_int,
        51 => env_bool,
        52 => env_slice,
        53 => env_address,
        54 => env_cell,
    });
}
