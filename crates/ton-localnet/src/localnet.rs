use crate::executor::TvmEmulatorAdapter;
use crate::node::{Node, StateSource};
use crate::storage;
use crate::storage::{AccountStatus, BlockMeta, EMPTY_CELL_BASE64, MsgMeta, TransactionInfo};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use anyhow::Context;
use base64::Engine;
use crc::{CRC_16_XMODEM, Crc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use ton_executor::DEFAULT_CONFIG;
use ton_executor::ExecutorVerbosity;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use tvm_ffi::json_stack::json_to_legacy_item;
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{ExtInMsgInfo, Message, MsgInfo, StdAddr, StdAddrFormat};
use tycho_types::num::Tokens;

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);
const MAX_LOOP_REQUESTS: usize = 1024;

pub const DEFAULT_BLOCK_PRODUCTION_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BlockProductionMode {
    #[default]
    Instant,
    Interval {
        block_time: Duration,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetBlockId {
    pub workchain: i32,
    pub shard: i64,
    pub seqno: Seqno,
    pub root_hash: Hash256,
    pub file_hash: Hash256,
}

impl LocalnetBlockId {
    pub const fn first() -> Self {
        Self {
            workchain: 0,
            shard: -9223372036854775808,
            seqno: 0,
            root_hash: Hash256([0; 32]),
            file_hash: Hash256([0; 32]),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetAccountState {
    pub address: Addr,
    pub account_state_hash: Hash256,
    pub balance: u128,
    pub code: Option<BocBytes>,
    pub code_hash: Option<Hash256>,
    pub data: Option<BocBytes>,
    pub data_hash: Option<Hash256>,
    pub last_transaction_id: LocalnetTransactionId,
    pub block_id: LocalnetBlockId,
    pub state: AccountStatus,
    pub sync_utime: u64,
    pub frozen_hash: Option<Hash256>,
}

impl LocalnetAccountState {
    pub fn empty(address: Addr, block_id: LocalnetBlockId, sync_utime: u64) -> Self {
        Self {
            address,
            account_state_hash: Hash256([0; 32]),
            balance: 0,
            code: None,
            code_hash: None,
            data: None,
            data_hash: None,
            last_transaction_id: LocalnetTransactionId::default(),
            block_id,
            state: AccountStatus::Nonexist,
            sync_utime,
            frozen_hash: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LocalnetTransactionId {
    pub lt: Lt,
    pub hash: Hash256,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetTransaction {
    pub hash: Hash256,
    pub address: Addr,
    pub mc_block_seqno: u32,
    pub utime: u32,
    pub data: BocBytes,
    pub success: bool,
    pub exit_code: i32,
    pub transaction_id: LocalnetTransactionId,
    pub in_msg: LocalnetMessage,
    pub out_msgs: Vec<LocalnetMessage>,
    pub total_fees: u128,
    pub storage_fees: u128,
    pub other_fees: u128,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetMessage {
    pub hash: Hash256,
    #[serde(default)]
    pub hash_norm: Option<Hash256>,
    pub source: Option<Addr>,
    pub destination: Option<Addr>,
    pub value: u128,
    pub body_hash: Hash256,
    pub body: BocBytes,
    pub init_state: BocBytes,
    pub opcode: Option<u32>,
    pub fwd_fee: u128,
    pub ihr_fee: u128,
    pub created_lt: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetRunGetMethodResult {
    pub gas_used: u64,
    pub stack: BocBytes,
    pub exit_code: i32,
    pub vm_log: Arc<str>,
    pub block_id: LocalnetBlockId,
    pub last_transaction_id: LocalnetTransactionId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetMasterchainInfo {
    pub last: LocalnetBlockId,
    pub state_root_hash: Hash256,
    pub init: LocalnetBlockId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetConsensusBlock {
    pub consensus_block: Seqno,
    pub timestamp: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetBlockHeader {
    pub id: LocalnetBlockId,
    pub gen_utime: u32,
    pub start_lt: Lt,
    pub end_lt: Lt,
    pub prev_seqno: Option<Seqno>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetBlockTransactions {
    pub id: LocalnetBlockId,
    pub transactions: Vec<LocalnetTransaction>,
    pub msg_hash: Option<Hash256>,
    #[serde(default)]
    pub msg_hash_norm: Option<Hash256>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetLibrary {
    pub hash: Hash256,
    pub found: bool,
    pub data: Option<BocBytes>,
    pub publishers_count: usize,
    pub publishers: Vec<Addr>,
}

#[derive(Debug)]
pub(crate) enum Request {
    SendBoc {
        boc: BocBytes,
        resp: oneshot::Sender<anyhow::Result<LocalnetBlockTransactions>>,
    },
    SendInternalBoc {
        boc: BocBytes,
        resp: oneshot::Sender<anyhow::Result<LocalnetBlockTransactions>>,
    },
    GetAddressInformation {
        address: Addr,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LocalnetAccountState>>,
    },
    GetShardAccountCell {
        address: Addr,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<BocBytes>>,
    },
    SetShardAccount {
        address: Addr,
        shard_account: BocBytes,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    GetTransactions {
        address: Addr,
        limit: usize,
        lt: Option<u64>,
        hash: Option<Hash256>,
        to_lt: Option<u64>,
        resp: oneshot::Sender<anyhow::Result<Vec<LocalnetTransaction>>>,
    },
    GetAllTransactions {
        resp: oneshot::Sender<anyhow::Result<Vec<LocalnetTransaction>>>,
    },
    GetPendingTransactions {
        resp: oneshot::Sender<anyhow::Result<Vec<LocalnetTransaction>>>,
    },
    TryLocateTx {
        source: Addr,
        destination: Addr,
        created_lt: u64,
        resp: oneshot::Sender<anyhow::Result<LocalnetTransaction>>,
    },
    TryLocateResultTx {
        source: Addr,
        destination: Addr,
        created_lt: u64,
        resp: oneshot::Sender<anyhow::Result<LocalnetTransaction>>,
    },
    TryLocateSourceTx {
        source: Addr,
        destination: Addr,
        created_lt: u64,
        resp: oneshot::Sender<anyhow::Result<LocalnetTransaction>>,
    },
    RunGetMethod {
        address: Addr,
        method_id: i32,
        stack: Tuple,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LocalnetRunGetMethodResult>>,
    },
    GetBlockHeader {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<LocalnetBlockHeader>>,
    },
    GetBlockTransactions {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<LocalnetBlockTransactions>>,
    },
    GetMasterchainInfo {
        resp: oneshot::Sender<anyhow::Result<LocalnetMasterchainInfo>>,
    },
    GetConsensusBlock {
        resp: oneshot::Sender<anyhow::Result<LocalnetConsensusBlock>>,
    },
    GetLibraries {
        hashes: Vec<Hash256>,
        resp: oneshot::Sender<anyhow::Result<Vec<LocalnetLibrary>>>,
    },
    GetConfigParam {
        param: u32,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<BocBytes>>,
    },
    GetConfigAll {
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<BocBytes>>,
    },
    GetShards {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<Vec<LocalnetBlockId>>>,
    },
    LookupBlock {
        #[allow(dead_code)] // unused since localnet have only one workchain
        workchain: i32,
        #[allow(dead_code)] // unused since localnet have only one shard
        shard: i64,
        seqno: Option<u32>,
        lt: Option<u64>,
        unixtime: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LocalnetBlockId>>,
    },
    Faucet {
        address: Addr,
        amount: u128,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetTraces {
        tx_hash: Hash256,
        resp: oneshot::Sender<anyhow::Result<storage::TraceNode>>,
    },
    GetTracesByMessageHash {
        msg_hash: Hash256,
        resp: oneshot::Sender<anyhow::Result<storage::TraceNode>>,
    },
    EmulateTrace {
        boc: BocBytes,
        ignore_chksig: bool,
        mc_block_seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<storage::EmulateTraceResult>>,
    },
    GetJettonMasters {
        address: Option<Addr>,
        admin_address: Option<Addr>,
        limit: usize,
        offset: usize,
        resp: oneshot::Sender<anyhow::Result<Vec<storage::JettonMasterMeta>>>,
    },
    GetJettonWallets {
        address: Option<Addr>,
        owner_address: Option<Addr>,
        jetton_address: Option<Addr>,
        exclude_zero_balance: bool,
        limit: usize,
        offset: usize,
        resp: oneshot::Sender<anyhow::Result<Vec<storage::JettonWalletMeta>>>,
    },
    GetNftItems {
        address: Option<Addr>,
        owner_address: Option<Addr>,
        collection_address: Option<Addr>,
        index: Option<String>,
        sort_by_last_transaction_lt: bool,
        limit: usize,
        offset: usize,
        resp: oneshot::Sender<anyhow::Result<Vec<storage::NftItemMeta>>>,
    },
    SetAddressName {
        address: Addr,
        name: String,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    GetAddressName {
        address: Addr,
        resp: oneshot::Sender<anyhow::Result<Option<String>>>,
    },
    RegisterCompilerAbis {
        entries: Vec<(Hash256, Value)>,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    GetCompilerAbi {
        code_hash: Hash256,
        resp: oneshot::Sender<anyhow::Result<Option<Value>>>,
    },
    DumpState {
        path: String,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    LoadState {
        path: String,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
}

pub struct Localnet {
    tx: mpsc::Sender<Request>,
    started_at: SystemTime,
}

impl Default for Localnet {
    fn default() -> Self {
        Self::new(StateSource::Local, None)
    }
}

impl Localnet {
    #[must_use]
    pub fn new(state_source: StateSource, db_path: Option<String>) -> Self {
        Self::with_block_production(state_source, db_path, BlockProductionMode::Instant)
    }

    #[must_use]
    pub fn with_block_production(
        state_source: StateSource,
        db_path: Option<String>,
        block_production: BlockProductionMode,
    ) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let started_at = SystemTime::now();

        std::thread::spawn(move || {
            if let Err(e) = run_node_loop(rx, state_source, db_path, block_production) {
                tracing::error!("Node loop failed: {:?}", e);
            }
        });

        Self { tx, started_at }
    }

    #[must_use]
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at
            .elapsed()
            .map_or(0, |duration| duration.as_secs())
    }

    pub async fn send_boc(&self, boc_str: String) -> anyhow::Result<LocalnetBlockTransactions> {
        let boc = base64::engine::general_purpose::STANDARD
            .decode(&boc_str)
            .context("Invalid BOC base64")?
            .into();
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::SendBoc { boc, resp }).await?;
        rx.await?
    }

    pub async fn send_internal_boc(
        &self,
        boc_str: String,
    ) -> anyhow::Result<LocalnetBlockTransactions> {
        let boc = base64::engine::general_purpose::STANDARD
            .decode(&boc_str)
            .context("Invalid BOC base64")?
            .into();
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::SendInternalBoc { boc, resp }).await?;
        rx.await?
    }

    pub async fn get_address_information(
        &self,
        address_str: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<LocalnetAccountState> {
        let address = Self::parse_addr(&address_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressInformation {
                address,
                seqno,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_shard_account_cell(
        &self,
        address_str: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<BocBytes> {
        let address = Self::parse_addr(&address_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetShardAccountCell {
                address,
                seqno,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn set_shard_account(
        &self,
        address_str: String,
        shard_account: String,
    ) -> anyhow::Result<()> {
        let address = Self::parse_addr(&address_str)?;
        let shard_account =
            BocBytes::from_base64(&shard_account).context("Invalid shard_account base64")?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::SetShardAccount {
                address,
                shard_account,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_transactions(
        &self,
        address_str: String,
        limit: usize,
        lt: Option<u64>,
        hash_str: Option<String>,
        to_lt: Option<u64>,
    ) -> anyhow::Result<Vec<LocalnetTransaction>> {
        let address = Self::parse_addr(&address_str)?;
        let hash = if let Some(h) = hash_str {
            Some(Hash256::from_base64(&h)?)
        } else {
            None
        };

        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetTransactions {
                address,
                limit,
                lt,
                hash,
                to_lt,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_all_transactions(&self) -> anyhow::Result<Vec<LocalnetTransaction>> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetAllTransactions { resp }).await?;
        rx.await?
    }

    pub async fn get_pending_transactions(&self) -> anyhow::Result<Vec<LocalnetTransaction>> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetPendingTransactions { resp })
            .await?;
        rx.await?
    }

    pub async fn try_locate_tx(
        &self,
        source_str: String,
        destination_str: String,
        created_lt: u64,
    ) -> anyhow::Result<LocalnetTransaction> {
        let source = Self::parse_addr(&source_str)?;
        let destination = Self::parse_addr(&destination_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::TryLocateTx {
                source,
                destination,
                created_lt,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn try_locate_result_tx(
        &self,
        source_str: String,
        destination_str: String,
        created_lt: u64,
    ) -> anyhow::Result<LocalnetTransaction> {
        let source = Self::parse_addr(&source_str)?;
        let destination = Self::parse_addr(&destination_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::TryLocateResultTx {
                source,
                destination,
                created_lt,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn try_locate_source_tx(
        &self,
        source_str: String,
        destination_str: String,
        created_lt: u64,
    ) -> anyhow::Result<LocalnetTransaction> {
        let source = Self::parse_addr(&source_str)?;
        let destination = Self::parse_addr(&destination_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::TryLocateSourceTx {
                source,
                destination,
                created_lt,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn run_get_method(
        &self,
        address_str: String,
        method: String,
        stack_json: Vec<Value>,
        seqno: Option<u32>,
    ) -> anyhow::Result<LocalnetRunGetMethodResult> {
        let address = Self::parse_addr(&address_str)?;
        let method_id = if let Ok(id) = method.parse::<i32>() {
            id
        } else {
            let crc = CRC16.checksum(method.as_bytes());
            (i32::from(crc) & 0xffff) | 0x10000
        };

        let stack = Tuple(
            stack_json
                .into_iter()
                .map(json_to_legacy_item)
                .collect::<anyhow::Result<Vec<_>>>()?,
        );

        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::RunGetMethod {
                address,
                method_id,
                stack,
                seqno,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_address_balance(
        &self,
        address: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<u128> {
        let info = self.get_address_information(address, seqno).await?;
        Ok(info.balance)
    }

    pub async fn get_address_state(
        &self,
        address: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<AccountStatus> {
        let info = self.get_address_information(address, seqno).await?;
        Ok(info.state)
    }

    pub async fn get_block_header(&self, seqno: u32) -> anyhow::Result<LocalnetBlockHeader> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetBlockHeader { seqno, resp })
            .await?;
        rx.await?
    }

    pub async fn get_block_transactions(
        &self,
        seqno: u32,
    ) -> anyhow::Result<LocalnetBlockTransactions> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetBlockTransactions { seqno, resp })
            .await?;
        rx.await?
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<LocalnetMasterchainInfo> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetMasterchainInfo { resp }).await?;
        rx.await?
    }

    pub async fn get_consensus_block(&self) -> anyhow::Result<LocalnetConsensusBlock> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetConsensusBlock { resp }).await?;
        rx.await?
    }

    pub async fn get_libraries(
        &self,
        hashes: Vec<Hash256>,
    ) -> anyhow::Result<Vec<LocalnetLibrary>> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetLibraries { hashes, resp }).await?;
        rx.await?
    }

    pub async fn get_config_param(
        &self,
        param: u32,
        seqno: Option<u32>,
    ) -> anyhow::Result<BocBytes> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetConfigParam { param, seqno, resp })
            .await?;
        rx.await?
    }

    pub async fn get_config_all(&self, seqno: Option<u32>) -> anyhow::Result<BocBytes> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetConfigAll { seqno, resp }).await?;
        rx.await?
    }

    pub async fn get_shards(&self, seqno: u32) -> anyhow::Result<Vec<LocalnetBlockId>> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetShards { seqno, resp }).await?;
        rx.await?
    }

    pub async fn lookup_block(
        &self,
        workchain: i32,
        shard: String,
        seqno: Option<u32>,
        lt: Option<u64>,
        unixtime: Option<u32>,
    ) -> anyhow::Result<LocalnetBlockId> {
        let shard = shard.parse::<i64>().context("invalid shard number")?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::LookupBlock {
                workchain,
                shard,
                seqno,
                lt,
                unixtime,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn faucet(&self, address_str: String, amount: u128) -> anyhow::Result<Value> {
        let address = Self::parse_addr(&address_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::Faucet {
                address,
                amount,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_traces(&self, tx_hash: Hash256) -> anyhow::Result<storage::TraceNode> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetTraces { tx_hash, resp }).await?;
        rx.await?
    }

    pub async fn get_traces_by_message_hash(
        &self,
        msg_hash: Hash256,
    ) -> anyhow::Result<storage::TraceNode> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetTracesByMessageHash { msg_hash, resp })
            .await?;
        rx.await?
    }

    pub async fn emulate_trace(
        &self,
        boc_str: String,
        ignore_chksig: Option<bool>,
        mc_block_seqno: Option<u32>,
    ) -> anyhow::Result<storage::EmulateTraceResult> {
        let boc = base64::engine::general_purpose::STANDARD
            .decode(&boc_str)
            .context("Invalid BOC base64")?
            .into();
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::EmulateTrace {
                boc,
                ignore_chksig: ignore_chksig.unwrap_or(false),
                mc_block_seqno,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_jetton_masters(
        &self,
        address: Option<String>,
        admin_address: Option<String>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<storage::JettonMasterMeta>> {
        let address = address.map(|s| Self::parse_addr(&s)).transpose()?;
        let admin_address = admin_address.map(|s| Self::parse_addr(&s)).transpose()?;

        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetJettonMasters {
                address,
                admin_address,
                limit: limit.unwrap_or(10),
                offset: offset.unwrap_or(0),
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_jetton_wallets(
        &self,
        address: Option<String>,
        owner_address: Option<String>,
        jetton_address: Option<String>,
        exclude_zero_balance: Option<bool>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<storage::JettonWalletMeta>> {
        let address = address.map(|s| Self::parse_addr(&s)).transpose()?;
        let owner_address = owner_address.map(|s| Self::parse_addr(&s)).transpose()?;
        let jetton_address = jetton_address.map(|s| Self::parse_addr(&s)).transpose()?;

        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetJettonWallets {
                address,
                owner_address,
                jetton_address,
                exclude_zero_balance: exclude_zero_balance.unwrap_or(false),
                limit: limit.unwrap_or(10),
                offset: offset.unwrap_or(0),
                resp,
            })
            .await?;
        rx.await?
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn get_nft_items(
        &self,
        address: Option<String>,
        owner_address: Option<String>,
        collection_address: Option<String>,
        index: Option<String>,
        sort_by_last_transaction_lt: Option<bool>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<storage::NftItemMeta>> {
        let address = address.map(|s| Self::parse_addr(&s)).transpose()?;
        let owner_address = owner_address.map(|s| Self::parse_addr(&s)).transpose()?;
        let collection_address = collection_address
            .map(|s| Self::parse_addr(&s))
            .transpose()?;

        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetNftItems {
                address,
                owner_address,
                collection_address,
                index,
                sort_by_last_transaction_lt: sort_by_last_transaction_lt.unwrap_or(false),
                limit: limit.unwrap_or(10),
                offset: offset.unwrap_or(0),
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn set_address_name(&self, address_str: String, name: String) -> anyhow::Result<()> {
        let address = Self::parse_addr(&address_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::SetAddressName {
                address,
                name,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_address_name(&self, address_str: String) -> anyhow::Result<Option<String>> {
        let address = Self::parse_addr(&address_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressName { address, resp })
            .await?;
        rx.await?
    }

    pub async fn register_compiler_abis(
        &self,
        entries: Vec<(Hash256, Value)>,
    ) -> anyhow::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::RegisterCompilerAbis { entries, resp })
            .await?;
        rx.await?
    }

    pub async fn get_compiler_abi(&self, code_hash: Hash256) -> anyhow::Result<Option<Value>> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetCompilerAbi { code_hash, resp })
            .await?;
        rx.await?
    }

    pub async fn dump_state(&self, path: String) -> anyhow::Result<()> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::DumpState { path, resp }).await?;
        rx.await?
    }

    pub async fn load_state(&self, path: String) -> anyhow::Result<()> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::LoadState { path, resp }).await?;
        rx.await?
    }

    fn parse_addr(s: &str) -> anyhow::Result<Addr> {
        let (int_addr, _) = StdAddr::from_str_ext(s, StdAddrFormat::any()).map_err(|_| {
            anyhow::anyhow!("Invalid address, only standard internal address is allowed")
        })?;
        Ok(Addr {
            workchain: i32::from(int_addr.workchain),
            addr: int_addr.address.0,
        })
    }
}

fn run_node_loop(
    mut rx: mpsc::Receiver<Request>,
    state_source: StateSource,
    db_path: Option<String>,
    block_production: BlockProductionMode,
) -> anyhow::Result<()> {
    let executor = Box::new(TvmEmulatorAdapter::new()?);
    let config_bytes = base64::engine::general_purpose::STANDARD.decode(DEFAULT_CONFIG)?;
    let mut node = Node::with_db_path(executor, config_bytes.into(), state_source, db_path)?;

    tracing::info!(
        "TON localnet started: block_production={:?}",
        block_production
    );

    match block_production {
        BlockProductionMode::Instant => run_instant_node_loop(&mut node, &mut rx),
        BlockProductionMode::Interval { block_time } => {
            run_interval_node_loop(&mut node, &mut rx, block_time)
        }
    }
}

fn run_instant_node_loop(node: &mut Node, rx: &mut mpsc::Receiver<Request>) -> anyhow::Result<()> {
    loop {
        // 1. Process all currently pending requests
        if !drain_loop_requests(node, rx, BlockProductionMode::Instant) {
            break;
        }

        // 2. If there are pending messages in the pool, mine one
        if node.has_pending_messages() {
            tracing::info!("Auto-mining message from pool");
            if let Err(e) = node.mine_one() {
                tracing::error!("Auto-mining failed: {:?}", e);
            }
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }

        // 3. If pool is empty, block until next request
        if let Some(req) = rx.blocking_recv() {
            process_loop_request(node, req, BlockProductionMode::Instant);
        } else {
            break; // Channel closed
        }
    }
    Ok(())
}

fn run_interval_node_loop(
    node: &mut Node,
    rx: &mut mpsc::Receiver<Request>,
    block_time: Duration,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !block_time.is_zero(),
        "block production interval must be greater than zero"
    );
    let mut next_block_at = Instant::now() + block_time;
    loop {
        if !drain_loop_requests(node, rx, BlockProductionMode::Interval { block_time }) {
            break;
        }

        let now = Instant::now();
        if now >= next_block_at {
            tracing::info!("Producing timed localnet block");
            if let Err(e) = node.produce_block() {
                tracing::error!("Timed block production failed: {:?}", e);
            }
            next_block_at = Instant::now() + block_time;
        }

        let now = Instant::now();
        std::thread::sleep(
            next_block_at
                .saturating_duration_since(now)
                .min(Duration::from_millis(10)),
        );
    }
    Ok(())
}

fn drain_loop_requests(
    node: &mut Node,
    rx: &mut mpsc::Receiver<Request>,
    block_production: BlockProductionMode,
) -> bool {
    for _ in 0..MAX_LOOP_REQUESTS {
        match rx.try_recv() {
            Ok(req) => process_loop_request(node, req, block_production),
            Err(mpsc::error::TryRecvError::Empty) => return true,
            Err(mpsc::error::TryRecvError::Disconnected) => return false,
        }
    }
    true
}

fn process_loop_request(node: &mut Node, req: Request, block_production: BlockProductionMode) {
    tracing::debug!("Node loop processing request: {:?}", req);
    match req {
        Request::SendBoc { boc, resp } => {
            let res = match block_production {
                BlockProductionMode::Instant => handle_send_boc(node, boc),
                BlockProductionMode::Interval { .. } => handle_enqueue_boc(node, boc),
            };
            let _ = resp.send(res);
        }
        Request::SendInternalBoc { boc, resp } => {
            let res = match block_production {
                BlockProductionMode::Instant => handle_send_internal_boc(node, boc),
                BlockProductionMode::Interval { .. } => handle_enqueue_internal_boc(node, boc),
            };
            let _ = resp.send(res);
        }
        Request::GetAddressInformation {
            address,
            seqno,
            resp,
        } => {
            let res = handle_get_address_info(node, address, seqno);
            let _ = resp.send(res);
        }
        Request::GetShardAccountCell {
            address,
            seqno,
            resp,
        } => {
            let res = node.get_shard_account_at_block(&address, seqno);
            let _ = resp.send(res);
        }
        Request::SetShardAccount {
            address,
            shard_account,
            resp,
        } => {
            let res = node.set_shard_account(&address, shard_account);
            let _ = resp.send(res);
        }
        Request::GetTransactions {
            address,
            limit,
            lt,
            hash,
            to_lt,
            resp,
        } => {
            let res = handle_get_transactions(node, address, limit, lt, hash, to_lt);
            let _ = resp.send(res);
        }
        Request::GetAllTransactions { resp } => {
            let res = handle_get_all_transactions(node);
            let _ = resp.send(res);
        }
        Request::GetPendingTransactions { resp } => {
            let res = handle_get_pending_transactions(node);
            let _ = resp.send(res);
        }
        Request::TryLocateTx {
            source,
            destination,
            created_lt,
            resp,
        } => {
            let res = handle_try_locate_tx(node, source, destination, created_lt);
            let _ = resp.send(res);
        }
        Request::TryLocateResultTx {
            source,
            destination,
            created_lt,
            resp,
        } => {
            let res = handle_try_locate_result_tx(node, source, destination, created_lt);
            let _ = resp.send(res);
        }
        Request::TryLocateSourceTx {
            source,
            destination,
            created_lt,
            resp,
        } => {
            let res = handle_try_locate_source_tx(node, source, destination, created_lt);
            let _ = resp.send(res);
        }
        Request::RunGetMethod {
            address,
            method_id,
            stack,
            seqno,
            resp,
        } => {
            let res = handle_run_get_method(node, address, method_id, stack, seqno);
            let _ = resp.send(res);
        }
        Request::GetBlockHeader { seqno, resp } => {
            let res = handle_get_block_header(node, seqno);
            let _ = resp.send(res);
        }
        Request::GetBlockTransactions { seqno, resp } => {
            let res = handle_get_block_transactions(node, seqno);
            let _ = resp.send(res);
        }
        Request::GetMasterchainInfo { resp } => {
            let res = handle_get_masterchain_info(node);
            let _ = resp.send(res);
        }
        Request::GetConsensusBlock { resp } => {
            let res = handle_get_consensus_block(node);
            let _ = resp.send(res);
        }
        Request::GetLibraries { hashes, resp } => {
            let res = handle_get_libraries(node, &hashes);
            let _ = resp.send(res);
        }
        Request::GetConfigParam { param, seqno, resp } => {
            let res = handle_get_config_param(node, param, seqno);
            let _ = resp.send(res);
        }
        Request::GetConfigAll { seqno, resp } => {
            let res = handle_get_config_all(node, seqno);
            let _ = resp.send(res);
        }
        Request::GetShards { seqno, resp } => {
            let res = handle_get_shards(node, seqno);
            let _ = resp.send(res);
        }
        Request::LookupBlock {
            workchain: _, // unused since localnet have only one workchain
            shard: _,     // unused since localnet have only one shard
            seqno,
            lt,
            unixtime,
            resp,
        } => {
            let res = handle_lookup_block(node, seqno, lt, unixtime);
            let _ = resp.send(res);
        }
        Request::Faucet {
            address,
            amount,
            resp,
        } => {
            let res = match block_production {
                BlockProductionMode::Instant => node.faucet(&address, amount),
                BlockProductionMode::Interval { .. } => {
                    handle_enqueue_faucet(node, address, amount)
                }
            };
            let _ = resp.send(res);
        }
        Request::GetTraces { tx_hash, resp } => {
            let res = node.get_traces(&tx_hash);
            let _ = resp.send(res);
        }
        Request::GetTracesByMessageHash { msg_hash, resp } => {
            let res = node.get_traces_by_message_hash(&msg_hash);
            let _ = resp.send(res);
        }
        Request::EmulateTrace {
            boc,
            ignore_chksig,
            mc_block_seqno,
            resp,
        } => {
            let res = node.emulate_trace_by_external_message(boc, ignore_chksig, mc_block_seqno);
            let _ = resp.send(res);
        }
        Request::GetJettonMasters {
            address,
            admin_address,
            limit,
            offset,
            resp,
        } => {
            let res = node.get_jetton_masters(address, admin_address, limit, offset);
            let _ = resp.send(res);
        }
        Request::GetJettonWallets {
            address,
            owner_address,
            jetton_address,
            exclude_zero_balance,
            limit,
            offset,
            resp,
        } => {
            let res = node.get_jetton_wallets(
                address,
                owner_address,
                jetton_address,
                exclude_zero_balance,
                limit,
                offset,
            );
            let _ = resp.send(res);
        }
        Request::GetNftItems {
            address,
            owner_address,
            collection_address,
            index,
            sort_by_last_transaction_lt,
            limit,
            offset,
            resp,
        } => {
            let res = node.get_nft_items(
                address,
                owner_address,
                collection_address,
                index,
                sort_by_last_transaction_lt,
                limit,
                offset,
            );
            let _ = resp.send(res);
        }
        Request::SetAddressName {
            address,
            name,
            resp,
        } => {
            node.history.address_names.insert(address, name);
            let _ = resp.send(Ok(()));
        }
        Request::GetAddressName { address, resp } => {
            let res = node.history.address_names.get(&address).cloned();
            let _ = resp.send(Ok(res));
        }
        Request::RegisterCompilerAbis { entries, resp } => {
            let res = entries
                .into_iter()
                .try_for_each(|(code_hash, compiler_abi)| {
                    node.history.set_compiler_abi(code_hash, compiler_abi)
                });
            let _ = resp.send(res);
        }
        Request::GetCompilerAbi { code_hash, resp } => {
            let _ = resp.send(Ok(node.history.get_compiler_abi(&code_hash)));
        }
        Request::DumpState { path, resp } => {
            let res = node.dump_state_to_path(path);
            let _ = resp.send(res);
        }
        Request::LoadState { path, resp } => {
            let res = node.load_state_from_path(path);
            let _ = resp.send(res);
        }
    }
}

fn handle_send_boc(node: &mut Node, boc: BocBytes) -> anyhow::Result<LocalnetBlockTransactions> {
    let msg_hash_norm = normalized_ext_in_hash_from_boc(&boc)?;
    let (msg_hash, tx_hash, seqno, _) = node.send_boc(boc)?;
    build_send_boc_response(node, msg_hash, msg_hash_norm, tx_hash, seqno)
}

fn handle_enqueue_boc(node: &mut Node, boc: BocBytes) -> anyhow::Result<LocalnetBlockTransactions> {
    let msg_hash_norm = normalized_ext_in_hash_from_boc(&boc)?;
    let msg_hash = node.enqueue_boc(boc)?;
    Ok(build_accepted_message_response(
        node,
        msg_hash,
        msg_hash_norm,
    ))
}

fn handle_send_internal_boc(
    node: &mut Node,
    boc: BocBytes,
) -> anyhow::Result<LocalnetBlockTransactions> {
    let (msg_hash, tx_hash, seqno, _) = node.send_internal_boc(boc)?;
    build_send_boc_response(node, msg_hash, None, tx_hash, seqno)
}

fn handle_enqueue_internal_boc(
    node: &mut Node,
    boc: BocBytes,
) -> anyhow::Result<LocalnetBlockTransactions> {
    let msg_hash = node.enqueue_internal_boc(boc)?;
    Ok(build_accepted_message_response(node, msg_hash, None))
}

fn handle_enqueue_faucet(node: &mut Node, address: Addr, amount: u128) -> anyhow::Result<Value> {
    let msg_hash = node.enqueue_faucet(&address, amount)?;
    Ok(serde_json::json!({
        "ok": true,
        "result": {
            "msg_hash": msg_hash.to_hex()
        }
    }))
}

fn build_accepted_message_response(
    node: &Node,
    msg_hash: Hash256,
    msg_hash_norm: Option<Hash256>,
) -> LocalnetBlockTransactions {
    let id = node
        .get_block_header(node.globals.head_seqno)
        .map_or_else(LocalnetBlockId::first, |block| block.block_id());

    LocalnetBlockTransactions {
        id,
        transactions: Vec::new(),
        msg_hash: Some(msg_hash),
        msg_hash_norm,
    }
}

fn build_send_boc_response(
    node: &Node,
    msg_hash: Hash256,
    msg_hash_norm: Option<Hash256>,
    tx_hash: Hash256,
    seqno: Seqno,
) -> anyhow::Result<LocalnetBlockTransactions> {
    let Some(ext_tx) = node.get_transaction_by_hash(&tx_hash) else {
        anyhow::bail!("Transaction not found after mining")
    };

    let tx_boc = node.get_cell(&tx_hash).unwrap_or_default();
    let tx_struct = convert_to_tx_struct(&ext_tx, tx_boc)?;

    let Some(block_header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block {seqno} with transaction not found after mining")
    };

    Ok(LocalnetBlockTransactions {
        id: block_header.block_id(),
        transactions: vec![tx_struct],
        msg_hash: Some(msg_hash),
        msg_hash_norm,
    })
}

fn handle_get_address_info(
    node: &mut Node,
    address: Addr,
    seqno: Option<u32>,
) -> anyhow::Result<LocalnetAccountState> {
    let seqno = seqno.unwrap_or(node.globals.head_seqno);
    let meta = node.get_address_information_at_block(&address, seqno);

    let block_id = if let Some(block_header) = node.get_block_header(seqno) {
        block_header.block_id()
    } else if seqno == 0 {
        LocalnetBlockId::first()
    } else {
        anyhow::bail!("Block {seqno} not found")
    };
    let sync_utime = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let Some(meta) = meta else {
        return Ok(LocalnetAccountState::empty(address, block_id, sync_utime));
    };

    let code = meta.code_hash.and_then(|h| node.get_cell(&h));
    let data = meta.data_hash.and_then(|h| node.get_cell(&h));
    let last_transaction_id = meta.last_tx_id();

    Ok(LocalnetAccountState {
        address,
        account_state_hash: meta.account_hash,
        balance: meta.cached_balance.unwrap_or(0),
        code,
        code_hash: meta.code_hash,
        data,
        data_hash: meta.data_hash,
        last_transaction_id,
        block_id,
        state: meta.status,
        sync_utime,
        frozen_hash: meta.frozen_hash,
    })
}

fn handle_get_transactions(
    node: &Node,
    address: Addr,
    limit: usize,
    lt: Option<u64>,
    hash: Option<Hash256>,
    to_lt: Option<u64>,
) -> anyhow::Result<Vec<LocalnetTransaction>> {
    let mut raw_txs = node.get_transactions(&address, limit, lt, hash);

    if let Some(min_lt) = to_lt {
        raw_txs.retain(|tx| tx.meta.lt >= min_lt);
    }

    let full_txs = raw_txs
        .iter()
        .flat_map(|tx| {
            let tx_boc = node.get_cell(&tx.meta.tx_hash).unwrap_or_default();
            convert_to_tx_struct(tx, tx_boc)
        })
        .collect();
    Ok(full_txs)
}

fn handle_get_all_transactions(node: &Node) -> anyhow::Result<Vec<LocalnetTransaction>> {
    let mut metas = node
        .history
        .tx_by_hash
        .values()
        .cloned()
        .collect::<Vec<_>>();
    metas.sort_by(|a, b| b.lt.cmp(&a.lt).then_with(|| b.tx_hash.cmp(&a.tx_hash)));

    let mut result = Vec::with_capacity(metas.len());
    for meta in metas {
        if let Some(tx) = node.get_transaction_by_hash(&meta.tx_hash) {
            result.push(convert_to_tx_struct(&tx, tx.tx_boc.clone())?);
        }
    }
    Ok(result)
}

fn handle_get_pending_transactions(node: &Node) -> anyhow::Result<Vec<LocalnetTransaction>> {
    let mut pending_tx_hashes = Vec::new();
    let mut seen = HashSet::new();
    for msg_hash in node.pool.external.iter().chain(node.pool.internal.iter()) {
        if let Some(tx_hash) = node.history.msg_to_tx.get(msg_hash)
            && seen.insert(*tx_hash)
        {
            pending_tx_hashes.push(*tx_hash);
        }
    }

    let mut result = Vec::with_capacity(pending_tx_hashes.len());
    for tx_hash in pending_tx_hashes {
        if let Some(tx) = node.get_transaction_by_hash(&tx_hash) {
            result.push(convert_to_tx_struct(&tx, tx.tx_boc.clone())?);
        }
    }
    result.sort_by(|a, b| {
        b.transaction_id
            .lt
            .cmp(&a.transaction_id.lt)
            .then_with(|| b.hash.cmp(&a.hash))
    });
    Ok(result)
}

fn handle_try_locate_tx(
    node: &Node,
    source: Addr,
    destination: Addr,
    created_lt: u64,
) -> anyhow::Result<LocalnetTransaction> {
    let msg_hash = find_message_hash(node, source, destination, created_lt)?;
    let tx_hash = node
        .history
        .msg_to_tx
        .get(&msg_hash)
        .copied()
        .context("Destination transaction not found for message")?;
    let tx = node
        .get_transaction_by_hash(&tx_hash)
        .context("Located destination transaction is missing")?;

    if tx.meta.account != destination {
        anyhow::bail!("Located transaction does not belong to destination account")
    }

    convert_to_tx_struct(&tx, tx.tx_boc.clone())
}

fn handle_try_locate_result_tx(
    node: &Node,
    source: Addr,
    destination: Addr,
    created_lt: u64,
) -> anyhow::Result<LocalnetTransaction> {
    handle_try_locate_tx(node, source, destination, created_lt)
}

fn handle_try_locate_source_tx(
    node: &Node,
    source: Addr,
    destination: Addr,
    created_lt: u64,
) -> anyhow::Result<LocalnetTransaction> {
    let msg_hash = find_message_hash(node, source, destination, created_lt)?;
    let tx_hash = node
        .history
        .tx_by_hash
        .iter()
        .find_map(|(hash, tx)| {
            (tx.account == source && tx.out_msg_hashes.contains(&msg_hash)).then_some(*hash)
        })
        .context("Source transaction not found for message")?;
    let tx = node
        .get_transaction_by_hash(&tx_hash)
        .context("Located source transaction is missing")?;
    convert_to_tx_struct(&tx, tx.tx_boc.clone())
}

fn find_message_hash(
    node: &Node,
    source: Addr,
    destination: Addr,
    created_lt: u64,
) -> anyhow::Result<Hash256> {
    node.history
        .msg_by_hash
        .iter()
        .find_map(|(hash, msg)| {
            (msg.src == Some(source)
                && msg.dst == Some(destination)
                && msg.created_lt == Some(created_lt))
            .then_some(*hash)
        })
        .context("Message not found by source, destination and created_lt")
}

fn handle_run_get_method(
    node: &mut Node,
    address: Addr,
    method_id: i32,
    stack: Tuple,
    seqno: Option<u32>,
) -> anyhow::Result<LocalnetRunGetMethodResult> {
    let seqno = seqno.unwrap_or(node.globals.head_seqno);
    let meta = node.get_address_information_at_block(&address, seqno);
    let meta = meta.ok_or_else(|| anyhow::anyhow!("Account {address} not found"))?;

    let Some(block_header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block {seqno} not found")
    };

    let block_id = block_header.block_id();
    let last_transaction_id = meta.last_tx_id();

    let code_boc = meta.code_hash.and_then(|h| node.get_cell(&h)).map_or_else(
        || EMPTY_CELL_BASE64.to_owned(),
        |b| base64::engine::general_purpose::STANDARD.encode(b),
    );

    let data_boc = meta.data_hash.and_then(|h| node.get_cell(&h)).map_or_else(
        || EMPTY_CELL_BASE64.to_owned(),
        |b| base64::engine::general_purpose::STANDARD.encode(b),
    );
    let libs = node
        .build_vm_global_libs_boc()?
        .map_or_else(String::new, |boc| {
            base64::engine::general_purpose::STANDARD.encode(boc)
        });

    let balance_tokens = meta.cached_balance.unwrap_or(0);

    let args = RunGetMethodArgs {
        code: code_boc,
        data: data_boc,
        method_id,
        address: address.to_string(),
        unixtime: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64,
        balance: balance_tokens.to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        gas_limit: "10000000".to_owned(),
        debug_enabled: false,
        verbosity: ExecutorVerbosity::Short,
        libs,
        extra_currencies: Default::default(),
        prev_blocks_info: None,
    };

    let stack_cell = stack
        .serialize()
        .context("Failed to serialize stack to BoC")?;
    let stack_b64 = Boc::encode_base64(&stack_cell);

    let exec = GetExecutor::new(&args).context("Failed to create GetExecutor")?;

    let res = exec
        .run_get_method(&stack_b64, &args, None)
        .context("Execution failed")?;

    match res {
        GetMethodResult::Success(s) => {
            let stack_bytes = base64::engine::general_purpose::STANDARD
                .decode(s.stack.as_ref())
                .unwrap_or_default();
            Ok(LocalnetRunGetMethodResult {
                gas_used: s.gas_used.parse().unwrap_or(0),
                stack: stack_bytes.into(),
                exit_code: s.vm_exit_code,
                vm_log: s.vm_log,
                block_id,
                last_transaction_id,
            })
        }
        GetMethodResult::Error(e) => anyhow::bail!("Get method error: {e:?}"),
    }
}

pub(crate) fn convert_to_tx_struct(
    tx: &TransactionInfo,
    tx_boc: BocBytes,
) -> anyhow::Result<LocalnetTransaction> {
    let in_msg_struct = if let Some(in_msg) = &tx.in_msg {
        convert_to_message_struct(&in_msg.meta, &in_msg.boc)?
    } else {
        LocalnetMessage {
            hash: Hash256([0; 32]),
            hash_norm: None,
            source: None,
            destination: None,
            value: 0,
            body_hash: Hash256([0; 32]),
            body: Vec::new().into(),
            init_state: Vec::new().into(),
            opcode: None,
            fwd_fee: 0,
            ihr_fee: 0,
            created_lt: 0,
        }
    };

    let mut out_msgs_struct = Vec::new();
    for out_msg in &tx.out_msgs {
        out_msgs_struct.push(convert_to_message_struct(&out_msg.meta, &out_msg.boc)?);
    }

    Ok(LocalnetTransaction {
        hash: tx.meta.tx_hash,
        address: tx.meta.account,
        mc_block_seqno: tx.meta.block_seqno,
        utime: tx.meta.now,
        data: tx_boc,
        success: tx.meta.success,
        exit_code: tx.meta.compute_exit_code.unwrap_or(0),
        transaction_id: LocalnetTransactionId {
            lt: tx.meta.lt,
            hash: tx.meta.tx_hash,
        },
        in_msg: in_msg_struct,
        out_msgs: out_msgs_struct,
        total_fees: tx.meta.total_fees.unwrap_or(0),
        storage_fees: tx.meta.storage_fees.unwrap_or(0),
        other_fees: tx.meta.other_fees.unwrap_or(0),
    })
}

pub(crate) fn compute_normalized_ext_in_hash(msg: &Message<'_>) -> anyhow::Result<Hash256> {
    let MsgInfo::ExtIn(info) = &msg.info else {
        anyhow::bail!("TEP-467 normalization only applies to external-in messages");
    };

    let mut body_builder = CellBuilder::new();
    body_builder.store_slice(msg.body)?;
    let body_cell = body_builder.build()?;

    let normalized_info = ExtInMsgInfo {
        src: None,
        dst: info.dst.clone(),
        import_fee: Tokens::ZERO,
    };

    let ctx = Cell::empty_context();
    let mut builder = CellBuilder::new();
    builder.store_small_uint(0b10, 2)?;
    normalized_info.store_into(&mut builder, ctx)?;
    builder.store_bit_zero()?;
    builder.store_bit_one()?;
    builder.store_reference(body_cell)?;
    Ok(Hash256(*builder.build()?.repr_hash().as_array()))
}

fn normalized_ext_in_hash_from_boc(boc: &[u8]) -> anyhow::Result<Option<Hash256>> {
    let cell = Boc::decode(boc)?;
    let msg = cell.parse::<Message<'_>>()?;
    if !matches!(&msg.info, MsgInfo::ExtIn(_)) {
        return Ok(None);
    }
    Ok(Some(compute_normalized_ext_in_hash(&msg)?))
}

pub(crate) fn convert_to_message_struct(
    meta: &MsgMeta,
    boc: &[u8],
) -> anyhow::Result<LocalnetMessage> {
    let cell = Boc::decode(boc)?;
    let msg = cell.parse::<Message<'_>>()?;
    let hash_norm = match &msg.info {
        MsgInfo::ExtIn(_) => Some(compute_normalized_ext_in_hash(&msg)?),
        _ => None,
    };

    // Extract body
    let mut builder = CellBuilder::new();
    builder.store_slice(msg.body)?;
    let body_cell = builder.build()?;
    let body_hash = Hash256(*body_cell.repr_hash().as_array());
    let body_bytes = Boc::encode(body_cell);

    let (fwd_fee, ihr_fee) = match &msg.info {
        MsgInfo::Int(info) => (info.fwd_fee.into(), info.ihr_fee.into()),
        _ => (0, 0),
    };

    // Extract opcode (first 32 bits)
    let mut opcode = None;
    let mut body_slice = msg.body;
    if body_slice.size_bits() >= 32
        && let Ok(op) = body_slice.load_uint(32)
    {
        opcode = Some(op as u32);
    }

    let mut init_state_bytes = Vec::new();
    if let Some(init) = msg.init {
        let mut builder = CellBuilder::new();
        let _ = init.store_into(&mut builder, Cell::empty_context());
        if let Ok(cell) = builder.build() {
            init_state_bytes = Boc::encode(cell);
        }
    }

    Ok(LocalnetMessage {
        hash: meta.msg_hash,
        hash_norm,
        source: meta.src,
        destination: meta.dst,
        value: meta.value.unwrap_or(0),
        body_hash,
        body: body_bytes.into(),
        init_state: init_state_bytes.into(),
        opcode,
        fwd_fee,
        ihr_fee,
        created_lt: meta.created_lt.unwrap_or(0),
    })
}

fn handle_get_block_header(node: &Node, seqno: u32) -> anyhow::Result<LocalnetBlockHeader> {
    let Some(header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block {seqno} not found")
    };

    Ok(LocalnetBlockHeader {
        id: header.block_id(),
        gen_utime: header.gen_utime,
        start_lt: header.start_lt,
        end_lt: header.end_lt,
        prev_seqno: header.prev_seqno,
    })
}

fn handle_get_block_transactions(
    node: &Node,
    seqno: u32,
) -> anyhow::Result<LocalnetBlockTransactions> {
    let Some(block_header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block {seqno} not found")
    };
    let Some(txs) = node.get_block_transactions(&block_header) else {
        anyhow::bail!("Transaction in block {seqno} not found")
    };

    let mut result = Vec::new();
    for tx in txs {
        let Some(ext_tx) = node.get_transaction_by_hash(&tx.tx_hash) else {
            continue;
        };

        let tx_boc = node.get_cell(&ext_tx.meta.tx_hash).unwrap_or_default();
        result.push(convert_to_tx_struct(&ext_tx, tx_boc)?);
    }

    let block_id = block_header.block_id();

    Ok(LocalnetBlockTransactions {
        id: block_id,
        transactions: result,
        msg_hash: None,
        msg_hash_norm: None,
    })
}

fn handle_get_masterchain_info(node: &Node) -> anyhow::Result<LocalnetMasterchainInfo> {
    let head_block = node.get_block_header(node.globals.head_seqno);
    let block_id = head_block
        .as_ref()
        .map_or_else(LocalnetBlockId::first, BlockMeta::block_id);

    Ok(LocalnetMasterchainInfo {
        state_root_hash: block_id.root_hash,
        last: block_id,
        init: LocalnetBlockId::first(),
    })
}

fn handle_get_consensus_block(node: &Node) -> anyhow::Result<LocalnetConsensusBlock> {
    let consensus_block = node.globals.head_seqno;
    let timestamp = node
        .get_block_header(consensus_block)
        .map(|block| block.gen_utime)
        .unwrap_or_default();

    Ok(LocalnetConsensusBlock {
        consensus_block,
        timestamp,
    })
}

fn handle_get_libraries(node: &Node, hashes: &[Hash256]) -> anyhow::Result<Vec<LocalnetLibrary>> {
    let entries = node.get_libraries(hashes);
    let mut result = Vec::with_capacity(entries.len());
    for lookup in entries {
        if let Some(entry) = lookup.entry {
            result.push(LocalnetLibrary {
                hash: lookup.hash,
                found: true,
                data: Some(entry.lib_boc),
                publishers_count: entry.publishers.len(),
                publishers: entry.publishers.into_iter().collect(),
            });
        } else {
            result.push(LocalnetLibrary {
                hash: lookup.hash,
                found: false,
                data: None,
                publishers_count: 0,
                publishers: Vec::new(),
            });
        }
    }
    Ok(result)
}

fn handle_get_config_param(
    node: &Node,
    param: u32,
    seqno: Option<u32>,
) -> anyhow::Result<BocBytes> {
    ensure_seqno_exists(node, seqno)?;

    let config_boc = handle_get_config_all(node, seqno)?;
    let config_cell = Boc::decode(&config_boc).context("Failed to decode blockchain config BOC")?;
    let mut slice = config_cell.as_slice_allow_exotic();
    let config_dict = Dict::<u32, Cell>::load_from_root_ext(&mut slice, Cell::empty_context())
        .context("Failed to parse blockchain config dictionary")?;
    let param_cell = config_dict
        .get(param)
        .context("Failed to read config parameter")?
        .with_context(|| format!("Config parameter {param} not found"))?;

    Ok(Boc::encode(param_cell).into())
}

fn handle_get_config_all(node: &Node, seqno: Option<u32>) -> anyhow::Result<BocBytes> {
    ensure_seqno_exists(node, seqno)?;

    node.get_cell(&node.globals.config_boc_hash)
        .context("Blockchain config cell not found")
}

fn handle_get_shards(node: &Node, seqno: u32) -> anyhow::Result<Vec<LocalnetBlockId>> {
    let Some(block_header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block not found for seqno={seqno}")
    };
    Ok(vec![block_header.block_id()])
}

fn ensure_seqno_exists(node: &Node, seqno: Option<u32>) -> anyhow::Result<()> {
    if let Some(seqno) = seqno
        && seqno > 0
        && node.get_block_header(seqno).is_none()
    {
        anyhow::bail!("Block {seqno} not found");
    }
    Ok(())
}

fn handle_lookup_block(
    node: &Node,
    seqno: Option<u32>,
    lt: Option<u64>,
    unixtime: Option<u32>,
) -> anyhow::Result<LocalnetBlockId> {
    let found_block = if let Some(s) = seqno {
        node.get_block_header(s)
    } else if let Some(l) = lt {
        node.find_block_by_lt(l)
    } else if let Some(u) = unixtime {
        node.find_block_by_unixtime(u)
    } else {
        None
    };

    let Some(block) = found_block else {
        anyhow::bail!("Block not found for seqno={seqno:?}, lt={lt:?}, unixtime={unixtime:?}")
    };

    Ok(block.block_id())
}
