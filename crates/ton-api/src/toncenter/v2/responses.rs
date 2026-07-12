use super::StringOrNumber;
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tvm_ffi::json_stack::{json_to_legacy_stack, json_to_stack};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonlibResponse<T> {
    pub ok: bool,
    pub result: T,
    #[serde(rename = "@extra")]
    pub extra: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    /// `TonCenter` accepts JSON-RPC requests but may omit JSON-RPC metadata in responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jsonrpc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<StringOrNumber>,
    #[serde(flatten)]
    pub response: TonlibResponse<T>,
}

impl<T> JsonRpcResponse<T> {
    pub fn into_result(self) -> T {
        self.response.result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcResult {
    Ok(Box<ResultOk>),
    ExternalMessage(Box<ExtMessageInfo>),
    RunGetMethod(Box<RunGetMethodResult>),
    DetectAddress(Box<DetectAddress>),
    DetectHash(Box<DetectHash>),
    String(String),
    AddressInformation(Box<AddressInformation>),
    ShardAccountCell(Box<TvmCell>),
    Libraries(Box<LibraryResult>),
    ExtendedAddressInformation(Box<ExtendedAddressInformation>),
    WalletInformation(Box<WalletInformation>),
    TokenData(Box<TokenData>),
    Transactions(Vec<Transaction>),
    RawTransactions(Box<RawTransactions>),
    ConfigInfo(Box<ConfigInfo>),
    Transaction(Box<Transaction>),
    BlockHeader(Box<BlockHeader>),
    BlockTransactions(Box<BlockTransactions>),
    BlockTransactionsExt(Box<BlockTransactionsExt>),
    MasterchainInfo(Box<MasterchainInfo>),
    ConsensusBlock(Box<ConsensusBlock>),
    OutMsgQueueSizes(Box<OutMsgQueueSizes>),
    Shards(Box<Shards>),
    BlockId(Box<TonBlockIdExt>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonBlockIdExt {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub workchain: i32,
    pub shard: String,
    pub seqno: u64,
    pub root_hash: String,
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterchainInfo {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub last: TonBlockIdExt,
    pub state_root_hash: String,
    pub init: TonBlockIdExt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectAddressBase64Variant {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub b64: String,
    pub b64url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectAddress {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub raw_form: String,
    pub bounceable: DetectAddressBase64Variant,
    pub non_bounceable: DetectAddressBase64Variant,
    pub given_type: String,
    pub test_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectHash {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub b64: String,
    pub b64url: String,
    pub hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub id: TonBlockIdExt,
    pub global_id: i32,
    pub version: i32,
    pub after_merge: bool,
    pub after_split: bool,
    pub before_split: bool,
    pub want_merge: bool,
    pub want_split: bool,
    pub validator_list_hash_short: i32,
    pub catchain_seqno: i32,
    pub min_ref_mc_seqno: i32,
    pub is_key_block: bool,
    pub prev_key_block_seqno: i32,
    pub start_lt: String,
    pub end_lt: String,
    pub gen_utime: u32,
    pub prev_blocks: Vec<TonBlockIdExt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultOk {
    #[serde(rename = "@type")]
    pub type_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountAddress {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub account_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInformation {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub wallet: bool,
    pub balance: String,
    pub extra_currencies: Vec<ExtraCurrencyBalance>,
    pub account_state: String,
    pub last_transaction_id: InternalTransactionId,
    pub wallet_type: Option<String>,
    pub seqno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedAddressInformation {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub address: AccountAddress,
    pub balance: String,
    pub extra_currencies: Vec<ExtraCurrencyBalance>,
    pub last_transaction_id: InternalTransactionId,
    pub block_id: TonBlockIdExt,
    pub sync_utime: u64,
    pub account_state: AccountStateKind,
    pub revision: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum AccountStateKind {
    #[serde(rename = "uninited.accountState")]
    Uninited { frozen_hash: String },
    #[serde(rename = "raw.accountState")]
    Raw {
        code: String,
        data: String,
        frozen_hash: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum TokenData {
    #[serde(rename = "ext.tokens.jettonMasterData")]
    JettonMaster {
        address: String,
        contract_type: String,
        total_supply: String,
        mintable: bool,
        admin_address: Option<String>,
        jetton_content: TokenContent,
        jetton_wallet_code: String,
    },
    #[serde(rename = "ext.tokens.jettonWalletData")]
    JettonWallet {
        address: String,
        contract_type: String,
        balance: String,
        owner: String,
        jetton: String,
        jetton_wallet_code: String,
    },
    #[serde(rename = "ext.tokens.nftCollectionData")]
    NftCollection {
        address: String,
        contract_type: String,
        next_item_index: String,
        owner_address: Option<String>,
        collection_content: TokenContent,
    },
    #[serde(rename = "ext.tokens.nftItemData")]
    NftItem {
        address: String,
        contract_type: String,
        init: bool,
        index: String,
        collection_address: Option<String>,
        owner_address: Option<String>,
        content: TokenContent,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenContent {
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTransactions {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub transactions: Vec<RawTransaction>,
    pub previous_transaction_id: InternalTransactionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTransaction {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub address: AccountAddress,
    pub utime: u64,
    pub data: String,
    pub transaction_id: InternalTransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<MessageStd>,
    pub out_msgs: Vec<MessageStd>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionExt {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub address: AccountAddress,
    pub account: String,
    pub utime: u64,
    pub data: String,
    pub transaction_id: InternalTransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<MessageStd>,
    pub out_msgs: Vec<MessageStd>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum Message {
    #[serde(rename = "msg.message")]
    Empty,
    #[serde(rename = "ext.message")]
    Full(Box<MessageFull>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFull {
    pub hash: String,
    pub source: String,
    pub destination: String,
    pub value: String,
    pub fwd_fee: String,
    pub ihr_fee: String,
    pub created_lt: String,
    pub body_hash: String,
    pub msg_data: MessageData,
    pub extra_currencies: Vec<ExtraCurrencyBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStd {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub hash: String,
    pub source: AccountAddress,
    pub destination: AccountAddress,
    pub value: String,
    pub fwd_fee: String,
    pub ihr_fee: String,
    pub created_lt: String,
    pub body_hash: String,
    pub msg_data: MessageData,
    pub extra_currencies: Vec<ExtraCurrencyBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum MessageData {
    #[serde(rename = "msg.dataRaw")]
    Raw { body: String, init_state: String },
    #[serde(rename = "msg.dataText")]
    Text { text: String },
    #[serde(rename = "msg.dataDecryptedText")]
    DecryptedText { text: String },
    #[serde(rename = "msg.dataEncryptedText")]
    EncryptedText { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTransactionsExt {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub id: TonBlockIdExt,
    pub req_count: usize,
    pub incomplete: bool,
    pub transactions: Vec<TransactionExt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTransactions {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub id: TonBlockIdExt,
    pub req_count: usize,
    pub incomplete: bool,
    pub transactions: Vec<ShortTxId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortTxId {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub mode: i32,
    pub account: String,
    pub lt: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusBlock {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub consensus_block: u32,
    pub timestamp: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shards {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub shards: Vec<TonBlockIdExt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutMsgQueueSizes {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub shards: Vec<OutMsgQueueSize>,
    pub ext_msg_queue_size_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutMsgQueueSize {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub id: TonBlockIdExt,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalMessageInfo {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtMessageInfo {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub hash: String,
    pub hash_norm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvmCell {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub bytes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigInfo {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub config: TvmCell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryResult {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub result: Vec<LibraryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub hash: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInformation {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub balance: StringOrNumber,
    pub extra_currencies: Vec<ExtraCurrencyBalance>,
    pub code: String,
    pub data: String,
    pub state: String,
    pub frozen_hash: String,
    pub last_transaction_id: InternalTransactionId,
    pub block_id: TonBlockIdExt,
    pub sync_utime: u64,
    pub suspended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraCurrencyBalance {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub id: i32,
    pub amount: StringOrNumber,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalTransactionId {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub lt: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunGetMethodResult {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub gas_used: StringOrNumber,
    pub stack: Vec<Value>,
    pub exit_code: i32,
    pub block_id: TonBlockIdExt,
    pub last_transaction_id: InternalTransactionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_log: Option<String>,
}

impl RunGetMethodResult {
    pub fn parse_stack_tuple(&self) -> anyhow::Result<tvm_ffi::stack::Tuple> {
        match json_to_legacy_stack(self.stack.clone()) {
            Ok(tuple) => Ok(tuple),
            Err(legacy_err) => json_to_stack(self.stack.clone()).with_context(|| {
                format!(
                    "Failed to parse stack as legacy and std formats. Legacy error: {legacy_err}"
                )
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub address: AccountAddress,
    pub account: String,
    pub utime: u64,
    pub data: String,
    pub transaction_id: InternalTransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<Message>,
    pub out_msgs: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonlibErrorResponse {
    pub ok: bool,
    pub error: String,
    pub code: i32,
    #[serde(rename = "@extra")]
    pub extra: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jsonrpc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<StringOrNumber>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_response_deserializes_generic_result() {
        let response: JsonRpcResponse<String> = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": "request-1",
            "ok": true,
            "result": "active",
            "@extra": "metadata"
        }))
        .expect("generic response must deserialize");

        assert_eq!(response.into_result(), "active");
    }

    #[test]
    fn transaction_ext_deserializes_openapi_message_std_and_optional_in_msg() {
        let transaction = serde_json::json!({
            "@type": "raw.transactionExt",
            "address": {
                "@type": "accountAddress",
                "account_address": "0:1111111111111111111111111111111111111111111111111111111111111111"
            },
            "account": "0:1111111111111111111111111111111111111111111111111111111111111111",
            "utime": 1,
            "data": "te6ccgEBAQEAAgAAAA==",
            "transaction_id": {
                "@type": "internal.transactionId",
                "lt": "1",
                "hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
            },
            "fee": "0",
            "storage_fee": "0",
            "other_fee": "0",
            "in_msg": {
                "@type": "raw.message",
                "hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "source": {"@type": "accountAddress", "account_address": ""},
                "destination": {
                    "@type": "accountAddress",
                    "account_address": "0:1111111111111111111111111111111111111111111111111111111111111111"
                },
                "value": "0",
                "fwd_fee": "0",
                "ihr_fee": "0",
                "created_lt": "0",
                "body_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "msg_data": {"@type": "msg.dataRaw", "body": "", "init_state": ""},
                "extra_currencies": []
            },
            "out_msgs": []
        });

        let parsed: TransactionExt =
            serde_json::from_value(transaction.clone()).expect("transactionExt must deserialize");
        assert_eq!(parsed.type_field, "raw.transactionExt");
        assert_eq!(
            parsed
                .in_msg
                .as_ref()
                .map(|message| message.type_field.as_str()),
            Some("raw.message")
        );

        let mut without_in_msg = transaction;
        without_in_msg
            .as_object_mut()
            .expect("fixture must be object")
            .remove("in_msg");
        let parsed: TransactionExt = serde_json::from_value(without_in_msg)
            .expect("transactionExt without in_msg must deserialize");
        assert!(parsed.in_msg.is_none());
    }

    #[test]
    fn message_data_deserializes_all_openapi_variants() {
        let fixtures = [
            serde_json::json!({
                "@type": "msg.dataRaw",
                "body": "te6ccgEBAQEAAgAAAA==",
                "init_state": ""
            }),
            serde_json::json!({"@type": "msg.dataText", "text": "aGVsbG8="}),
            serde_json::json!({"@type": "msg.dataDecryptedText", "text": "aGVsbG8="}),
            serde_json::json!({"@type": "msg.dataEncryptedText", "text": "aGVsbG8="}),
        ];

        for fixture in fixtures {
            let parsed: MessageData =
                serde_json::from_value(fixture.clone()).expect("message data must deserialize");
            assert_eq!(
                serde_json::to_value(parsed).expect("message data must serialize"),
                fixture
            );
        }
    }
}
