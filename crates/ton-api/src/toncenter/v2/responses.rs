use super::{StringOrNumber, TvmCell, TvmStackEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tvm_ffi::json_stack::{json_to_legacy_stack, std_stack_into_tuple};

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
    RunGetMethodStd(Box<RunGetMethodStdResult>),
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
    pub account_state: String,
    pub last_transaction_id: InternalTransactionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seqno: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_id: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_signature_allowed: Option<bool>,
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
    #[serde(rename = "raw.accountState")]
    Raw {
        code: String,
        data: String,
        frozen_hash: String,
    },
    #[serde(rename = "wallet.v3.accountState")]
    WalletV3 { wallet_id: i64, seqno: i32 },
    #[serde(rename = "wallet.v4.accountState")]
    WalletV4 { wallet_id: i64, seqno: i32 },
    #[serde(rename = "wallet.highload.v1.accountState")]
    WalletHighloadV1 { wallet_id: i64, seqno: i32 },
    #[serde(rename = "wallet.highload.v2.accountState")]
    WalletHighloadV2 { wallet_id: i64 },
    #[serde(rename = "dns.accountState")]
    Dns { wallet_id: i64 },
    #[serde(rename = "rwallet.accountState")]
    RWallet {
        wallet_id: i64,
        seqno: i32,
        unlocked_balance: i64,
        config: RWalletConfig,
    },
    #[serde(rename = "pchan.accountState")]
    PChan {
        config: PChanConfig,
        state: PChanState,
        description: String,
    },
    #[serde(rename = "uninited.accountState")]
    Uninited { frozen_hash: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RWalletLimit {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub seconds: i32,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RWalletConfig {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub start_at: i64,
    pub limits: Vec<RWalletLimit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PChanConfig {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub alice_public_key: String,
    pub alice_address: AccountAddress,
    pub bob_public_key: String,
    pub bob_address: AccountAddress,
    pub init_timeout: i32,
    pub close_timeout: i32,
    pub channel_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum PChanState {
    #[serde(rename = "pchan.stateInit")]
    Init {
        #[serde(rename = "signed_A")]
        signed_a: bool,
        #[serde(rename = "signed_B")]
        signed_b: bool,
        #[serde(rename = "min_A")]
        min_a: i64,
        #[serde(rename = "min_B")]
        min_b: i64,
        expire_at: i64,
        #[serde(rename = "A")]
        a: i64,
        #[serde(rename = "B")]
        b: i64,
    },
    #[serde(rename = "pchan.stateClose")]
    Close {
        #[serde(rename = "signed_A")]
        signed_a: bool,
        #[serde(rename = "signed_B")]
        signed_b: bool,
        #[serde(rename = "min_A")]
        min_a: i64,
        #[serde(rename = "min_B")]
        min_b: i64,
        expire_at: i64,
        #[serde(rename = "A")]
        a: i64,
        #[serde(rename = "B")]
        b: i64,
    },
    #[serde(rename = "pchan.statePayout")]
    Payout {
        #[serde(rename = "A")]
        a: i64,
        #[serde(rename = "B")]
        b: i64,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mintless_is_claimed: Option<bool>,
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
        content: NftContent,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenContent {
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NftContent {
    Token(TokenContent),
    Dns(Box<DnsContent>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsContent {
    pub domain: String,
    pub data: DnsRecordSet,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DnsRecordSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_next_resolver: Option<DnsRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<DnsRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<DnsRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<DnsRecord>,
    #[serde(flatten)]
    pub extra: HashMap<String, DnsRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum DnsRecord {
    #[serde(rename = "dns_storage_address")]
    StorageAddress { bag_id: String },
    #[serde(rename = "dns_smc_address")]
    SmcAddress { smc_addr: SmcAddress },
    #[serde(rename = "dns_adnl_address")]
    AdnlAddress { adnl_addr: String },
    #[serde(rename = "dns_next_resolver")]
    NextResolver { resolver: SmcAddress },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmcAddress {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub workchain_id: i32,
    pub address: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_decode_error: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunGetMethodStdResult {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub gas_used: i64,
    pub stack: Vec<TvmStackEntry>,
    pub exit_code: i32,
}

impl RunGetMethodResult {
    pub fn parse_stack_tuple(&self) -> anyhow::Result<tvm_ffi::stack::Tuple> {
        json_to_legacy_stack(self.stack.clone())
    }
}

impl RunGetMethodStdResult {
    pub fn parse_stack_tuple(&self) -> anyhow::Result<tvm_ffi::stack::Tuple> {
        std_stack_into_tuple(self.stack.clone())
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

    #[test]
    fn wallet_information_deserializes_upstream_optional_fields() {
        let fixtures = [
            serde_json::json!({
                "@type": "ext.accounts.walletInformation",
                "wallet": false,
                "balance": "0",
                "account_state": "uninitialized",
                "last_transaction_id": {
                    "@type": "internal.transactionId",
                    "lt": "0",
                    "hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                }
            }),
            serde_json::json!({
                "@type": "ext.accounts.walletInformation",
                "wallet": true,
                "balance": "1000000000",
                "account_state": "active",
                "last_transaction_id": {
                    "@type": "internal.transactionId",
                    "lt": "7",
                    "hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                },
                "wallet_type": "wallet v5 r1",
                "seqno": -2147483648_i64,
                "wallet_id": -3,
                "is_signature_allowed": false
            }),
        ];

        for fixture in fixtures {
            let parsed: WalletInformation = serde_json::from_value(fixture.clone())
                .expect("wallet information must deserialize");
            assert_eq!(
                serde_json::to_value(parsed).expect("wallet information must serialize"),
                fixture
            );
        }
    }

    #[test]
    fn account_state_kind_round_trips_all_openapi_variants() {
        let account = serde_json::json!({
            "@type": "accountAddress",
            "account_address": "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ"
        });
        let config = serde_json::json!({
            "@type": "pchan.config",
            "alice_public_key": "alice",
            "alice_address": account,
            "bob_public_key": "bob",
            "bob_address": account,
            "init_timeout": 10,
            "close_timeout": 20,
            "channel_id": 30
        });
        let fixtures = [
            serde_json::json!({
                "@type": "raw.accountState",
                "code": "",
                "data": "",
                "frozen_hash": ""
            }),
            serde_json::json!({
                "@type": "wallet.v3.accountState",
                "wallet_id": 1,
                "seqno": -1
            }),
            serde_json::json!({
                "@type": "wallet.v4.accountState",
                "wallet_id": 2,
                "seqno": 3
            }),
            serde_json::json!({
                "@type": "wallet.highload.v1.accountState",
                "wallet_id": 4,
                "seqno": 5
            }),
            serde_json::json!({
                "@type": "wallet.highload.v2.accountState",
                "wallet_id": 6
            }),
            serde_json::json!({"@type": "dns.accountState", "wallet_id": 7}),
            serde_json::json!({
                "@type": "rwallet.accountState",
                "wallet_id": 8,
                "seqno": 9,
                "unlocked_balance": 10,
                "config": {
                    "@type": "rwallet.config",
                    "start_at": 11,
                    "limits": [{"@type": "rwallet.limit", "seconds": 12, "value": 13}]
                }
            }),
            serde_json::json!({
                "@type": "pchan.accountState",
                "config": config,
                "state": {
                    "@type": "pchan.stateInit",
                    "signed_A": true,
                    "signed_B": false,
                    "min_A": 1,
                    "min_B": 2,
                    "expire_at": 3,
                    "A": 4,
                    "B": 5
                },
                "description": "init"
            }),
            serde_json::json!({
                "@type": "pchan.accountState",
                "config": config,
                "state": {
                    "@type": "pchan.stateClose",
                    "signed_A": false,
                    "signed_B": true,
                    "min_A": 6,
                    "min_B": 7,
                    "expire_at": 8,
                    "A": 9,
                    "B": 10
                },
                "description": "close"
            }),
            serde_json::json!({
                "@type": "pchan.accountState",
                "config": config,
                "state": {"@type": "pchan.statePayout", "A": 11, "B": 12},
                "description": "payout"
            }),
            serde_json::json!({"@type": "uninited.accountState", "frozen_hash": ""}),
        ];

        for fixture in fixtures {
            let parsed: AccountStateKind =
                serde_json::from_value(fixture.clone()).expect("account state must deserialize");
            assert_eq!(
                serde_json::to_value(parsed).expect("account state must serialize"),
                fixture
            );
        }
    }

    #[test]
    fn token_data_round_trips_mintless_wallet_and_dns_nft() {
        let fixtures = [
            serde_json::json!({
                "@type": "ext.tokens.jettonWalletData",
                "address": "0:1111111111111111111111111111111111111111111111111111111111111111",
                "contract_type": "jetton_wallet",
                "balance": "1000",
                "owner": "0:2222222222222222222222222222222222222222222222222222222222222222",
                "jetton": "0:3333333333333333333333333333333333333333333333333333333333333333",
                "mintless_is_claimed": false,
                "jetton_wallet_code": "te6ccgEBAQEAAgAAAA=="
            }),
            serde_json::json!({
                "@type": "ext.tokens.nftItemData",
                "address": "0:1111111111111111111111111111111111111111111111111111111111111111",
                "contract_type": "nft_item",
                "init": true,
                "index": "7",
                "collection_address": "0:2222222222222222222222222222222222222222222222222222222222222222",
                "owner_address": "0:3333333333333333333333333333333333333333333333333333333333333333",
                "content": {
                    "domain": "example.ton",
                    "data": {
                        "dns_next_resolver": {
                            "@type": "dns_next_resolver",
                            "resolver": {
                                "@type": "addr_std",
                                "workchain_id": 0,
                                "address": "4444444444444444444444444444444444444444444444444444444444444444"
                            }
                        },
                        "wallet": {
                            "@type": "dns_smc_address",
                            "smc_addr": {
                                "@type": "addr_std",
                                "workchain_id": 0,
                                "address": "5555555555555555555555555555555555555555555555555555555555555555"
                            }
                        },
                        "site": {
                            "@type": "dns_adnl_address",
                            "adnl_addr": "6666666666666666666666666666666666666666666666666666666666666666"
                        },
                        "storage": {
                            "@type": "dns_storage_address",
                            "bag_id": "7777777777777777777777777777777777777777777777777777777777777777"
                        }
                    }
                }
            }),
        ];

        for fixture in fixtures {
            let parsed: TokenData =
                serde_json::from_value(fixture.clone()).expect("token data must deserialize");
            assert_eq!(
                serde_json::to_value(parsed).expect("token data must serialize"),
                fixture
            );
        }
    }
}
