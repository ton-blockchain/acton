use crate::executor::TvmEmulatorAdapter;
use crate::node::Node;
use crate::storage::{MsgMeta, TxMeta};
use crate::types::{Addr, BocBytes, Hash256};
use anyhow::Context;
use base64::Engine;
use crc::{CRC_16_XMODEM, Crc};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use ton_executor::DEFAULT_CONFIG;
use ton_executor::ExecutorVerbosity;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::json_stack::{
    json_to_legacy_stack, json_to_stack, legacy_stack_to_json, stack_to_json,
};
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::{CellFamily, Store};
use tycho_types::models::{Message, StdAddr, StdAddrFormat};

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);

#[derive(Debug)]
pub(crate) enum Request {
    SendBoc {
        boc: String,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetAddressInformation {
        address: String,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetTransactions {
        address: String,
        limit: u32,
        lt: Option<u64>,
        hash: Option<String>,
        to_lt: Option<u64>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    // Optional/Legacy
    RunGetMethod {
        address: String,
        method: String,
        stack: Vec<Value>,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    RunGetMethodStd {
        address: String,
        method: String,
        stack: Vec<Value>,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetAddressBalance {
        address: String,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetAddressState {
        address: String,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetExtendedAddressInformation {
        address: String,
        seqno: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetBlockHeader {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetBlockTransactionsExt {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetMasterchainInfo {
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetShards {
        seqno: u32,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    LookupBlock {
        _workchain: i32,
        _shard: String,
        seqno: Option<u32>,
        lt: Option<u64>,
        unixtime: Option<u32>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    Faucet {
        address: String,
        amount: u128,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
}

pub struct LiteNode {
    tx: mpsc::Sender<Request>,
}

impl Default for LiteNode {
    fn default() -> Self {
        Self::new()
    }
}

impl LiteNode {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);

        std::thread::spawn(move || {
            if let Err(e) = run_node_loop(rx) {
                tracing::error!("Node loop failed: {:?}", e);
            }
        });

        Self { tx }
    }

    pub async fn send_boc(&self, boc: String) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::SendBoc { boc, resp }).await?;
        rx.await?
    }

    pub async fn get_address_information(
        &self,
        address: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<Value> {
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
    ) -> anyhow::Result<Value> {
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
    ) -> anyhow::Result<Value> {
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

    pub async fn run_get_method_std(
        &self,
        address: String,
        method: String,
        stack: Vec<Value>,
        seqno: Option<u32>,
    ) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::RunGetMethodStd {
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
    ) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressBalance {
                address,
                seqno,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_address_state(
        &self,
        address: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressState {
                address,
                seqno,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_extended_address_information(
        &self,
        address: String,
        seqno: Option<u32>,
    ) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetExtendedAddressInformation {
                address,
                seqno,
                resp,
            })
            .await?;
        rx.await?
    }

    pub async fn get_block_header(&self, seqno: u32) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetBlockHeader { seqno, resp })
            .await?;
        rx.await?
    }

    pub async fn get_block_transactions_ext(&self, seqno: u32) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetBlockTransactionsExt { seqno, resp })
            .await?;
        rx.await?
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx.send(Request::GetMasterchainInfo { resp }).await?;
        rx.await?
    }

    pub async fn get_shards(&self, seqno: u32) -> anyhow::Result<Value> {
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
    ) -> anyhow::Result<Value> {
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
}

fn run_node_loop(mut rx: mpsc::Receiver<Request>) -> anyhow::Result<()> {
    let executor = Box::new(TvmEmulatorAdapter::new()?);
    let config_bytes = base64::engine::general_purpose::STANDARD.decode(DEFAULT_CONFIG)?;
    let mut node = Node::new(executor, config_bytes)?;

    tracing::info!("LiteNode started (new architecture)");

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
        Request::RunGetMethodStd {
            address,
            method,
            stack,
            seqno,
            resp,
        } => {
            let res = handle_run_get_method_std(node, address, method, stack, seqno);
            let _ = resp.send(res);
        }
        Request::GetAddressBalance {
            address,
            seqno,
            resp,
        } => {
            let res = handle_get_address_balance(node, address, seqno);
            let _ = resp.send(res);
        }
        Request::GetAddressState {
            address,
            seqno,
            resp,
        } => {
            let res = handle_get_address_state(node, address, seqno);
            let _ = resp.send(res);
        }
        Request::GetExtendedAddressInformation {
            address,
            seqno,
            resp,
        } => {
            let res = handle_get_extended_address_info(node, address, seqno);
            let _ = resp.send(res);
        }
        Request::GetBlockHeader { seqno, resp } => {
            let res = handle_get_block_header(node, seqno);
            let _ = resp.send(res);
        }
        Request::GetBlockTransactionsExt { seqno, resp } => {
            let res = handle_get_block_transactions_ext(node, seqno);
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
            let res = crate::node::handle_faucet(node, address, amount);
            let _ = resp.send(res);
        }
    }
}

pub(crate) fn parse_addr(s: &str) -> anyhow::Result<Addr> {
    let (int_addr, _) = StdAddr::from_str_ext(s, StdAddrFormat::any())
        .map_err(|_| anyhow::anyhow!("Invalid address"))?;
    Ok(Addr {
        workchain: int_addr.workchain as i32,
        addr: int_addr.address.0,
    })
}

fn handle_send_boc(node: &mut Node, boc_str: String) -> anyhow::Result<Value> {
    tracing::info!("handle_send_boc: decoding BOC");
    let boc = base64::engine::general_purpose::STANDARD
        .decode(&boc_str)
        .context("Invalid BOC base64")?;
    let (tx_hash, seqno, _) = node.send_boc(boc)?;

    // Fetch full tx info
    if let Some((tx, in_msg, out_msgs)) = node.get_transaction_by_hash_extended(&tx_hash) {
        let tx_boc_b64 = node
            .cas
            .get(&tx.tx_boc_hash)
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .unwrap_or_default();
        let tx_json = convert_to_tx_json(&tx, in_msg.as_ref(), &out_msgs, tx_boc_b64)?;

        let block_id_json = node
            .get_block_header(seqno)
            .as_ref()
            .map(convert_to_block_id_json)
            .unwrap_or_else(|| {
                serde_json::json!({
                    "@type": "ton.blockIdExt",
                    "workchain": 0,
                    "shard": "-9223372036854775808",
                    "seqno": seqno,
                    "root_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                    "file_hash": "0000000000000000000000000000000000000000000000000000000000000000"
                })
            });

        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "blocks.transactionsExt",
                "id": block_id_json,
                "req_count": 1,
                "incomplete": false,
                "transactions": [tx_json]
            }
        }))
    } else {
        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "tx_hash": tx_hash.to_hex(),
                "block_seqno": seqno
            }
        }))
    }
}

fn convert_to_block_id_json(h: &crate::storage::BlockMeta) -> Value {
    serde_json::json!({
        "@type": "ton.blockIdExt",
        "workchain": 0,
        "shard": "-9223372036854775808",
        "seqno": h.seqno,
        "root_hash": h.block_boc_hash.to_hex(),
        "file_hash": h.block_boc_hash.to_hex()
    })
}

fn handle_get_address_info(
    node: &Node,
    addr_str: String,
    seqno: Option<u32>,
) -> anyhow::Result<Value> {
    let addr = parse_addr(&addr_str)?;
    let meta = if let Some(s) = seqno {
        node.get_address_information_at_block(&addr, s)
    } else {
        node.get_address_information(&addr)
    };

    let query_seqno = seqno.unwrap_or(node.globals.head_seqno);
    let block = node.get_block_header(query_seqno);
    let block_id_json = block
        .as_ref()
        .map(convert_to_block_id_json)
        .unwrap_or_else(|| {
            serde_json::json!({
                "@type": "ton.blockIdExt",
                "workchain": 0,
                "shard": "-9223372036854775808",
                "seqno": query_seqno,
                "root_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                "file_hash": "0000000000000000000000000000000000000000000000000000000000000000"
            })
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

        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "raw.fullAccountState",
                "balance": m.balance_cache.unwrap_or(0).to_string(),
                "code": code_boc,
                "data": data_boc,
                "last_transaction_id": {
                    "@type": "internal.transactionId",
                    "lt": m.last_trans_lt.unwrap_or(0).to_string(),
                    "hash": m.last_trans_hash.map(|h| h.to_hex()).unwrap_or_default()
                },
                "block_id": block_id_json,
                "frozen_hash": "",
                "extra_currencies": [],
                "sync_utime": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                "state": format!("{:?}", m.status).to_lowercase()
            }
        }))
    } else {
        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "raw.fullAccountState",
                "balance": "0",
                "code": "",
                "data": "",
                "last_transaction_id": {
                     "@type": "internal.transactionId",
                     "lt": "0",
                     "hash": "0000000000000000000000000000000000000000000000000000000000000000"
                },
                "block_id": block_id_json,
                "frozen_hash": "",
                "extra_currencies": [],
                "sync_utime": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                "state": "nonexist"
            }
        }))
    }
}

