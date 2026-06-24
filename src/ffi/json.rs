use crate::context::Context;
use num_bigint::BigInt;
use serde_json::Value;
use std::str::FromStr;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvm_ffi::stack::{Tuple, TupleItem};

/// Parses `src` as a JSON document and returns the value at the given
/// JSON Pointer (RFC 6901, e.g. `/token/decimals` or `/items/0`).
///
/// Returns `None` when `src` is not valid JSON or when the pointer does not
/// resolve to a value. An empty pointer (`""`) selects the whole document.
fn lookup(src: &str, pointer: &str) -> Option<Value> {
    let root: Value = serde_json::from_str(src).ok()?;
    root.pointer(pointer).cloned()
}

/// Parses an integer from a JSON string value: decimal, or hexadecimal when
/// prefixed with `0x`/`0X`. Mirrors the parsing accepted by `env<int>`.
fn parse_int_str(raw: &str) -> Option<BigInt> {
    let raw = raw.trim();
    if let Ok(value) = BigInt::from_str(raw) {
        return Some(value);
    }
    let hex = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X"))?;
    BigInt::parse_bytes(hex.as_bytes(), 16)
}

extension!(json_get_int in (Context) with (pointer: String, src: String) using json_get_int_impl);
fn json_get_int_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    pointer: String,
    src: String,
) -> anyhow::Result<()> {
    // NOTE: stack is LIFO, so the host receives the Tolk arguments reversed:
    // Tolk `json.getInt(src, pointer)` -> Rust `(pointer, src)`.
    let parsed = lookup(&src, &pointer).and_then(|value| match value {
        // Bare JSON integers. Floats fail `BigInt::from_str` and yield `null`.
        // Values beyond 64-bit should be encoded as JSON strings (see below),
        // since serde_json parses oversized bare numbers as lossy floats.
        Value::Number(number) => BigInt::from_str(&number.to_string()).ok(),
        // Quoted numbers (decimal or 0x-hex). This is the lossless way to carry
        // full 257-bit TON integers through JSON.
        Value::String(text) => parse_int_str(&text),
        _ => None,
    });
    match parsed {
        Some(value) => stack.push(TupleItem::Int(value)),
        None => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(json_get_string in (Context) with (pointer: String, src: String) using json_get_string_impl);
fn json_get_string_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    pointer: String,
    src: String,
) -> anyhow::Result<()> {
    match lookup(&src, &pointer) {
        Some(Value::String(text)) => stack.push_string(&text),
        _ => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(json_get_bool in (Context) with (pointer: String, src: String) using json_get_bool_impl);
fn json_get_bool_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    pointer: String,
    src: String,
) -> anyhow::Result<()> {
    match lookup(&src, &pointer) {
        Some(Value::Bool(flag)) => stack.push_bool(flag),
        _ => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(json_exists in (Context) with (pointer: String, src: String) using json_exists_impl);
fn json_exists_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    pointer: String,
    src: String,
) -> anyhow::Result<()> {
    stack.push_bool(lookup(&src, &pointer).is_some());
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        300 => json_get_int : 2,
        301 => json_get_string : 2,
        302 => json_get_bool : 2,
        303 => json_exists : 2,
    });
}
