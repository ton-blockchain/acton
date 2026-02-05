use crate::executor::TvmEmulatorAdapter;
use crate::node::Node;
use crate::storage;
use crate::storage::{AccountStatus, MsgMeta, TransactionInfo};
use crate::types::{Addr, Hash256, Lt, Seqno};
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
use tvmffi::json_stack::json_to_legacy_stack;
use tycho_types::boc::Boc;
use tycho_types::cell::{CellFamily, Store};
use tycho_types::models::{Message, StdAddr, StdAddrFormat};

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeBlockId {
    pub workchain: i32,
    pub shard: String,
    pub seqno: Seqno,
    pub root_hash: Hash256,
    pub file_hash: Hash256,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeAccountState {
    pub address: Addr,
    pub balance: u128,
    pub code: String, // base64
    pub data: String, // base64
    pub last_transaction_id: Option<LiteNodeTransactionId>,
    pub block_id: LiteNodeBlockId,
    pub state: AccountStatus,
    pub sync_utime: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeTransactionId {
    pub lt: Lt,
    pub hash: Hash256,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeTransaction {
    pub hash: Hash256,
    pub address: Addr,
    pub utime: u32,
    pub data: String, // base64
    pub success: bool,
    pub exit_code: i32,
    pub transaction_id: LiteNodeTransactionId,
    pub in_msg: LiteNodeMessage,
    pub out_msgs: Vec<LiteNodeMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeMessage {
    pub hash: Hash256,
    pub source: Option<Addr>,
    pub destination: Option<Addr>,
    pub value: u128,
    pub body_hash: String,
    pub body: String,       // base64
    pub init_state: String, // base64
    pub opcode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiteNodeRunGetMethodResult {
    pub gas_used: u64,
    pub stack: String, // base64 BOC
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
}

#[derive(Debug)]
pub(crate) enum Request {
    SendBoc {
        boc: String,
        resp: oneshot::Sender<anyhow::Result<LiteNodeBlockTransactions>>,
    },
    GetAddressInformation {
        address: String,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LiteNodeAccountState>>,
    },
    GetTransactions {
        address: String,
        limit: u32,
        lt: Option<u64>,
        hash: Option<String>,
        to_lt: Option<u64>,
        resp: oneshot::Sender<anyhow::Result<Vec<LiteNodeTransaction>>>,
    },
    RunGetMethod {
        address: String,
        method: String,
        stack: Vec<Value>,
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
        _workchain: i32,
        _shard: String,
        seqno: Option<u32>,
        lt: Option<u64>,
        unixtime: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<LiteNodeBlockId>>,
    },
    Faucet {
        address: String,
        amount: u128,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetTraces {
        tx_hash: String,
        resp: oneshot::Sender<anyhow::Result<storage::TraceNode>>,
    },
    SetStateSource {
        source: crate::node::StateSource,
        resp: oneshot::Sender<anyhow::Result<()>>,
    },
    GetStateSource {
        resp: oneshot::Sender<anyhow::Result<crate::node::StateSource>>,
    },
}

pub struct LiteNode {
    tx: mpsc::Sender<Request>,
}

impl Default for LiteNode {
    fn default() -> Self {
        Self::new(crate::node::StateSource::Local, None)
    }
}

impl LiteNode {
    pub fn new(state_source: crate::node::StateSource, db_path: Option<String>) -> Self {
        let (tx, rx) = mpsc::channel(100);

        std::thread::spawn(move || {
            if let Err(e) = run_node_loop(rx, state_source, db_path) {
                tracing::error!("Node loop failed: {:?}", e);
            }
        });

        Self { tx }
    }

    pub async fn send_boc(&self, boc: String) -> anyhow::Result<LiteNodeBlockTransactions> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::SendBoc { boc, resp }).await?;
        rx.await?
    }

    pub async fn get_address_information(
        &self,
        address: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<LiteNodeAccountState> {
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
        address: String,
        limit: u32,
        lt: Option<u64>,
        hash: Option<String>,
        to_lt: Option<u64>,
    ) -> anyhow::Result<Vec<LiteNodeTransaction>> {
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
        address: String,
        method: String,
        stack: Vec<Value>,
        seqno: Option<u32>,
    ) -> anyhow::Result<LiteNodeRunGetMethodResult> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::RunGetMethod {
                address,
                method,
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
        let info = self.get_address_information(address, seqno).await;
        info.map(|i| i.balance)
    }

    pub async fn get_address_state(
        &self,
        address: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<AccountStatus> {
        let info = self.get_address_information(address, seqno).await;
        info.map(|i| i.state)
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
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::LookupBlock {
                _workchain: workchain,
                _shard: shard,
                seqno,
                lt,
                unixtime,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn faucet(&self, address: String, amount: u128) -> anyhow::Result<Value> {
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

    pub async fn get_traces(&self, tx_hash: String) -> anyhow::Result<storage::TraceNode> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetTraces { tx_hash, resp }).await?;
        rx.await?
    }

    pub async fn set_state_source(&self, source: crate::node::StateSource) -> anyhow::Result<()> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::SetStateSource { source, resp })
            .await?;
        rx.await?
    }

    pub async fn get_state_source(&self) -> anyhow::Result<crate::node::StateSource> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetStateSource { resp }).await?;
        rx.await?
    }
}

fn run_node_loop(
    mut rx: mpsc::Receiver<Request>,
    state_source: crate::node::StateSource,
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
            method,
            stack,
            seqno,
            resp,
        } => {
            let res = handle_run_get_method(node, address, method, stack, seqno);
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
            _workchain: _,
            _shard: _,
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
            let res = handle_faucet(node, address, amount);
            let _ = resp.send(res);
        }
        Request::GetTraces { tx_hash, resp } => {
            let res = handle_get_traces(node, tx_hash);
            let _ = resp.send(res);
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

fn handle_faucet(node: &mut Node, addr_str: String, amount: u128) -> anyhow::Result<Value> {
    let addr = parse_addr(&addr_str)?;
    node.faucet(&addr, amount)
}

fn handle_get_traces(node: &Node, tx_hash_hex: String) -> anyhow::Result<storage::TraceNode> {
    let tx_hash = Hash256::from_hex(&tx_hash_hex)?;
    node.get_traces(&tx_hash)
}

pub(crate) fn parse_addr(s: &str) -> anyhow::Result<Addr> {
    let (int_addr, _) = StdAddr::from_str_ext(s, StdAddrFormat::any())
        .map_err(|_| anyhow::anyhow!("Invalid address"))?;
    Ok(Addr {
        workchain: int_addr.workchain as i32,
        addr: int_addr.address.0,
    })
}

fn handle_send_boc(node: &mut Node, boc_str: String) -> anyhow::Result<LiteNodeBlockTransactions> {
    tracing::info!("handle_send_boc: decoding BOC");
    let boc = base64::engine::general_purpose::STANDARD
        .decode(&boc_str)
        .context("Invalid BOC base64")?;
    let (tx_hash, seqno, _) = node.send_boc(boc)?;

    // Fetch full tx info
    if let Some(ext_tx) = node.get_transaction_by_hash(&tx_hash) {
        let tx_boc_b64 = node
            .cas
            .get(&ext_tx.meta.tx_boc_hash)
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .unwrap_or_default();
        let tx_struct = convert_to_tx_struct(&ext_tx, tx_boc_b64)?;

        let block_id = node
            .get_block_header(seqno)
            .as_ref()
            .map(convert_to_block_id_struct)
            .unwrap_or_else(|| LiteNodeBlockId {
                workchain: 0,
                shard: "-9223372036854775808".to_string(),
                seqno,
                root_hash: Hash256([0; 32]),
                file_hash: Hash256([0; 32]),
            });

        Ok(LiteNodeBlockTransactions {
            id: block_id,
            transactions: vec![tx_struct],
        })
    } else {
        anyhow::bail!("Transaction not found after mining")
    }
}

fn convert_to_block_id_struct(h: &storage::BlockMeta) -> LiteNodeBlockId {
    LiteNodeBlockId {
        workchain: 0,
        shard: "-9223372036854775808".to_string(),
        seqno: h.seqno,
        root_hash: h.block_boc_hash,
        file_hash: h.block_boc_hash,
    }
}

fn handle_get_address_info(
    node: &mut Node,
    addr_str: String,
    seqno: Option<u32>,
) -> anyhow::Result<LiteNodeAccountState> {
    let address = parse_addr(&addr_str)?;
    let meta = if let Some(s) = seqno {
        node.get_address_information_at_block(&address, s)
    } else {
        node.get_address_information(&address)
    };

    let query_seqno = seqno.unwrap_or(node.globals.head_seqno);
    let block = node.get_block_header(query_seqno);
    let block_id = block
        .as_ref()
        .map(convert_to_block_id_struct)
        .unwrap_or_else(|| LiteNodeBlockId {
            workchain: 0,
            shard: "-9223372036854775808".to_string(),
            seqno: query_seqno,
            root_hash: Hash256([0; 32]),
            file_hash: Hash256([0; 32]),
        });

    if let Some(m) = meta {
        let code_boc = m
            .code_hash
            .and_then(|h| node.cas.get(&h))
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .unwrap_or_default();
        let data_boc = m
            .data_hash
            .and_then(|h| node.cas.get(&h))
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .unwrap_or_default();

        Ok(LiteNodeAccountState {
            address,
            balance: m.balance_cache.unwrap_or(0),
            code: code_boc,
            data: data_boc,
            last_transaction_id: Some(LiteNodeTransactionId {
                lt: m.last_trans_lt.unwrap_or(0),
                hash: m.last_trans_hash.unwrap_or(Hash256([0; 32])),
            }),
            block_id,
            state: m.status,
            sync_utime: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        })
    } else {
        Ok(LiteNodeAccountState {
            address,
            balance: 0,
            code: "".to_string(),
            data: "".to_string(),
            last_transaction_id: Some(LiteNodeTransactionId {
                lt: 0,
                hash: Hash256([0; 32]),
            }),
            block_id,
            state: AccountStatus::Nonexist,
            sync_utime: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        })
    }
}

fn handle_get_transactions(
    node: &Node,
    addr_str: String,
    limit: u32,
    lt: Option<u64>,
    hash: Option<String>,
    to_lt: Option<u64>,
) -> anyhow::Result<Vec<LiteNodeTransaction>> {
    let addr = parse_addr(&addr_str)?;
    let hash_obj = if let Some(h) = hash {
        Some(Hash256::from_base64(&h)?)
    } else {
        None
    };

    let mut txs = node.get_transactions(&addr, limit as usize, lt, hash_obj);

    if let Some(min_lt) = to_lt {
        txs.retain(|tx| tx.meta.lt >= min_lt);
    }

    let mut result = Vec::new();
    for ext_tx in txs {
        let tx_boc_b64 = node
            .cas
            .get(&ext_tx.meta.tx_boc_hash)
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .unwrap_or_default();
        result.push(convert_to_tx_struct(&ext_tx, tx_boc_b64)?);
    }

    Ok(result)
}

fn handle_run_get_method(
    node: &mut Node,
    addr_str: String,
    method: String,
    stack_json: Vec<Value>,
    seqno: Option<u32>,
) -> anyhow::Result<LiteNodeRunGetMethodResult> {
    let addr = parse_addr(&addr_str)?;
    let meta = if let Some(s) = seqno {
        node.get_address_information_at_block(&addr, s)
    } else {
        node.get_address_information(&addr)
    };

    let meta = meta.ok_or_else(|| anyhow::anyhow!("Account not found"))?;

    let query_seqno = seqno.unwrap_or(node.globals.head_seqno);
    let block = node.get_block_header(query_seqno);
    let block_id = block
        .as_ref()
        .map(convert_to_block_id_struct)
        .unwrap_or_else(|| LiteNodeBlockId {
            workchain: 0,
            shard: "-9223372036854775808".to_string(),
            seqno: query_seqno,
            root_hash: Hash256([0; 32]),
            file_hash: Hash256([0; 32]),
        });

    let last_transaction_id = LiteNodeTransactionId {
        lt: meta.last_trans_lt.unwrap_or(0),
        hash: meta.last_trans_hash.unwrap_or(Hash256([0; 32])),
    };

    let code_boc = meta
        .code_hash
        .and_then(|h| node.cas.get(&h))
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
        .ok_or_else(|| anyhow::anyhow!("Account has no code"))?;

    let data_boc = meta
        .data_hash
        .and_then(|h| node.cas.get(&h))
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
        .ok_or_else(|| anyhow::anyhow!("Account has no data"))?;

    let method_id = if let Ok(id) = method.parse::<i32>() {
        id
    } else {
        let crc = CRC16.checksum(method.as_bytes());
        (crc as i32 & 0xffff) | 0x10000
    };

    let balance_tokens = meta.balance_cache.unwrap_or(0);

    let args = RunGetMethodArgs {
        code: code_boc,
        data: data_boc,
        method_id,
        address: addr_str,
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

    let tuple_stack =
        json_to_legacy_stack(stack_json).context("Failed to parse input legacy stack")?;
    let stack_cell = tuple_stack
        .serialize()
        .context("Failed to serialize stack to BOC")?;
    let stack_b64 = stack_cell
        .to_boc_b64(false)
        .context("Failed to encode stack to Base64")?;

    let exec = GetExecutor::new(&args).context("Failed to create GetExecutor")?;

    let res = exec
        .run_get_method(&stack_b64, &args, None)
        .context("Execution failed")?;

    match res {
        GetMethodResult::Success(s) => Ok(LiteNodeRunGetMethodResult {
            gas_used: s.gas_used.parse().unwrap_or(0),
            stack: s.stack,
            exit_code: s.vm_exit_code,
            vm_log: s.vm_log,
            block_id,
            last_transaction_id,
        }),
        GetMethodResult::Error(e) => anyhow::bail!("Get method error: {:?}", e),
    }
}

pub(crate) fn convert_to_tx_struct(
    tx: &TransactionInfo,
    tx_boc_b64: String,
) -> anyhow::Result<LiteNodeTransaction> {
    let in_msg_struct = if let Some(in_msg) = &tx.in_msg {
        convert_to_message_struct(&in_msg.meta, &in_msg.boc)?
    } else {
        LiteNodeMessage {
            hash: Hash256([0; 32]),
            source: None,
            destination: None,
            value: 0,
            body_hash: "".to_string(),
            body: "".to_string(),
            init_state: "".to_string(),
            opcode: None,
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
        data: tx_boc_b64,
        success: tx.meta.success,
        exit_code: tx.meta.compute_exit_code.unwrap_or(0),
        transaction_id: LiteNodeTransactionId {
            lt: tx.meta.lt,
            hash: tx.meta.tx_hash,
        },
        in_msg: in_msg_struct,
        out_msgs: out_msgs_struct,
    })
}

fn convert_to_message_struct(meta: &MsgMeta, boc: &[u8]) -> anyhow::Result<LiteNodeMessage> {
    let cell = Boc::decode(boc)?;
    let msg = cell.parse::<Message<'_>>()?;

    // Extract body
    let mut builder = tycho_types::cell::CellBuilder::new();
    builder.store_slice(msg.body)?;
    let body_cell = builder.build()?;
    let body_hash = hex::encode(body_cell.repr_hash().as_slice());
    let body_base64 = base64::engine::general_purpose::STANDARD.encode(Boc::encode(body_cell));

    // Extract opcode (first 32 bits)
    let mut opcode = None;
    let mut body_slice = msg.body;
    if body_slice.size_bits() >= 32
        && let Ok(op) = body_slice.load_uint(32)
    {
        opcode = Some(format!("0x{:08x}", op));
    }

    let mut init_state_b64 = String::new();
    if let Some(init) = msg.init {
        let mut builder = tycho_types::cell::CellBuilder::new();
        let _ = init.store_into(&mut builder, tycho_types::cell::Cell::empty_context());
        if let Ok(cell) = builder.build() {
            init_state_b64 = base64::engine::general_purpose::STANDARD.encode(Boc::encode(cell));
        }
    }

    Ok(LiteNodeMessage {
        hash: meta.msg_hash,
        source: meta.src,
        destination: meta.dst,
        value: meta.value.unwrap_or(0),
        body_hash,
        body: body_base64,
        init_state: init_state_b64,
        opcode,
    })
}

fn handle_get_block_header(node: &Node, seqno: u32) -> anyhow::Result<LiteNodeBlockHeader> {
    let header = node.get_block_header(seqno);
    if let Some(h) = header {
        Ok(LiteNodeBlockHeader {
            id: convert_to_block_id_struct(&h),
            gen_utime: h.gen_utime,
            start_lt: h.start_lt,
            end_lt: h.end_lt,
            prev_seqno: h.prev_seqno,
        })
    } else {
        Err(anyhow::anyhow!("Block not found"))
    }
}

fn handle_get_block_transactions(
    node: &Node,
    seqno: u32,
) -> anyhow::Result<LiteNodeBlockTransactions> {
    let txs = node.get_block_transactions(seqno);
    if let Some(txs) = txs {
        let mut result = Vec::new();
        for tx in txs {
            if let Some(ext_tx) = node.get_transaction_by_hash(&tx.tx_hash) {
                let tx_boc_b64 = node
                    .cas
                    .get(&ext_tx.meta.tx_boc_hash)
                    .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
                    .unwrap_or_default();
                result.push(convert_to_tx_struct(&ext_tx, tx_boc_b64)?);
            }
        }

        let block_id = node
            .get_block_header(seqno)
            .as_ref()
            .map(convert_to_block_id_struct)
            .unwrap_or_else(|| LiteNodeBlockId {
                workchain: 0,
                shard: "-9223372036854775808".to_string(),
                seqno,
                root_hash: Hash256([0; 32]),
                file_hash: Hash256([0; 32]),
            });

        Ok(LiteNodeBlockTransactions {
            id: block_id,
            transactions: result,
        })
    } else {
        Err(anyhow::anyhow!("Block not found"))
    }
}

fn handle_get_masterchain_info(node: &Node) -> anyhow::Result<LiteNodeMasterchainInfo> {
    let head_block = node.get_block_header(node.globals.head_seqno);
    let block_id = head_block
        .as_ref()
        .map(convert_to_block_id_struct)
        .unwrap_or_else(|| LiteNodeBlockId {
            workchain: 0,
            shard: "-9223372036854775808".to_string(),
            seqno: 0,
            root_hash: Hash256([0; 32]),
            file_hash: Hash256([0; 32]),
        });

    Ok(LiteNodeMasterchainInfo {
        last: block_id,
        state_root_hash: head_block
            .map(|h| h.block_boc_hash)
            .unwrap_or(Hash256([0; 32])),
        init: LiteNodeBlockId {
            workchain: 0,
            shard: "-9223372036854775808".to_string(),
            seqno: 0,
            root_hash: Hash256([0; 32]),
            file_hash: Hash256([0; 32]),
        },
    })
}

fn handle_get_shards(node: &Node, seqno: u32) -> anyhow::Result<Vec<LiteNodeBlockId>> {
    let block = node.get_block_header(seqno);
    let Some(block_id) = block.as_ref().map(convert_to_block_id_struct) else {
        anyhow::bail!("Block not found for seqno={seqno}")
    };

    Ok(vec![block_id])
}

fn handle_lookup_block(
    node: &Node,
    seqno: Option<u32>,
    lt: Option<u64>,
    unixtime: Option<u32>,
) -> anyhow::Result<LiteNodeBlockId> {
    let block = if let Some(s) = seqno {
        node.get_block_header(s)
    } else if let Some(l) = lt {
        node.find_block_by_lt(l)
    } else if let Some(u) = unixtime {
        node.find_block_by_unixtime(u)
    } else {
        None
    };

    match block {
        Some(b) => Ok(convert_to_block_id_struct(&b)),
        None => anyhow::bail!("Block not found"),
    }
}