fn handle_get_transactions(
    node: &Node,
    addr_str: String,
    limit: u32,
    lt: Option<u64>,
    hash: Option<String>,
    to_lt: Option<u64>,
) -> anyhow::Result<Value> {
    tracing::info!(
        "handle_get_transactions: addr={}, limit={}, lt={:?}, to_lt={:?}",
        addr_str,
        limit,
        lt,
        to_lt
    );
    let addr = parse_addr(&addr_str)?;
    let hash_obj = if let Some(h) = hash {
        Some(Hash256::from_base64(&h)?)
    } else {
        None
    };

    let mut txs = node.get_transactions_extended(&addr, limit as usize, lt, hash_obj);

    if let Some(min_lt) = to_lt {
        txs.retain(|(tx, _, _)| tx.lt >= min_lt);
    }

    let mut result_json = Vec::new();
    for (tx, in_msg, out_msgs) in txs {
        let tx_boc_b64 = node
            .cas
            .get(&tx.tx_boc_hash)
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .unwrap_or_default();
        result_json.push(convert_to_tx_json(
            &tx,
            in_msg.as_ref(),
            &out_msgs,
            tx_boc_b64,
        )?);
    }

    Ok(serde_json::json!({
        "ok": true,
        "result": result_json
    }))
}

fn handle_run_get_method(
    node: &Node,
    addr_str: String,
    method: String,
    stack_json: Vec<Value>,
    seqno: Option<u32>,
) -> anyhow::Result<Value> {
    tracing::info!(
        "handle_run_get_method: addr={}, method={}, seqno={:?}",
        addr_str,
        method,
        seqno
    );
    let addr = match parse_addr(&addr_str) {
        Ok(a) => a,
        Err(_) => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Invalid address",
                "code": 400
            }));
        }
    };
    let meta = if let Some(s) = seqno {
        node.get_address_information_at_block(&addr, s)
    } else {
        node.get_address_information(&addr)
    };

    let meta = match meta {
        Some(m) => m,
        None => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Account not found",
                "code": 404
            }));
        }
    };

    let query_seqno = seqno.unwrap_or(node.globals.head_seqno);
    let block = node.get_block_header(query_seqno);
    let block_id_json = block
        .as_ref()
        .map(convert_to_block_id_json)
        .unwrap_or_else(|| {
            serde_json::json!({
                "@type": "ton.blockIdExt",
                "workchain": 0,
                "shard": "-9223372036854775808",
                "seqno": query_seqno,
                "root_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                "file_hash": "0000000000000000000000000000000000000000000000000000000000000000"
            })
        });

    let last_transaction_id = serde_json::json!({
        "@type": "internal.transactionId",
        "lt": meta.last_trans_lt.unwrap_or(0).to_string(),
        "hash": meta.last_trans_hash.map(|h| h.to_hex()).unwrap_or_default()
    });

    let code_boc = match meta
        .code_hash
        .and_then(|h| node.cas.get(&h))
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
    {
        Some(c) => c,
        None => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Account has no code",
                "code": 404
            }));
        }
    };

    let data_boc = match meta
        .data_hash
        .and_then(|h| node.cas.get(&h))
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
    {
        Some(d) => d,
        None => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Account has no data",
                "code": 404
            }));
        }
    };

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
        GetMethodResult::Success(s) => {
            let stack_cell =
                ArcCell::from_boc_b64(&s.stack).context("Failed to decode result stack BOC")?;
            let stack_tuple =
                Tuple::deserialize(&stack_cell).context("Failed to deserialize result stack")?;
            let result_stack_json = legacy_stack_to_json(&stack_tuple)
                .context("Failed to convert result legacy stack to JSON")?;

            Ok(serde_json::json!({
                "ok": true,
                "result": {
                    "@type": "smc.runResult",
                    "gas_used": s.gas_used,
                    "stack": result_stack_json,
                    "exit_code": s.vm_exit_code,
                    "vm_log": s.vm_log,
                    "block_id": block_id_json,
                    "last_transaction_id": last_transaction_id,
                }
            }))
        }
        GetMethodResult::Error(e) => Ok(serde_json::json!({
            "ok": false,
            "error": format!("Get method error: {:?}", e),
            "code": 500
        })),
    }
}

