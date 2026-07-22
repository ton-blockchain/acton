use crate::common::run_get_method;
use crate::content::parse_token_content;
use num_bigint::BigInt;
use serde_json::Value;
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

#[must_use]
pub fn get_nft_item_data(address: String, code: Cell, data: Cell) -> Option<NftItemData> {
    run_get_method(address, code, data, None, "get_nft_data").ok()
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

#[must_use]
pub fn parse_nft_content(content_cell: Cell) -> Value {
    parse_token_content(content_cell, NFT_CONTENT_KEYS)
}
