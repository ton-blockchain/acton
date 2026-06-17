use crate::executor::TvmEmulatorAdapter;
use crate::node::{Node, NodeClockInfo, StateSource};
use crate::node_snapshot::NodeStateSnapshot;
use crate::storage;
use crate::storage::{AccountStatus, BlockMeta, MsgMeta, TransactionInfo};
use crate::streaming::StreamingCommitEvent;
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use anyhow::Context;
use crc::{CRC_16_XMODEM, Crc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::Instant;
use ton_executor::DEFAULT_CONFIG;
use ton_executor::ExecutorVerbosity;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::message::PrevBlockId;
use tvm_ffi::json_stack::json_to_legacy_item;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{ExtInMsgInfo, Message, MsgInfo, StdAddr, StdAddrFormat};
use tycho_types::num::Tokens;

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);

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

impl From<LocalnetBlockId> for PrevBlockId {
    fn from(block_id: LocalnetBlockId) -> Self {
        Self {
            workchain: block_id.workchain,
            shard: block_id.shard,
            seqno: block_id.seqno,
            root_hash: block_id.root_hash.0,
            file_hash: block_id.file_hash.0,
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

#[derive(Debug, Clone)]
pub struct LocalnetAddressInfo {
    pub address: Addr,
    pub code_hash: Option<Hash256>,
    pub jetton_wallet: Option<storage::JettonWalletMeta>,
    pub jetton_master: Option<storage::JettonMasterMeta>,
    pub nft_item: Option<storage::NftItemMeta>,
    pub nft_collection_item: Option<storage::NftItemMeta>,
}

#[derive(Debug, Clone)]
pub struct LocalnetAccountStateWithInfo {
    pub state: LocalnetAccountState,
    pub info: LocalnetAddressInfo,
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
    pub bounce: bool,
    pub bounced: bool,
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
pub struct LocalnetMineResult {
    pub blocks_mined: u32,
    pub last_block_seqno: Seqno,
    pub blocks: Vec<LocalnetBlockId>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetRecoveryPointResult {
    pub id: u64,
    pub block_seqno: Seqno,
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
pub struct LocalnetAcceptedExternalMessage {
    /// Hash of the exact external-in message BOC accepted into the localnet queue.
    pub msg_hash: Hash256,
    /// TEP-467 normalized hash used by TonCenter-compatible lookups for external-in messages.
    pub msg_hash_norm: Hash256,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalnetAcceptedInternalMessage {
    /// Hash of the exact internal message BOC accepted into the localnet queue.
    pub msg_hash: Hash256,
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
        resp: oneshot::Sender<anyhow::Result<LocalnetAcceptedExternalMessage>>,
    },
    SendInternalBoc {
        boc: BocBytes,
        resp: oneshot::Sender<anyhow::Result<LocalnetAcceptedInternalMessage>>,
    },
    GetAddressInformation {
        address: Addr,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LocalnetAccountState>>,
    },
    GetAccountStates {
        addresses: Vec<Addr>,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<Vec<LocalnetAccountStateWithInfo>>>,
    },
    GetAddressInfos {
        addresses: Vec<Addr>,
        resp: oneshot::Sender<anyhow::Result<Vec<LocalnetAddressInfo>>>,
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
        resp: oneshot::Sender<anyhow::Result<LocalnetAcceptedInternalMessage>>,
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
    GetAddressNames {
        addresses: Vec<Addr>,
        resp: oneshot::Sender<anyhow::Result<Vec<Option<String>>>>,
    },
    RegisterCompilerAbis {
        entries: Vec<(Hash256, Value)>,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    GetCompilerAbis {
        code_hashes: Vec<Hash256>,
        resp: oneshot::Sender<
            anyhow::Result<Vec<Option<acton_abi_catalog::ExtendedContractAbi<Value>>>>,
        >,
    },
    DumpState {
        path: String,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    LoadState {
        path: String,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    CreateRecoveryPoint {
        resp: oneshot::Sender<anyhow::Result<LocalnetRecoveryPointResult>>,
    },
    RevertRecoveryPoint {
        id: u64,
        resp: oneshot::Sender<anyhow::Result<LocalnetRecoveryPointResult>>,
    },
    MineBlocks {
        count: u32,
        resp: oneshot::Sender<anyhow::Result<LocalnetMineResult>>,
    },
    GetClockInfo {
        resp: oneshot::Sender<anyhow::Result<NodeClockInfo>>,
    },
    IncreaseTime {
        seconds: u64,
        resp: oneshot::Sender<anyhow::Result<NodeClockInfo>>,
    },
    SetTime {
        timestamp: u32,
        resp: oneshot::Sender<anyhow::Result<NodeClockInfo>>,
    },
    SetNextBlockTimestamp {
        timestamp: u32,
        resp: oneshot::Sender<anyhow::Result<NodeClockInfo>>,
    },
}

pub struct Localnet {
    tx: mpsc::Sender<Request>,
    events_tx: broadcast::Sender<StreamingCommitEvent>,
    started_at: SystemTime,
}

#[derive(Default)]
struct RecoveryPoints {
    next_id: u64,
    points: Vec<RecoveryPoint>,
}

struct RecoveryPoint {
    id: u64,
    snapshot: NodeStateSnapshot,
}

impl RecoveryPoints {
    fn create(&mut self, node: &Node) -> anyhow::Result<LocalnetRecoveryPointResult> {
        let snapshot = node.build_snapshot()?;
        self.next_id = self
            .next_id
            .checked_add(1)
            .context("Recovery point id overflow")?;
        let id = self.next_id;
        let block_seqno = snapshot.globals.head_seqno;
        self.points.push(RecoveryPoint { id, snapshot });
        Ok(LocalnetRecoveryPointResult { id, block_seqno })
    }

    fn revert(&mut self, node: &mut Node, id: u64) -> anyhow::Result<LocalnetRecoveryPointResult> {
        let index = self
            .points
            .iter()
            .position(|point| point.id == id)
            .with_context(|| format!("Recovery point {id} not found"))?;
        let snapshot = self.points[index].snapshot.clone();
        let block_seqno = snapshot.globals.head_seqno;
        node.apply_snapshot(snapshot)?;
        self.points.truncate(index);
        Ok(LocalnetRecoveryPointResult { id, block_seqno })
    }

    fn clear(&mut self) {
        self.points.clear();
    }
}

pub const DEFAULT_BLOCK_INTERVAL_MS: u64 = 500;

impl Localnet {
    #[must_use]
    pub fn new(
        state_source: StateSource,
        db_path: Option<String>,
        block_interval: Duration,
        auto_mining: bool,
    ) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let (events_tx, _) = broadcast::channel(1024);
        let started_at = SystemTime::now();
        let node_events_tx = events_tx.clone();

        std::thread::spawn(move || {
            if let Err(e) = run_node_loop(
                rx,
                node_events_tx,
                state_source,
                db_path,
                block_interval,
                auto_mining,
            ) {
                tracing::error!("Node loop failed: {:?}", e);
            }
        });

        Self {
            tx,
            events_tx,
            started_at,
        }
    }

    #[must_use]
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at
            .elapsed()
            .map_or(0, |duration| duration.as_secs())
    }

    #[must_use]
    pub fn subscribe_streaming_events(&self) -> broadcast::Receiver<StreamingCommitEvent> {
        self.events_tx.subscribe()
    }

    pub async fn send_boc(
        &self,
        boc_str: String,
    ) -> anyhow::Result<LocalnetAcceptedExternalMessage> {
        let boc = BocBytes::from_base64(&boc_str).context("Invalid BOC base64")?;
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::SendBoc { boc, resp }).await?;
        rx.await?
    }

    pub async fn send_internal_boc(
        &self,
        boc_str: String,
    ) -> anyhow::Result<LocalnetAcceptedInternalMessage> {
        let boc = BocBytes::from_base64(&boc_str).context("Invalid BOC base64")?;
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

    pub async fn get_account_states(
        &self,
        addresses: Vec<Addr>,
        seqno: Option<u32>,
    ) -> anyhow::Result<Vec<LocalnetAccountStateWithInfo>> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAccountStates {
                addresses,
                seqno,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_address_infos(
        &self,
        addresses: Vec<Addr>,
    ) -> anyhow::Result<Vec<LocalnetAddressInfo>> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressInfos { addresses, resp })
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

    pub async fn faucet(
        &self,
        address_str: String,
        amount: u128,
    ) -> anyhow::Result<LocalnetAcceptedInternalMessage> {
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
        let boc = BocBytes::from_base64(&boc_str).context("Invalid BOC base64")?;
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

    pub async fn get_address_names(
        &self,
        address_strs: Vec<String>,
    ) -> anyhow::Result<Vec<(String, Option<String>)>> {
        let addresses = address_strs
            .iter()
            .map(|address| Self::parse_addr(address))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressNames { addresses, resp })
            .await?;
        let names = rx.await??;

        Ok(address_strs.into_iter().zip(names).collect())
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

    pub async fn get_compiler_abis(
        &self,
        code_hash_strs: Vec<String>,
    ) -> anyhow::Result<
        Vec<(
            String,
            Option<acton_abi_catalog::ExtendedContractAbi<Value>>,
        )>,
    > {
        let code_hashes = code_hash_strs
            .iter()
            .map(|code_hash| {
                Hash256::from_hex(code_hash).or_else(|_| Hash256::from_base64(code_hash))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetCompilerAbis { code_hashes, resp })
            .await?;
        let abis = rx.await??;

        Ok(code_hash_strs.into_iter().zip(abis).collect())
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

    pub async fn create_recovery_point(&self) -> anyhow::Result<LocalnetRecoveryPointResult> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::CreateRecoveryPoint { resp }).await?;
        rx.await?
    }

    pub async fn revert_recovery_point(
        &self,
        id: u64,
    ) -> anyhow::Result<LocalnetRecoveryPointResult> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::RevertRecoveryPoint { id, resp })
            .await?;
        rx.await?
    }

    pub async fn mine_blocks(&self, count: u32) -> anyhow::Result<LocalnetMineResult> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::MineBlocks { count, resp }).await?;
        rx.await?
    }

    pub async fn clock_info(&self) -> anyhow::Result<NodeClockInfo> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetClockInfo { resp }).await?;
        rx.await?
    }

    pub async fn increase_time(&self, seconds: u64) -> anyhow::Result<NodeClockInfo> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::IncreaseTime { seconds, resp })
            .await?;
        rx.await?
    }

    pub async fn set_time(&self, timestamp: u32) -> anyhow::Result<NodeClockInfo> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::SetTime { timestamp, resp }).await?;
        rx.await?
    }

    pub async fn set_next_block_timestamp(&self, timestamp: u32) -> anyhow::Result<NodeClockInfo> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::SetNextBlockTimestamp { timestamp, resp })
            .await?;
        rx.await?
    }

    pub(crate) fn parse_addr(s: &str) -> anyhow::Result<Addr> {
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
    events_tx: broadcast::Sender<StreamingCommitEvent>,
    state_source: StateSource,
    db_path: Option<String>,
    block_interval: Duration,
    auto_mining: bool,
) -> anyhow::Result<()> {
    let mut node = create_node(events_tx, state_source, db_path)?;
    let mut recovery_points = RecoveryPoints::default();
    tracing::info!(
        "TON localnet started, block interval: {}ms, auto mining: {}",
        block_interval.as_millis(),
        auto_mining
    );

    if !auto_mining {
        while let Some(req) = rx.blocking_recv() {
            process_loop_request(&mut node, &mut recovery_points, req);
        }
        return Ok(());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .context("Failed to create localnet node runtime")?;
    runtime.block_on(run_node_loop_async(rx, node, block_interval))
}

fn create_node(
    events_tx: broadcast::Sender<StreamingCommitEvent>,
    state_source: StateSource,
    db_path: Option<String>,
) -> anyhow::Result<Node> {
    let executor = Box::new(TvmEmulatorAdapter::new()?);
    let config_boc = BocBytes::from_base64(DEFAULT_CONFIG)?;
    let mut node = Node::with_db_path(executor, config_boc, state_source, db_path)?;
    node.streaming_events = Some(events_tx);
    Ok(node)
}

// The node loop runs on a dedicated current-thread Tokio runtime, so the
// non-Send executor stored in Node never crosses thread boundaries.
#[allow(clippy::future_not_send)]
async fn run_node_loop_async(
    mut rx: mpsc::Receiver<Request>,
    mut node: Node,
    block_interval: Duration,
) -> anyhow::Result<()> {
    let mut next_block_at = Instant::now() + block_interval;
    let mut recovery_points = RecoveryPoints::default();

    loop {
        if Instant::now() >= next_block_at {
            next_block_at = mine_scheduled_block(&mut node, block_interval);
            continue;
        }

        tokio::select! {
            biased;
            () = tokio::time::sleep_until(next_block_at) => {
                next_block_at = mine_scheduled_block(&mut node, block_interval);
            }
            req = rx.recv() => {
                let Some(req) = req else {
                    return Ok(());
                };
                process_loop_request(&mut node, &mut recovery_points, req);
            }
        }
    }
}

fn mine_scheduled_block(node: &mut Node, block_interval: Duration) -> Instant {
    tracing::info!("Mining localnet block");
    if let Err(e) = node.mine_block() {
        tracing::error!("Block mining failed: {:?}", e);
    }
    Instant::now() + block_interval
}

fn handle_mine_blocks(node: &mut Node, count: u32) -> anyhow::Result<LocalnetMineResult> {
    anyhow::ensure!(count > 0, "blocks must be greater than 0");

    let mut blocks = Vec::with_capacity(count as usize);
    for _ in 0..count {
        tracing::info!("Manually mining localnet block");
        let block = node.mine_block()?;
        blocks.push(block.block_id());
    }

    let last_block_seqno = blocks
        .last()
        .map_or(node.globals.head_seqno, |block| block.seqno);
    Ok(LocalnetMineResult {
        blocks_mined: count,
        last_block_seqno,
        blocks,
    })
}

fn process_loop_request(node: &mut Node, recovery_points: &mut RecoveryPoints, req: Request) {
    tracing::debug!("Node loop processing request: {:?}", req);
    match req {
        Request::SendBoc { boc, resp } => {
            let res = handle_send_boc(node, boc);
            let _ = resp.send(res);
        }
        Request::SendInternalBoc { boc, resp } => {
            let res = handle_send_internal_boc(node, boc);
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
        Request::GetAccountStates {
            addresses,
            seqno,
            resp,
        } => {
            let res = handle_get_account_states(node, addresses, seqno);
            let _ = resp.send(res);
        }
        Request::GetAddressInfos { addresses, resp } => {
            let res = handle_get_address_infos(node, addresses);
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
            let res = node
                .faucet(&address, amount)
                .map(|msg_hash| LocalnetAcceptedInternalMessage { msg_hash });
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
            let res = handle_get_jetton_masters(node, address, admin_address, limit, offset);
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
            let res = handle_get_jetton_wallets(
                node,
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
            let res = handle_get_nft_items(
                node,
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
        Request::GetAddressNames { addresses, resp } => {
            let res = addresses
                .iter()
                .map(|address| node.history.address_names.get(address).cloned())
                .collect();
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
        Request::GetCompilerAbis { code_hashes, resp } => {
            let res = code_hashes
                .iter()
                .map(|code_hash| {
                    node.history
                        .get_compiler_abi(code_hash)
                        .map(|compiler_abi| {
                            registered_compiler_abi_payload(code_hash, compiler_abi)
                        })
                        .or_else(|| catalog_compiler_abi_payload(code_hash))
                })
                .collect();
            let _ = resp.send(Ok(res));
        }
        Request::DumpState { path, resp } => {
            let res = node.dump_state_to_path(path);
            let _ = resp.send(res);
        }
        Request::LoadState { path, resp } => {
            let res = node.load_state_from_path(path);
            if res.is_ok() {
                recovery_points.clear();
            }
            let _ = resp.send(res);
        }
        Request::CreateRecoveryPoint { resp } => {
            let res = recovery_points.create(node);
            let _ = resp.send(res);
        }
        Request::RevertRecoveryPoint { id, resp } => {
            let res = recovery_points.revert(node, id);
            let _ = resp.send(res);
        }
        Request::MineBlocks { count, resp } => {
            let res = handle_mine_blocks(node, count);
            let _ = resp.send(res);
        }
        Request::GetClockInfo { resp } => {
            let res = node.clock_info();
            let _ = resp.send(res);
        }
        Request::IncreaseTime { seconds, resp } => {
            let res = node.increase_time(seconds);
            let _ = resp.send(res);
        }
        Request::SetTime { timestamp, resp } => {
            let res = node.set_time(timestamp);
            let _ = resp.send(res);
        }
        Request::SetNextBlockTimestamp { timestamp, resp } => {
            let res = node.set_next_block_timestamp(timestamp);
            let _ = resp.send(res);
        }
    }
}

fn registered_compiler_abi_payload(
    code_hash: &Hash256,
    compiler_abi: Value,
) -> acton_abi_catalog::ExtendedContractAbi<Value> {
    let display_name = compiler_abi
        .get("contract_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned);

    acton_abi_catalog::ExtendedContractAbi {
        compiler_abi,
        display_name,
        code_hashes: vec![code_hash.to_hex()],
        links: Vec::new(),
    }
}

fn catalog_compiler_abi_payload(
    code_hash: &Hash256,
) -> Option<acton_abi_catalog::ExtendedContractAbi<Value>> {
    let contract = acton_abi_catalog::find_contract_by_code_hash(&code_hash.to_hex())?;
    let extended_abi = contract.extended_abi();
    let compiler_abi = serde_json::to_value(&extended_abi.compiler_abi).ok()?;

    Some(acton_abi_catalog::ExtendedContractAbi {
        compiler_abi,
        display_name: extended_abi.display_name,
        code_hashes: extended_abi.code_hashes,
        links: extended_abi.links,
    })
}

fn handle_send_boc(
    node: &mut Node,
    boc: BocBytes,
) -> anyhow::Result<LocalnetAcceptedExternalMessage> {
    let msg_hash_norm = normalized_ext_in_hash_from_boc(&boc)?
        .context("sendBoc accepts only external-in messages")?;
    let msg_hash = node.send_boc(boc)?;
    Ok(LocalnetAcceptedExternalMessage {
        msg_hash,
        msg_hash_norm,
    })
}

fn handle_send_internal_boc(
    node: &mut Node,
    boc: BocBytes,
) -> anyhow::Result<LocalnetAcceptedInternalMessage> {
    let msg_hash = node.send_internal_boc(boc)?;
    Ok(LocalnetAcceptedInternalMessage { msg_hash })
}

fn handle_get_address_info(
    node: &mut Node,
    address: Addr,
    seqno: Option<u32>,
) -> anyhow::Result<LocalnetAccountState> {
    let seqno = account_query_seqno(node, seqno);
    let meta = node.get_address_information_at_block(&address, seqno);
    let block_id = block_id_for_query_seqno(node, seqno)?;
    let sync_utime = u64::from(node.now_unix()?);

    let Some(meta) = meta else {
        return Ok(LocalnetAccountState::empty(address, block_id, sync_utime));
    };

    let code = meta.code_hash.and_then(|h| node.get_cell(&h));
    let data = meta.data_hash.and_then(|h| node.get_cell(&h));
    let last_transaction_id = meta.last_tx_id();

    Ok(LocalnetAccountState {
        address,
        account_state_hash: meta.account_hash,
        balance: meta.balance,
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

fn handle_get_account_states(
    node: &mut Node,
    addresses: Vec<Addr>,
    seqno: Option<u32>,
) -> anyhow::Result<Vec<LocalnetAccountStateWithInfo>> {
    addresses
        .into_iter()
        .map(|address| {
            let state = handle_get_address_info(node, address, seqno)?;
            let info = handle_get_address_context(node, address)?;
            Ok(LocalnetAccountStateWithInfo { state, info })
        })
        .collect()
}

fn handle_get_address_infos(
    node: &mut Node,
    addresses: Vec<Addr>,
) -> anyhow::Result<Vec<LocalnetAddressInfo>> {
    addresses
        .into_iter()
        .map(|address| handle_get_address_context(node, address))
        .collect()
}

fn handle_get_address_context(
    node: &mut Node,
    address: Addr,
) -> anyhow::Result<LocalnetAddressInfo> {
    node.ensure_detected_assets_for_address(&address)?;

    let code_hash = node
        .get_address_information(&address)
        .and_then(|meta| meta.code_hash);
    let jetton_wallet = node
        .iter_jetton_wallets()
        .find(|wallet| wallet.address == address)
        .cloned();
    let jetton_master = node
        .iter_jetton_masters()
        .find(|master| master.address == address)
        .cloned();
    let nft_item = node
        .iter_nft_items()
        .find(|item| item.address == address)
        .cloned();
    let nft_collection_item = node
        .iter_nft_items()
        .find(|item| item.collection_address == Some(address))
        .cloned();

    Ok(LocalnetAddressInfo {
        address,
        code_hash,
        jetton_wallet,
        jetton_master,
        nft_item,
        nft_collection_item,
    })
}

const fn account_query_seqno(node: &Node, seqno: Option<Seqno>) -> Seqno {
    match seqno {
        Some(0) | None => node.globals.head_seqno,
        Some(seqno) => seqno,
    }
}

fn block_id_for_query_seqno(node: &Node, seqno: Seqno) -> anyhow::Result<LocalnetBlockId> {
    if seqno == 0 {
        return Ok(LocalnetBlockId::first());
    }

    node.get_block_header(seqno)
        .map(|block| block.block_id())
        .ok_or_else(|| anyhow::anyhow!("Block {seqno} not found"))
}

fn handle_get_jetton_masters(
    node: &mut Node,
    address: Option<Addr>,
    admin_address: Option<Addr>,
    limit: usize,
    offset: usize,
) -> anyhow::Result<Vec<storage::JettonMasterMeta>> {
    if let Some(addr) = address {
        node.ensure_detected_assets_for_address(&addr)?;
    }

    Ok(node
        .iter_jetton_masters()
        .filter(|master| {
            if let Some(addr) = address
                && master.address != addr
            {
                return false;
            }
            if let Some(addr) = admin_address
                && master.admin_address != Some(addr)
            {
                return false;
            }
            true
        })
        .skip(offset)
        .take(limit)
        .cloned()
        .collect())
}

fn handle_get_jetton_wallets(
    node: &mut Node,
    address: Option<Addr>,
    owner_address: Option<Addr>,
    jetton_address: Option<Addr>,
    exclude_zero_balance: bool,
    limit: usize,
    offset: usize,
) -> anyhow::Result<Vec<storage::JettonWalletMeta>> {
    if let Some(addr) = address {
        node.ensure_detected_assets_for_address(&addr)?;
    }

    Ok(node
        .iter_jetton_wallets()
        .filter(|wallet| {
            if let Some(addr) = address
                && wallet.address != addr
            {
                return false;
            }
            if let Some(addr) = owner_address
                && wallet.owner_address != addr
            {
                return false;
            }
            if let Some(addr) = jetton_address
                && wallet.jetton_address != addr
            {
                return false;
            }
            if exclude_zero_balance && wallet.balance == 0 {
                return false;
            }
            true
        })
        .skip(offset)
        .take(limit)
        .cloned()
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn handle_get_nft_items(
    node: &mut Node,
    address: Option<Addr>,
    owner_address: Option<Addr>,
    collection_address: Option<Addr>,
    index: Option<String>,
    sort_by_last_transaction_lt: bool,
    limit: usize,
    offset: usize,
) -> anyhow::Result<Vec<storage::NftItemMeta>> {
    if let Some(addr) = address {
        node.ensure_detected_assets_for_address(&addr)?;
    }

    let items = node.iter_nft_items().filter(|item| {
        if let Some(addr) = address
            && item.address != addr
        {
            return false;
        }
        if let Some(addr) = owner_address
            && item.owner_address != Some(addr)
        {
            return false;
        }
        if let Some(addr) = collection_address
            && item.collection_address != Some(addr)
        {
            return false;
        }
        if let Some(expected_index) = &index
            && &item.index != expected_index
        {
            return false;
        }
        true
    });

    if sort_by_last_transaction_lt {
        let mut items = items.cloned().collect::<Vec<_>>();
        items.sort_by(|a, b| {
            b.last_transaction_lt
                .cmp(&a.last_transaction_lt)
                .then_with(|| a.address.cmp(&b.address))
        });
        let start = offset.min(items.len());
        let end = start.saturating_add(limit).min(items.len());
        items.truncate(end);
        items.drain(..start);
        return Ok(items);
    }

    Ok(items.skip(offset).take(limit).cloned().collect())
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
    let seqno = account_query_seqno(node, seqno);
    let meta = node.get_address_information_at_block(&address, seqno);
    let block_id = block_id_for_query_seqno(node, seqno)?;

    let Some(meta) = meta else {
        return no_code_run_get_method_result(
            method_id,
            block_id,
            LocalnetTransactionId::default(),
        );
    };

    let last_transaction_id = meta.last_tx_id();
    let Some(code_hash) = meta.code_hash else {
        return no_code_run_get_method_result(method_id, block_id, last_transaction_id);
    };

    let code_boc = node.get_cell_or_empty(Some(code_hash)).to_base64();
    let data_boc = node.get_cell_or_empty(meta.data_hash).to_base64();
    let libs = node
        .build_vm_global_libs_boc()?
        .map_or_else(String::new, |boc| boc.to_base64());

    let args = RunGetMethodArgs {
        code: code_boc,
        data: data_boc,
        method_id,
        address: address.to_string(),
        unixtime: i64::from(node.now_unix()?),
        balance: meta.balance.to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        gas_limit: "10000000".to_owned(),
        debug_enabled: false,
        verbosity: ExecutorVerbosity::Short,
        libs,
        extra_currencies: Default::default(),
        prev_blocks_info: Some(
            node.prev_blocks_info_at(seqno)
                .to_stack_entry_boc_base64()?,
        ),
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
        GetMethodResult::Success(s) => Ok(LocalnetRunGetMethodResult {
            gas_used: s.gas_used.parse().unwrap_or(0),
            stack: BocBytes::from_base64(s.stack.as_ref()).unwrap_or_default(),
            exit_code: s.vm_exit_code,
            vm_log: s.vm_log,
            block_id,
            last_transaction_id,
        }),
        GetMethodResult::Error(e) => anyhow::bail!("Get method error: {e:?}"),
    }
}

fn no_code_run_get_method_result(
    method_id: i32,
    block_id: LocalnetBlockId,
    last_transaction_id: LocalnetTransactionId,
) -> anyhow::Result<LocalnetRunGetMethodResult> {
    let stack = Tuple(vec![TupleItem::Int(method_id.into())])
        .serialize()
        .context("Failed to serialize no-code get-method stack to BoC")?;
    Ok(LocalnetRunGetMethodResult {
        gas_used: 0,
        stack: BocBytes::from(Boc::encode(stack)),
        exit_code: -13,
        vm_log: Arc::from(""),
        block_id,
        last_transaction_id,
    })
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
            bounce: false,
            bounced: false,
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

    let (fwd_fee, ihr_fee, bounce, bounced) = match &msg.info {
        MsgInfo::Int(info) => (
            info.fwd_fee.into(),
            info.ihr_fee.into(),
            info.bounce,
            info.bounced,
        ),
        _ => (0, 0, false, false),
    };

    // Extract opcode, skipping the bounce prefix for bounced internal messages.
    let mut opcode = None;
    let mut body_slice = msg.body;
    if bounced {
        let _ = body_slice.load_uint(32);
    }
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
        bounce,
        bounced,
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
    for (hash, entry) in hashes.iter().copied().zip(entries) {
        if let Some(entry) = entry {
            result.push(LocalnetLibrary {
                hash: entry.hash,
                found: true,
                data: Some(entry.lib_boc),
                publishers_count: entry.publishers.len(),
                publishers: entry.publishers.into_iter().collect(),
            });
        } else {
            result.push(LocalnetLibrary {
                hash,
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

#[cfg(test)]
mod tests {
    use super::*;
    use tycho_types::boc::BocRepr;
    use tycho_types::cell::{CellSliceParts, HashBytes};
    use tycho_types::models::{CurrencyCollection, IntAddr, IntMsgInfo, OwnedMessage};

    const REGULAR_OPCODE: u32 = 0x178d_4519;
    const BOUNCE_PREFIX: u32 = 0xffff_ffff;

    #[test]
    fn convert_to_message_struct_extracts_regular_internal_opcode() {
        let message = internal_message_boc(false, &[REGULAR_OPCODE]);
        let hash = message.hash().expect("message must hash");
        let mapped =
            convert_to_message_struct(&message_meta(hash), &message).expect("message must map");

        assert_eq!(mapped.opcode, Some(REGULAR_OPCODE));
        assert!(!mapped.bounced);
    }

    #[test]
    fn convert_to_message_struct_extracts_bounced_opcode_after_prefix() {
        let message = internal_message_boc(true, &[BOUNCE_PREFIX, REGULAR_OPCODE]);
        let hash = message.hash().expect("message must hash");
        let mapped =
            convert_to_message_struct(&message_meta(hash), &message).expect("message must map");

        assert_eq!(mapped.opcode, Some(REGULAR_OPCODE));
        assert!(mapped.bounced);
    }

    fn internal_message_boc(bounced: bool, body_words: &[u32]) -> BocBytes {
        let mut body = CellBuilder::new();
        for word in body_words {
            body.store_u32(*word).expect("body word must store");
        }
        let body = body.build().expect("body cell must build");
        let message = OwnedMessage {
            info: MsgInfo::Int(IntMsgInfo {
                ihr_disabled: true,
                bounce: false,
                bounced,
                src: IntAddr::Std(test_std_addr(0x11)),
                dst: IntAddr::Std(test_std_addr(0x22)),
                value: CurrencyCollection::new(1),
                ihr_fee: Default::default(),
                fwd_fee: Default::default(),
                created_at: 0,
                created_lt: 0,
            }),
            init: None,
            body: CellSliceParts::from(body),
            layout: None,
        };

        BocRepr::encode(message)
            .expect("internal message must encode")
            .into()
    }

    fn message_meta(hash: Hash256) -> MsgMeta {
        MsgMeta {
            msg_hash: hash,
            msg_boc_hash: hash,
            src: Some(test_addr(0x11)),
            dst: Some(test_addr(0x22)),
            value: Some(1),
            bounce: Some(false),
            created_lt: Some(0),
            created_at: Some(0),
        }
    }

    fn test_addr(byte: u8) -> Addr {
        Addr {
            workchain: 0,
            addr: [byte; 32],
        }
    }

    fn test_std_addr(byte: u8) -> StdAddr {
        StdAddr {
            anycast: None,
            address: HashBytes([byte; 32]),
            workchain: 0,
        }
    }
}