fn handle_run_get_method_std(
    node: &Node,
    addr_str: String,
    method: String,
    stack_json: Vec<Value>,
    seqno: Option<u32>,
) -> anyhow::Result<Value> {
    tracing::info!(
        "handle_run_get_method_std: addr={}, method={}, seqno={:?}",
        addr_str,
        method,
        seqno
    );
    let addr = match parse_addr(&addr_str) {
        Ok(a) => a,
        Err(_) => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Invalid address",
                "code": 400
            }));
        }
    };
    let meta = if let Some(s) = seqno {
        node.get_address_information_at_block(&addr, s)
    } else {
        node.get_address_information(&addr)
    };

    let meta = match meta {
        Some(m) => m,
        None => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Account not found",
                "code": 404
            }));
        }
    };

    let query_seqno = seqno.unwrap_or(node.globals.head_seqno);
    let block = node.get_block_header(query_seqno);
    let block_id_json = block
        .as_ref()
        .map(convert_to_block_id_json)
        .unwrap_or_else(|| {
            serde_json::json!({
                "@type": "ton.blockIdExt",
                "workchain": 0,
                "shard": "-9223372036854775808",
                "seqno": query_seqno,
                "root_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                "file_hash": "0000000000000000000000000000000000000000000000000000000000000000"
            })
        });

    let last_transaction_id = serde_json::json!({
        "@type": "internal.transactionId",
        "lt": meta.last_trans_lt.unwrap_or(0).to_string(),
        "hash": meta.last_trans_hash.map(|h| h.to_hex()).unwrap_or_default()
    });

    let code_boc = match meta
        .code_hash
        .and_then(|h| node.cas.get(&h))
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
    {
        Some(c) => c,
        None => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Account has no code",
                "code": 404
            }));
        }
    };

    let data_boc = match meta
        .data_hash
        .and_then(|h| node.cas.get(&h))
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
    {
        Some(d) => d,
        None => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Account has no data",
                "code": 404
            }));
        }
    };

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

    let tuple_stack = json_to_stack(stack_json).context("Failed to parse input stack")?;
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
        GetMethodResult::Success(s) => {
            let stack_cell =
                ArcCell::from_boc_b64(&s.stack).context("Failed to decode result stack BOC")?;
            let stack_tuple =
                Tuple::deserialize(&stack_cell).context("Failed to deserialize result stack")?;
            let result_stack_json =
                stack_to_json(&stack_tuple).context("Failed to convert result stack to JSON")?;

            Ok(serde_json::json!({
                "ok": true,
                "result": {
                    "@type": "smc.runResult",
                    "gas_used": s.gas_used,
                    "stack": result_stack_json,
                    "exit_code": s.vm_exit_code,
                    "vm_log": s.vm_log,
                    "block_id": block_id_json,
                    "last_transaction_id": last_transaction_id,
                }
            }))
        }
        GetMethodResult::Error(e) => Ok(serde_json::json!({
            "ok": false,
            "error": format!("Get method error: {:?}", e),
            "code": 500
        })),
    }
}

