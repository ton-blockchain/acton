use crate::common::{run_get_method, run_get_method_with_stack};
use crate::content::{merge_token_content, parse_token_content, token_content_uri};
use num_bigint::BigInt;
use serde_json::Value;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::cell::{Cell, CellBuilder};
use tycho_types::models::IntAddr;

const JETTON_CONTENT_KEYS: &[&str] = &[
    "uri",
    "name",
    "description",
    "image",
    "image_data",
    "symbol",
    "decimals",
    "amount_style",
    "render_type",
];

#[derive(Debug, Clone, tvm_ffi::FromStackTuple)]
pub struct JettonData {
    pub total_supply: BigInt,
    pub mintable: bool,
    pub admin_address: Option<IntAddr>,
    pub jetton_content: Cell,
    pub jetton_wallet_code: Cell,
}

#[derive(Debug, Clone, tvm_ffi::FromStackTuple)]
pub struct JettonWalletData {
    pub balance: BigInt,
    pub owner_address: IntAddr,
    pub jetton_master_address: IntAddr,
    pub jetton_wallet_code: Cell,
}

#[derive(tvm_ffi::FromStackTuple)]
struct MintlessClaimData {
    is_claimed: bool,
}

#[derive(tvm_ffi::FromStackTuple)]
struct JettonWalletAddress {
    address: IntAddr,
}

#[must_use]
pub fn get_jetton_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<JettonData> {
    run_get_method(address, code, data, libs, "get_jetton_data").ok()
}

#[must_use]
pub fn get_jetton_wallet_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<JettonWalletData> {
    run_get_method(address, code, data, libs, "get_wallet_data").ok()
}

pub fn get_jetton_wallet_address(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
    owner_address: &IntAddr,
) -> anyhow::Result<IntAddr> {
    let stack = Tuple(vec![TupleItem::Slice(CellBuilder::build_from(
        owner_address,
    )?)]);
    let result: JettonWalletAddress =
        run_get_method_with_stack(address, code, data, libs, "get_wallet_address", stack)?;
    Ok(result.address)
}

#[must_use]
pub fn get_mintless_is_claimed(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<bool> {
    run_get_method::<MintlessClaimData>(address, code, data, libs, "is_claimed")
        .ok()
        .map(|data| data.is_claimed)
}

#[must_use]
pub fn parse_jetton_content(content_cell: Cell) -> Value {
    parse_token_content(content_cell, JETTON_CONTENT_KEYS)
}

#[must_use]
pub fn jetton_content_uri(content: &Value) -> Option<&str> {
    token_content_uri(content)
}

pub fn merge_jetton_content(content: &mut Value, remote_content: &Value) {
    merge_token_content(content, remote_content, JETTON_CONTENT_KEYS);
}
