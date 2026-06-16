use crate::common::run_get_method;
use crate::types::Map;
use num_bigint::BigInt;
use tycho_types::cell::Cell;
use tycho_types::models::IntAddr;

#[derive(Debug, Clone, tvm_ffi::FromStackTuple)]
pub struct MultisigData {
    pub next_order_seqno: BigInt,
    pub threshold: BigInt,
    pub signers: Map<u8, IntAddr>,
    pub proposers: Map<u8, IntAddr>,
}

#[must_use]
pub fn get_multisig_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<MultisigData> {
    get_multisig_data_result(address, code, data, libs).ok()
}

pub fn get_multisig_data_result(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> anyhow::Result<MultisigData> {
    run_get_method(address, code, data, libs, "get_multisig_data")
}