fn convert_to_tx_json(
    tx: &TxMeta,
    in_msg: Option<&(MsgMeta, BocBytes)>,
    out_msgs: &Vec<(MsgMeta, BocBytes)>,
    tx_boc_b64: String,
) -> anyhow::Result<Value> {
    let in_msg_json = if let Some((meta, boc)) = in_msg {
        convert_to_message_json(meta, boc)?
    } else {
        serde_json::json!({ "@type": "msg.message" })
    };

    let mut out_msgs_json = Vec::new();
    for (meta, boc) in out_msgs {
        out_msgs_json.push(convert_to_message_json(meta, boc)?);
    }

    Ok(serde_json::json!({
        "@type": "ext.transaction",
        "address": { "@type": "accountAddress", "account_address": tx.account.to_string() },
        "account": tx.account.to_string(),
        "utime": tx.now,
        "data": tx_boc_b64,
        "transaction_id": {
            "@type": "internal.transactionId",
            "lt": tx.lt.to_string(),
            "hash": tx.tx_hash.to_hex()
        },
        "fee": "0",
        "storage_fee": "0",
        "other_fee": "0",
        "in_msg": in_msg_json,
        "out_msgs": out_msgs_json
    }))
}

fn convert_to_message_json(meta: &MsgMeta, boc: &[u8]) -> anyhow::Result<Value> {
    let cell = Boc::decode(boc)?;
    let msg = cell.parse::<Message<'_>>()?;

    // Extract body
    let mut builder = tycho_types::cell::CellBuilder::new();
    builder.store_slice(msg.body)?;
    let body_cell = builder.build()?;
    let body_hash = hex::encode(body_cell.repr_hash().as_slice());
    let body_base64 = base64::engine::general_purpose::STANDARD.encode(Boc::encode(body_cell));

    let mut init_state_b64 = String::new();
    if let Some(init) = msg.init {
        let mut builder = tycho_types::cell::CellBuilder::new();
        let _ = init.store_into(&mut builder, tycho_types::cell::Cell::empty_context());
        if let Ok(cell) = builder.build() {
            init_state_b64 = base64::engine::general_purpose::STANDARD.encode(Boc::encode(cell));
        }
    }

    Ok(serde_json::json!({
        "@type": "raw.message",
        "hash": meta.msg_hash.to_hex(),
        "source": {
            "@type": "accountAddress",
            "account_address": meta.src.as_ref().map(|a| a.to_string()).unwrap_or_default()
        },
        "destination": {
            "@type": "accountAddress",
            "account_address": meta.dst.as_ref().map(|a| a.to_string()).unwrap_or_default()
        },
        "value": meta.value.unwrap_or(0).to_string(),
        "fwd_fee": "0",
        "ihr_fee": "0",
        "created_lt": "0",
        "body_hash": body_hash,
        "msg_data": {
            "@type": "msg.dataRaw",
            "body": body_base64,
            "init_state": init_state_b64
        },
        "extra_currencies": []
    }))
}

