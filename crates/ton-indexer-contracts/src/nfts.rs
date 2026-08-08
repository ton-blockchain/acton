use crate::common::{run_get_method, run_get_method_with_stack};
use crate::content::parse_token_content;
use num_bigint::BigInt;
use serde_json::Value;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::cell::Cell;
use tycho_types::models::IntAddr;

const NFT_CONTENT_KEYS: &[&str] = &[
    "uri",
    "name",
    "description",
    "image",
    "image_data",
    "attributes",
    "cover_image",
    "animation_url",
    "external_url",
    "marketplace",
    "social_links",
];

#[derive(Debug, Clone, tvm_ffi::FromStackTuple)]
pub struct NftItemData {
    pub init: bool,
    pub index: BigInt,
    pub collection_address: Option<IntAddr>,
    pub owner_address: Option<IntAddr>,
    pub individual_content: Cell,
}

#[derive(Debug, Clone, tvm_ffi::FromStackTuple)]
pub struct NftCollectionData {
    pub next_item_index: BigInt,
    pub collection_content: Cell,
    pub owner_address: Option<IntAddr>,
}

#[derive(tvm_ffi::FromStackTuple)]
struct NftAddress {
    address: IntAddr,
}

#[derive(tvm_ffi::FromStackTuple)]
struct NftContent {
    content: Cell,
}

#[must_use]
pub fn get_nft_item_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<NftItemData> {
    run_get_method(address, code, data, libs, "get_nft_data").ok()
}

#[must_use]
pub fn get_nft_collection_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<NftCollectionData> {
    run_get_method(address, code, data, libs, "get_collection_data").ok()
}

pub fn get_nft_address_by_index(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
    index: BigInt,
) -> anyhow::Result<IntAddr> {
    let stack = Tuple(vec![TupleItem::Int(index)]);
    let result: NftAddress =
        run_get_method_with_stack(address, code, data, libs, "get_nft_address_by_index", stack)?;
    Ok(result.address)
}

pub fn get_nft_content(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
    index: BigInt,
    individual_content: Cell,
) -> anyhow::Result<Cell> {
    let stack = Tuple(vec![
        TupleItem::Int(index),
        TupleItem::Cell(individual_content),
    ]);
    let result: NftContent =
        run_get_method_with_stack(address, code, data, libs, "get_nft_content", stack)?;
    Ok(result.content)
}

#[must_use]
pub fn parse_nft_content(content_cell: Cell) -> Value {
    parse_token_content(content_cell, NFT_CONTENT_KEYS)
}
