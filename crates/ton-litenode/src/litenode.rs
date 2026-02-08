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
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use ton_executor::DEFAULT_CONFIG;
use ton_executor::ExecutorVerbosity;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::json_stack::json_to_legacy_item;
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::{CellFamily, Store};
use tycho_types::models::{Message, StdAddr, StdAddrFormat};

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeBlockId {
    pub workchain: i32,
    pub shard: i64,
    pub seqno: Seqno,
    pub root_hash: Hash256,
    pub file_hash: Hash256,
}

impl LiteNodeBlockId {
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
pub struct LiteNodeAccountState {
    pub address: Addr,
    pub balance: u128,
    pub code: Option<BocBytes>,
    pub data: Option<BocBytes>,
    pub last_transaction_id: LiteNodeTransactionId,
    pub block_id: LiteNodeBlockId,
    pub state: AccountStatus,
    pub sync_utime: u64,
    pub frozen_hash: Option<Hash256>,
}

impl LiteNodeAccountState {
    pub fn empty(address: Addr, block_id: LiteNodeBlockId, sync_utime: u64) -> Self {
        Self {
            address,
            balance: 0,
            code: None,
            data: None,
            last_transaction_id: LiteNodeTransactionId::default(),
            block_id,
            state: AccountStatus::Uninit,
            sync_utime,
            frozen_hash: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LiteNodeTransactionId {
    pub lt: Lt,
    pub hash: Hash256,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeTransaction {
    pub hash: Hash256,
    pub address: Addr,
    pub utime: u32,
    pub data: BocBytes,
    pub success: bool,
    pub exit_code: i32,
    pub transaction_id: LiteNodeTransactionId,
    pub in_msg: LiteNodeMessage,
    pub out_msgs: Vec<LiteNodeMessage>,
    pub total_fees: u128,
    pub storage_fees: u128,
    pub other_fees: u128,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeMessage {
    pub hash: Hash256,
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
pub struct LiteNodeRunGetMethodResult {
    pub gas_used: u64,
    pub stack: BocBytes,
    pub exit_code: i32,
    pub vm_log: String,
    pub block_id: LiteNodeBlockId,
    pub last_transaction_id: LiteNodeTransactionId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeMasterchainInfo {
    pub last: LiteNodeBlockId,
    pub state_root_hash: Hash256,
    pub init: LiteNodeBlockId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeBlockHeader {
    pub id: LiteNodeBlockId,
    pub gen_utime: u32,
    pub start_lt: Lt,
    pub end_lt: Lt,
    pub prev_seqno: Option<Seqno>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeBlockTransactions {
    pub id: LiteNodeBlockId,
    pub transactions: Vec<LiteNodeTransaction>,
    pub msg_hash: Option<Hash256>,
}

#[derive(Debug)]
pub(crate) enum Request {
    SendBoc {
        boc: BocBytes,
        resp: oneshot::Sender<anyhow::Result<LiteNodeBlockTransactions>>,
    },
    GetAddressInformation {
        address: Addr,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LiteNodeAccountState>>,
    },
    GetTransactions {
        address: Addr,
        limit: usize,
        lt: Option<u64>,
        hash: Option<Hash256>,
        to_lt: Option<u64>,
        resp: oneshot::Sender<anyhow::Result<Vec<LiteNodeTransaction>>>,
    },
    RunGetMethod {
        address: Addr,
        method_id: i32,
        stack: Tuple,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LiteNodeRunGetMethodResult>>,
    },
    GetBlockHeader {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<LiteNodeBlockHeader>>,
    },
    GetBlockTransactions {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<LiteNodeBlockTransactions>>,
    },
    GetMasterchainInfo {
        resp: oneshot::Sender<anyhow::Result<LiteNodeMasterchainInfo>>,
    },
    GetShards {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<Vec<LiteNodeBlockId>>>,
    },
    LookupBlock {
        #[allow(dead_code)] // unused since litenode have only one workchain
        workchain: i32,
        #[allow(dead_code)] // unused since litenode have only one shard
        shard: i64,
        seqno: Option<u32>,
        lt: Option<u64>,
        unixtime: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LiteNodeBlockId>>,
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
    SetAddressName {
        address: Addr,
        name: String,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    GetAddressName {
        address: Addr,
        resp: oneshot::Sender<anyhow::Result<Option<String>>>,
    },
    SetStateSource {
        source: StateSource,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    GetStateSource {
        resp: oneshot::Sender<anyhow::Result<StateSource>>,
    },
}

pub struct LiteNode {
    tx: mpsc::Sender<Request>,
}

impl Default for LiteNode {
    fn default() -> Self {
        Self::new(StateSource::Local, None)
    }
}

impl LiteNode {
    pub fn new(state_source: StateSource, db_path: Option<String>) -> Self {
        let (tx, rx) = mpsc::channel(100);

        std::thread::spawn(move || {
            if let Err(e) = run_node_loop(rx, state_source, db_path) {
                tracing::error!("Node loop failed: {:?}", e);
            }
        });

        Self { tx }
    }

    pub async fn send_boc(&self, boc_str: String) -> anyhow::Result<LiteNodeBlockTransactions> {
        let boc = base64::engine::general_purpose::STANDARD
            .decode(&boc_str)
            .context("Invalid BOC base64")?;
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::SendBoc { boc, resp }).await?;
        rx.await?
    }

    pub async fn get_address_information(
        &self,
        address_str: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<LiteNodeAccountState> {
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

    pub async fn get_transactions(
        &self,
        address_str: String,
        limit: usize,
        lt: Option<u64>,
        hash_str: Option<String>,
        to_lt: Option<u64>,
    ) -> anyhow::Result<Vec<LiteNodeTransaction>> {
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

    pub async fn run_get_method(
        &self,
        address_str: String,
        method: String,
        stack_json: Vec<Value>,
        seqno: Option<u32>,
    ) -> anyhow::Result<LiteNodeRunGetMethodResult> {
        let address = Self::parse_addr(&address_str)?;
        let method_id = if let Ok(id) = method.parse::<i32>() {
            id
        } else {
            let crc = CRC16.checksum(method.as_bytes());
            (crc as i32 & 0xffff) | 0x10000
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

    pub async fn get_block_header(&self, seqno: u32) -> anyhow::Result<LiteNodeBlockHeader> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetBlockHeader { seqno, resp })
            .await?;
        rx.await?
    }

    pub async fn get_block_transactions(
        &self,
        seqno: u32,
    ) -> anyhow::Result<LiteNodeBlockTransactions> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetBlockTransactions { seqno, resp })
            .await?;
        rx.await?
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<LiteNodeMasterchainInfo> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetMasterchainInfo { resp }).await?;
        rx.await?
    }

    pub async fn get_shards(&self, seqno: u32) -> anyhow::Result<Vec<LiteNodeBlockId>> {
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
    ) -> anyhow::Result<LiteNodeBlockId> {
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

    pub async fn get_traces(&self, tx_hash_str: String) -> anyhow::Result<storage::TraceNode> {
        let tx_hash = Hash256::from_hex(&tx_hash_str)?;
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetTraces { tx_hash, resp }).await?;
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

    pub async fn set_state_source(&self, source: StateSource) -> anyhow::Result<()> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::SetStateSource { source, resp })
            .await?;
        rx.await?
    }

    pub async fn get_state_source(&self) -> anyhow::Result<StateSource> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetStateSource { resp }).await?;
        rx.await?
    }

    fn parse_addr(s: &str) -> anyhow::Result<Addr> {
        let (int_addr, _) = StdAddr::from_str_ext(s, StdAddrFormat::any()).map_err(|_| {
            anyhow::anyhow!("Invalid address, only standard internal address is allowed")
        })?;
        Ok(Addr {
            workchain: int_addr.workchain as i32,
            addr: int_addr.address.0,
        })
    }
}

fn run_node_loop(
    mut rx: mpsc::Receiver<Request>,
    state_source: StateSource,
    db_path: Option<String>,
) -> anyhow::Result<()> {
    let executor = Box::new(TvmEmulatorAdapter::new()?);
    let config_bytes = base64::engine::general_purpose::STANDARD.decode(DEFAULT_CONFIG)?;
    let mut node = Node::with_db_path(executor, config_bytes, state_source, db_path)?;

    tracing::info!("TON lite node started");

    loop {
        // 1. Process all currently pending requests
        while let Ok(req) = rx.try_recv() {
            process_loop_request(&mut node, req);
        }

        // 2. If there are pending messages in the pool, mine one
        if node.has_pending_messages() {
            tracing::info!("Auto-mining message from pool");
            if let Err(e) = node.mine_one() {
                tracing::error!("Auto-mining failed: {:?}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }

        // 3. If pool is empty, block until next request
        if let Some(req) = rx.blocking_recv() {
            process_loop_request(&mut node, req);
        } else {
            break; // Channel closed
        }
    }
    Ok(())
}

fn process_loop_request(node: &mut Node, req: Request) {
    tracing::debug!("Node loop processing request: {:?}", req);
    match req {
        Request::SendBoc { boc, resp } => {
            let res = handle_send_boc(node, boc);
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
        Request::GetShards { seqno, resp } => {
            let res = handle_get_shards(node, seqno);
            let _ = resp.send(res);
        }
        Request::LookupBlock {
            workchain: _, // unused since litenode have only one workchain
            shard: _,     // unused since litenode have only one shard
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
            let res = node.faucet(&address, amount);
            let _ = resp.send(res);
        }
        Request::GetTraces { tx_hash, resp } => {
            let res = node.get_traces(&tx_hash);
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
        Request::SetStateSource { source, resp } => {
            node.state_source = source;
            let _ = resp.send(Ok(()));
        }
        Request::GetStateSource { resp } => {
            let _ = resp.send(Ok(node.state_source.clone()));
        }
    }
}

fn handle_send_boc(node: &mut Node, boc: BocBytes) -> anyhow::Result<LiteNodeBlockTransactions> {
    let (msg_hash, tx_hash, seqno, _) = node.send_boc(boc)?;

    let Some(ext_tx) = node.get_transaction_by_hash(&tx_hash) else {
        anyhow::bail!("Transaction not found after mining")
    };

    let tx_boc = node.get_cell(&tx_hash).unwrap_or_default();
    let tx_struct = convert_to_tx_struct(&ext_tx, tx_boc)?;

    let Some(block_header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block {seqno} with transaction not found after mining")
    };

    Ok(LiteNodeBlockTransactions {
        id: block_header.block_id(),
        transactions: vec![tx_struct],
        msg_hash: Some(msg_hash),
    })
}

fn handle_get_address_info(
    node: &mut Node,
    address: Addr,
    seqno: Option<u32>,
) -> anyhow::Result<LiteNodeAccountState> {
    let seqno = seqno.unwrap_or(node.globals.head_seqno);
    let meta = node.get_address_information_at_block(&address, seqno);

    let Some(block_header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block {seqno} not found")
    };

    let block_id = block_header.block_id();
    let sync_utime = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let Some(meta) = meta else {
        return Ok(LiteNodeAccountState::empty(address, block_id, sync_utime));
    };

    let code = meta.code_hash.and_then(|h| node.get_cell(&h));
    let data = meta.data_hash.and_then(|h| node.get_cell(&h));
    let last_transaction_id = meta.last_tx_id();

    Ok(LiteNodeAccountState {
        address,
        balance: meta.cached_balance.unwrap_or(0),
        code,
        data,
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
) -> anyhow::Result<Vec<LiteNodeTransaction>> {
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

fn handle_run_get_method(
    node: &mut Node,
    address: Addr,
    method_id: i32,
    stack: Tuple,
    seqno: Option<u32>,
) -> anyhow::Result<LiteNodeRunGetMethodResult> {
    let seqno = seqno.unwrap_or(node.globals.head_seqno);
    let meta = node.get_address_information_at_block(&address, seqno);
    let meta = meta.ok_or_else(|| anyhow::anyhow!("Account {address} not found"))?;

    let Some(block_header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block {seqno} not found")
    };

    let block_id = block_header.block_id();
    let last_transaction_id = meta.last_tx_id();

    let code_boc = meta
        .code_hash
        .and_then(|h| node.get_cell(&h))
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
        .unwrap_or_else(|| EMPTY_CELL_BASE64.to_owned());

    let data_boc = meta
        .data_hash
        .and_then(|h| node.get_cell(&h))
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
        .unwrap_or_else(|| EMPTY_CELL_BASE64.to_owned());

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
        libs: String::new(),
        extra_currencies: Default::default(),
        prev_blocks_info: None,
    };

    let stack_cell = stack
        .serialize()
        .context("Failed to serialize stack to BoC")?;
    let stack_b64 = stack_cell
        .to_boc_b64(false)
        .context("Failed to encode stack to Base64")?;

    let exec = GetExecutor::new(&args).context("Failed to create GetExecutor")?;

    let res = exec
        .run_get_method(&stack_b64, &args, None)
        .context("Execution failed")?;

    match res {
        GetMethodResult::Success(s) => {
            let stack_bytes = base64::engine::general_purpose::STANDARD
                .decode(&s.stack)
                .unwrap_or_default();
            Ok(LiteNodeRunGetMethodResult {
                gas_used: s.gas_used.parse().unwrap_or(0),
                stack: stack_bytes,
                exit_code: s.vm_exit_code,
                vm_log: s.vm_log,
                block_id,
                last_transaction_id,
            })
        }
        GetMethodResult::Error(e) => anyhow::bail!("Get method error: {:?}", e),
    }
}

pub(crate) fn convert_to_tx_struct(
    tx: &TransactionInfo,
    tx_boc: Vec<u8>,
) -> anyhow::Result<LiteNodeTransaction> {
    let in_msg_struct = if let Some(in_msg) = &tx.in_msg {
        convert_to_message_struct(&in_msg.meta, &in_msg.boc)?
    } else {
        LiteNodeMessage {
            hash: Hash256([0; 32]),
            source: None,
            destination: None,
            value: 0,
            body_hash: Hash256([0; 32]),
            body: Vec::new(),
            init_state: Vec::new(),
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

    Ok(LiteNodeTransaction {
        hash: tx.meta.tx_hash,
        address: tx.meta.account,
        utime: tx.meta.now,
        data: tx_boc,
        success: tx.meta.success,
        exit_code: tx.meta.compute_exit_code.unwrap_or(0),
        transaction_id: LiteNodeTransactionId {
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

fn convert_to_message_struct(meta: &MsgMeta, boc: &[u8]) -> anyhow::Result<LiteNodeMessage> {
    let cell = Boc::decode(boc)?;
    let msg = cell.parse::<Message<'_>>()?;

    // Extract body
    let mut builder = tycho_types::cell::CellBuilder::new();
    builder.store_slice(msg.body)?;
    let body_cell = builder.build()?;
    let body_hash = Hash256(*body_cell.repr_hash().as_array());
    let body_bytes = Boc::encode(body_cell);

    let (fwd_fee, ihr_fee) = match &msg.info {
        tycho_types::models::MsgInfo::Int(info) => (info.fwd_fee.into(), info.ihr_fee.into()),
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
        let mut builder = tycho_types::cell::CellBuilder::new();
        let _ = init.store_into(&mut builder, tycho_types::cell::Cell::empty_context());
        if let Ok(cell) = builder.build() {
            init_state_bytes = Boc::encode(cell);
        }
    }

    Ok(LiteNodeMessage {
        hash: meta.msg_hash,
        source: meta.src,
        destination: meta.dst,
        value: meta.value.unwrap_or(0),
        body_hash,
        body: body_bytes,
        init_state: init_state_bytes,
        opcode,
        fwd_fee,
        ihr_fee,
        created_lt: meta.created_lt.unwrap_or(0),
    })
}

fn handle_get_block_header(node: &Node, seqno: u32) -> anyhow::Result<LiteNodeBlockHeader> {
    let Some(header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block {seqno} not found")
    };

    Ok(LiteNodeBlockHeader {
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
) -> anyhow::Result<LiteNodeBlockTransactions> {
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

    Ok(LiteNodeBlockTransactions {
        id: block_id,
        transactions: result,
        msg_hash: None,
    })
}

fn handle_get_masterchain_info(node: &Node) -> anyhow::Result<LiteNodeMasterchainInfo> {
    let head_block = node.get_block_header(node.globals.head_seqno);
    let block_id = head_block
        .as_ref()
        .map(BlockMeta::block_id)
        .unwrap_or_else(LiteNodeBlockId::first);

    Ok(LiteNodeMasterchainInfo {
        state_root_hash: block_id.root_hash,
        last: block_id,
        init: LiteNodeBlockId::first(),
    })
}

fn handle_get_shards(node: &Node, seqno: u32) -> anyhow::Result<Vec<LiteNodeBlockId>> {
    let Some(block_header) = node.get_block_header(seqno) else {
        anyhow::bail!("Block not found for seqno={seqno}")
    };
    Ok(vec![block_header.block_id()])
}

fn handle_lookup_block(
    node: &Node,
    seqno: Option<u32>,
    lt: Option<u64>,
    unixtime: Option<u32>,
) -> anyhow::Result<LiteNodeBlockId> {
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