fn handle_get_address_balance(
    node: &Node,
    addr_str: String,
    seqno: Option<u32>,
) -> anyhow::Result<Value> {
    let addr = parse_addr(&addr_str)?;
    let meta = if let Some(s) = seqno {
        node.get_address_information_at_block(&addr, s)
    } else {
        node.get_address_information(&addr)
    };
    let balance = meta.and_then(|m| m.balance_cache).unwrap_or(0);

    Ok(serde_json::json!({
        "ok": true,
        "result": balance.to_string()
    }))
}

fn handle_get_address_state(
    node: &Node,
    addr_str: String,
    seqno: Option<u32>,
) -> anyhow::Result<Value> {
    let addr = parse_addr(&addr_str)?;
    let meta = if let Some(s) = seqno {
        node.get_address_information_at_block(&addr, s)
    } else {
        node.get_address_information(&addr)
    };
    let state = meta
        .map(|m| format!("{:?}", m.status).to_lowercase())
        .unwrap_or_else(|| "nonexist".to_string());

    Ok(serde_json::json!({
        "ok": true,
        "result": state
    }))
}

fn handle_get_extended_address_info(
    node: &Node,
    addr_str: String,
    seqno: Option<u32>,
) -> anyhow::Result<Value> {
    let addr = parse_addr(&addr_str)?;
    let meta = if let Some(s) = seqno {
        node.get_address_information_at_block(&addr, s)
    } else {
        node.get_address_information(&addr)
    };

    let query_seqno = seqno.unwrap_or(node.globals.head_seqno);
    let block = node.get_block_header(query_seqno);
    let block_id_json = block
        .as_ref()
        .map(convert_to_block_id_json)
        .unwrap_or_else(|| {
            serde_json::json!({
                "@type": "ton.blockIdExt",
                "workchain": 0,
                "shard": "-9223372036854775808",
                "seqno": query_seqno,
                "root_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                "file_hash": "0000000000000000000000000000000000000000000000000000000000000000"
            })
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

        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "fullAccountState",
                "address": { "@type": "accountAddress", "account_address": addr_str },
                "balance": m.balance_cache.unwrap_or(0).to_string(),
                "last_transaction_id": {
                    "@type": "internal.transactionId",
                    "lt": m.last_trans_lt.unwrap_or(0).to_string(),
                    "hash": m.last_trans_hash.map(|h| h.to_hex()).unwrap_or_default()
                },
                "block_id": block_id_json,
                "sync_utime": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                "account_state": {
                    "@type": "raw.accountState",
                    "code": code_boc,
                    "data": data_boc,
                    "frozen_hash": ""
                },
                "revision": 0
            }
        }))
    } else {
        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "fullAccountState",
                "address": { "@type": "accountAddress", "account_address": addr_str },
                "balance": "0",
                "last_transaction_id": {
                     "@type": "internal.transactionId",
                     "lt": "0",
                     "hash": "0000000000000000000000000000000000000000000000000000000000000000"
                },
                "block_id": block_id_json,
                "account_state": {
                    "@type": "uninited.accountState",
                    "frozen_hash": ""
                },
                "sync_utime": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                "revision": 0
            }
        }))
    }
}

