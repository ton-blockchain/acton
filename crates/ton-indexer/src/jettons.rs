use crate::common::run_get_method;
use crate::content::parse_token_content;
use num_bigint::BigInt;
use serde_json::Value;
use tycho_types::cell::Cell;
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
    pub admin_address: IntAddr,
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

#[must_use]
pub fn parse_jetton_content(content_cell: Cell) -> Value {
    parse_token_content(content_cell, JETTON_CONTENT_KEYS)
}
