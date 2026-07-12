use crate::toncenter::v3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulateTraceResponse {
    pub mc_block_seqno: u32,
    pub trace: v3::TraceNode,
    pub transactions: HashMap<String, v3::Transaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<v3::Action>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_cells: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_cells: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address_book: Option<v3::AddressBook>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<v3::Metadata>,
    pub rand_seed: String,
    pub is_incomplete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}