fn handle_get_block_header(node: &Node, seqno: u32) -> anyhow::Result<Value> {
    let header = node.get_block_header(seqno);
    if let Some(h) = header {
        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "ton.blockHeader",
                "id": convert_to_block_id_json(&h),
                "gen_utime": h.gen_utime,
                "start_lt": h.start_lt.to_string(),
                "end_lt": h.end_lt.to_string(),
                "prev_seqno": h.prev_seqno
            }
        }))
    } else {
        Err(anyhow::anyhow!("Block not found"))
    }
}

fn handle_get_block_transactions_ext(node: &Node, seqno: u32) -> anyhow::Result<Value> {
    let txs = node.get_block_transactions_ext(seqno);
    if let Some(txs) = txs {
        let mut result_json = Vec::new();
        for tx in txs {
            if let Some((tx_ext, in_msg, out_msgs)) =
                node.get_transaction_by_hash_extended(&tx.tx_hash)
            {
                let tx_boc_b64 = node
                    .cas
                    .get(&tx_ext.tx_boc_hash)
                    .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
                    .unwrap_or_default();
                result_json.push(convert_to_tx_json(
                    &tx_ext,
                    in_msg.as_ref(),
                    &out_msgs,
                    tx_boc_b64,
                )?);
            }
        }

        let block_id_json = node
            .get_block_header(seqno)
            .as_ref()
            .map(convert_to_block_id_json)
            .unwrap_or_else(|| {
                serde_json::json!({
                    "@type": "ton.blockIdExt",
                    "workchain": 0,
                    "shard": "-9223372036854775808",
                    "seqno": seqno,
                    "root_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                    "file_hash": "0000000000000000000000000000000000000000000000000000000000000000"
                })
            });

        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "blocks.transactionsExt",
                "id": block_id_json,
                "req_count": result_json.len(),
                "incomplete": false,
                "transactions": result_json
            }
        }))
    } else {
        Err(anyhow::anyhow!("Block not found"))
    }
}

fn handle_get_masterchain_info(node: &Node) -> anyhow::Result<Value> {
    let head_block = node.get_block_header(node.globals.head_seqno);
    let block_id_json = head_block
        .as_ref()
        .map(convert_to_block_id_json)
        .unwrap_or_else(|| {
            serde_json::json!({
                "@type": "ton.blockIdExt",
                "workchain": 0,
                "shard": "-9223372036854775808",
                "seqno": 0,
                "root_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                "file_hash": "0000000000000000000000000000000000000000000000000000000000000000"
            })
        });

    Ok(serde_json::json!({
        "ok": true,
        "result": {
            "@type": "blocks.masterchainInfo",
            "last": block_id_json,
            "state_root_hash": head_block.map(|h| h.block_boc_hash.to_hex()).unwrap_or_else(|| "0000000000000000000000000000000000000000000000000000000000000000".to_string()),
            "init": {
                "@type": "ton.blockIdExt",
                "workchain": 0,
                "shard": "-9223372036854775808",
                "seqno": 0,
                "root_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                "file_hash": "0000000000000000000000000000000000000000000000000000000000000000"
            }
        }
    }))
}

fn handle_get_shards(node: &Node, seqno: u32) -> anyhow::Result<Value> {
    let block = node.get_block_header(seqno);
    let Some(block_id_json) = block.as_ref().map(convert_to_block_id_json) else {
        anyhow::bail!("Block not found for seqno={seqno}")
    };

    Ok(serde_json::json!({
        "ok": true,
        "result": {
            "@type": "blocks.shards",
            "shards": [block_id_json]
        }
    }))
}

fn handle_lookup_block(
    node: &Node,
    seqno: Option<u32>,
    lt: Option<u64>,
    unixtime: Option<u32>,
) -> anyhow::Result<Value> {
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
        Some(b) => Ok(serde_json::json!({
            "ok": true,
            "result": convert_to_block_id_json(&b)
        })),
        None => Ok(serde_json::json!({
            "ok": false,
            "error": "Block not found",
            "code": 404
        })),
    }
}
