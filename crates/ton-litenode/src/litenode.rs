use anyhow::Context;
use base64::Engine;
use crc::{CRC_16_XMODEM, Crc};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::{mpsc, oneshot};
use ton_emulator::emulator::{Emulator, SendMessageResult};
use ton_emulator::world_state::{AccountsState, LocalAccountsState, WorldState};
use ton_executor::{
    ExecutorVerbosity,
    get::{GetExecutor, GetMethodResult, RunGetMethodArgs},
};
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::json_stack::{json_to_stack, stack_to_json};
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;
use tycho_types::models::{AccountState, IntAddr};

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);

#[derive(Debug)]
pub(crate) enum Request {
    SendBoc {
        boc: String,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    RunGetMethod {
        address: String,
        method: String, // name or id
        stack: Vec<Value>,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetAddressInformation {
        address: String,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetAddressBalance {
        address: String,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetAddressState {
        address: String,
        resp: oneshot::Sender<anyhow::Result<Value>>,
    },
    GetExtendedAddressInformation {
        address: String,
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
}

pub(crate) struct LiteNode {
    tx: mpsc::Sender<Request>,
}

impl LiteNode {
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);

        // spawn the node logic in a blocking task since emulator is single-threaded and CPU bound
        std::thread::spawn(move || {
            if let Err(e) = run_node_loop(rx) {
                tracing::error!("Node loop failed: {:?}", e);
            }
        });

        Self { tx }
    }

    pub(crate) async fn send_boc(&self, boc: String) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::SendBoc { boc, resp })
            .await
            .context("Failed to send request")?;
        rx.await.context("Node loop dropped response channel")?
    }

    pub(crate) async fn run_get_method(
        &self,
        address: String,
        method: String,
        stack: Vec<Value>,
    ) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::RunGetMethod {
                address,
                method,
                stack,
                resp,
            })
            .await
            .context("Failed to send request")?;
        rx.await.context("Node loop dropped response channel")?
    }

    pub(crate) async fn get_address_information(&self, address: String) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressInformation { address, resp })
            .await
            .context("Failed to send request")?;
        rx.await.context("Node loop dropped response channel")?
    }

    pub(crate) async fn get_address_balance(&self, address: String) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressBalance { address, resp })
            .await
            .context("Failed to send request")?;
        rx.await.context("Node loop dropped response channel")?
    }

    pub(crate) async fn get_address_state(&self, address: String) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetAddressState { address, resp })
            .await
            .context("Failed to send request")?;
        rx.await.context("Node loop dropped response channel")?
    }

    pub(crate) async fn get_extended_address_information(
        &self,
        address: String,
    ) -> anyhow::Result<Value> {
        let (resp, rx) = oneshot::channel();
        self.tx
            .send(Request::GetExtendedAddressInformation { address, resp })
            .await
            .context("Failed to send request")?;
        rx.await.context("Node loop dropped response channel")?
    }

    pub(crate) async fn get_transactions(
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
            .await
            .context("Failed to send request")?;
        rx.await.context("Node loop dropped response channel")?
    }
}

fn run_node_loop(mut rx: mpsc::Receiver<Request>) -> anyhow::Result<()> {
    let accounts_state = AccountsState::Local(LocalAccountsState::new());
    let mut world_state =
        WorldState::new(accounts_state, None).context("Failed to create world state")?;

    let emulator =
        Emulator::new(ExecutorVerbosity::Short, None).context("Failed to create emulator")?;

    let mut tx_history: HashMap<String, Vec<Value>> = HashMap::new();

    tracing::info!("LiteNode started");

    while let Some(req) = rx.blocking_recv() {
        match req {
            Request::SendBoc { boc, resp } => {
                let res = handle_send_boc(&emulator, &mut world_state, boc);
                if let Ok(map) = &res
                    && let Some(result) = map.get("result")
                    && let Some(txs) = result.get("transactions").and_then(|v| v.as_array())
                {
                    for tx_summary in txs {
                        if let Some(addr) = tx_summary.get("account").and_then(|v| v.as_str()) {
                            tx_history
                                .entry(addr.to_string())
                                .or_default()
                                .push(tx_summary.clone());
                        }
                    }
                }
                let _ = resp.send(res);
            }
            Request::RunGetMethod {
                address,
                method,
                stack,
                resp,
            } => {
                let res = handle_run_get_method(&mut world_state, address, method, stack);
                let _ = resp.send(res);
            }
            Request::GetAddressInformation { address, resp } => {
                let res = handle_get_address_information(&mut world_state, address);
                let _ = resp.send(res);
            }
            Request::GetAddressBalance { address, resp } => {
                let res = handle_get_address_balance(&mut world_state, address);
                let _ = resp.send(res);
            }
            Request::GetAddressState { address, resp } => {
                let res = handle_get_address_state(&mut world_state, address);
                let _ = resp.send(res);
            }
            Request::GetExtendedAddressInformation { address, resp } => {
                let res = handle_get_extended_address_information(&mut world_state, address);
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
                let res = handle_get_transactions(&tx_history, address, limit, lt, hash, to_lt);
                let _ = resp.send(Ok(res));
            }
        }
    }

    Ok(())
}

fn handle_send_boc(
    emulator: &Emulator,
    state: &mut WorldState,
    boc_str: String,
) -> anyhow::Result<Value> {
    let cell = Boc::decode_base64(&boc_str).context("Invalid BOC")?;
    let libs = tycho_types::dict::Dict::new(); // TODO: Load libs from state if needed

    let results = emulator
        .send_message(state, cell, &libs, None)
        .context("Emulation failed")?;

    let mut tx_summaries = Vec::new();
    for res in results {
        match res {
            SendMessageResult::Success(s) => {
                let tx_cell =
                    Boc::decode_base64(&s.raw_transaction).context("Failed to decode raw tx")?;

                let address = s
                    .shard_account
                    .account
                    .load()
                    .ok()
                    .and_then(|a| a.0)
                    .map(|a| a.address.to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                let tx_id = serde_json::json!({
                    "@type": "internal.transactionId",
                    "lt": s.transaction.lt.to_string(),
                    "hash": hex::encode(tx_cell.repr_hash().as_slice())
                });

                let in_msg_json = if let Some(in_msg) = s.transaction.in_msg.as_ref()
                    && let Ok(msg) = in_msg.parse::<tycho_types::models::Message>()
                {
                    let (src, dst) = match &msg.info {
                        tycho_types::models::MsgInfo::Int(i) => {
                            (i.src.to_string(), i.dst.to_string())
                        }
                        tycho_types::models::MsgInfo::ExtIn(i) => {
                            ("".to_string(), i.dst.to_string())
                        }
                        tycho_types::models::MsgInfo::ExtOut(i) => {
                            (i.src.to_string(), "".to_string())
                        }
                    };

                    let body_cell =
                        CellBuilder::build_from(msg.body).context("Failed to build body cell")?;

                    serde_json::json!({
                        "@type": "raw.message",
                        "source": src,
                        "destination": dst,
                        "value": match &msg.info {
                            tycho_types::models::MsgInfo::Int(i) => u128::from(i.value.tokens).to_string(),
                            _ => "0".to_string(),
                        },
                        "fwd_fee": "0",
                        "ihr_fee": "0",
                        "created_lt": "0",
                        "body_hash": hex::encode(body_cell.repr_hash().as_slice()),
                        "msg_data": {
                            "@type": "msg.dataRaw",
                            "body": Boc::encode_base64(&body_cell),
                            "init_state": ""
                        }
                    })
                } else {
                    Value::Null
                };

                let mut out_msgs = Vec::new();
                for msg_cell in &s.out_messages {
                    if let Ok(msg) = msg_cell.parse::<tycho_types::models::Message>() {
                        let (src, dst) = match &msg.info {
                            tycho_types::models::MsgInfo::Int(i) => {
                                (i.src.to_string(), i.dst.to_string())
                            }
                            tycho_types::models::MsgInfo::ExtIn(i) => {
                                ("".to_string(), i.dst.to_string())
                            }
                            tycho_types::models::MsgInfo::ExtOut(i) => {
                                (i.src.to_string(), "".to_string())
                            }
                        };

                        let body_cell = CellBuilder::build_from(msg.body)
                            .context("Failed to build body cell")?;

                        out_msgs.push(serde_json::json!({
                            "@type": "raw.message",
                            "source": src,
                            "destination": dst,
                            "value": match &msg.info {
                                tycho_types::models::MsgInfo::Int(i) => u128::from(i.value.tokens).to_string(),
                                _ => "0".to_string(),
                            },
                            "fwd_fee": "0",
                            "ihr_fee": "0",
                            "created_lt": "0",
                            "body_hash": hex::encode(body_cell.repr_hash().as_slice()),
                            "msg_data": {
                                "@type": "msg.dataRaw",
                                "body": Boc::encode_base64(&body_cell),
                                "init_state": ""
                            }
                        }));
                    }
                }

                tx_summaries.push(serde_json::json!({
                    "@type": "ext.transaction",
                    "address": {
                        "@type": "accountAddress",
                        "account_address": address
                    },
                    "account": address,
                    "utime": s.transaction.now,
                    "data": s.raw_transaction,
                    "transaction_id": tx_id,
                    "fee": u128::from(s.transaction.total_fees.tokens).to_string(),
                    "storage_fee": "0",
                    "other_fee": "0",
                    "in_msg": in_msg_json,
                    "out_msgs": out_msgs
                }));
            }
            SendMessageResult::Error(e) => {
                tx_summaries.push(serde_json::json!({
                    "success": false,
                    "error": format!("{:?}", e),
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "ok": true,
        "result": {
            "transactions": tx_summaries
        }
    }))
}

fn handle_get_transactions(
    tx_history: &HashMap<String, Vec<Value>>,
    address: String,
    limit: u32,
    lt: Option<u64>,
    hash: Option<String>,
    to_lt: Option<u64>,
) -> Value {
    let address = normalize_address(&address);
    let history = tx_history.get(&address);
    let result = match history {
        Some(txs) => {
            let mut filtered = txs.clone();
            filtered.reverse(); // Newest first

            let hash = hash.map(|h| {
                hex::encode(
                    base64::engine::general_purpose::STANDARD
                        .decode(h)
                        .expect("not lucky"),
                )
            });
            let start_idx = if let Some(start_lt) = lt {
                filtered
                    .iter()
                    .position(|tx| {
                        let id = tx.get("transaction_id");
                        let tx_lt = id
                            .and_then(|id| id.get("lt"))
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<u64>().ok());

                        if let Some(tx_lt) = tx_lt {
                            if let Some(start_hash) = &hash {
                                // if tx_lt == start_lt {
                                    let tx_hash =
                                        id.and_then(|id| id.get("hash")).and_then(|v| v.as_str());
                                    return tx_hash == Some(start_hash);
                                // }
                                // return tx_lt < start_lt;
                            } else {
                                return tx_lt <= start_lt;
                            }
                        }
                        false
                    })
                    .unwrap_or(filtered.len())
            } else {
                0
            };

            let mut final_txs = if start_idx < filtered.len() {
                filtered.split_off(start_idx)
            } else {
                vec![]
            };

            if let Some(min_lt) = to_lt {
                final_txs.retain(|tx| {
                    let tx_lt = tx
                        .get("transaction_id")
                        .and_then(|id| id.get("lt"))
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u64>().ok());
                    tx_lt.map(|lt| lt >= min_lt).unwrap_or(false)
                });
            }

            final_txs.truncate(limit as usize);
            final_txs
        }
        None => vec![],
    };

    serde_json::json!({
        "ok": true,
        "result": result
    })
}

fn normalize_address(address: &str) -> String {
    let address = IntAddr::from_str(address).unwrap_or_else(|_| IntAddr::default());

    address.to_string()
}

fn handle_run_get_method(
    state: &mut WorldState,
    address: String,
    method: String,
    stack_json: Vec<Value>,
) -> anyhow::Result<Value> {
    let address = normalize_address(&address);
    let account = state.get_account(&address);
    let loaded_account = account.account.load().context("Failed to load account")?;

    let (code, data) = match &loaded_account.0 {
        Some(acc) => match &acc.state {
            AccountState::Active(s) => {
                let code = s.code.as_ref().context("Account has no code")?;
                let data = s.data.as_ref().context("Account has no data")?;
                (Boc::encode_base64(code), Boc::encode_base64(data))
            }
            _ => anyhow::bail!("Account is not active"),
        },
        None => anyhow::bail!("Account not found"),
    };

    let method_id = if let Ok(id) = method.parse::<i32>() {
        id
    } else {
        let crc = CRC16.checksum(method.as_bytes());
        (crc as i32 & 0xffff) | 0x10000
    };

    // 3. Prepare args
    let balance_tokens = loaded_account
        .0
        .as_ref()
        .map(|a| u128::from(a.balance.tokens))
        .unwrap_or(0);

    let args = RunGetMethodArgs {
        code,
        data,
        method_id,
        address: address.clone(),
        unixtime: state.get_now() as i64,
        balance: balance_tokens.to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        gas_limit: "10000000".to_owned(),
        debug_enabled: false,
        verbosity: ExecutorVerbosity::Short,
        libs: String::new(), // TODO: serialize libs
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
                    "gas_used": s.gas_used,
                    "stack": result_stack_json,
                    "exit_code": s.vm_exit_code,
                    "vm_log": s.vm_log,
                }
            }))
        }
        GetMethodResult::Error(e) => anyhow::bail!("Get method error: {:?}", e),
    }
}

struct InternalAccountInfo {
    balance: String,
    code: String,
    data: String,
    last_trans_lt: String,
    last_trans_hash: String,
    frozen_hash: String,
    state: String,
}

fn get_account_info_internal(
    state: &mut WorldState,
    address: &str,
) -> anyhow::Result<InternalAccountInfo> {
    let address = normalize_address(address);
    let account = state.get_account(&address);
    let loaded_account = account.account.load().context("Failed to load account")?;

    let mut info = InternalAccountInfo {
        balance: "0".to_string(),
        code: String::new(),
        data: String::new(),
        last_trans_lt: account.last_trans_lt.to_string(),
        last_trans_hash: hex::encode(account.last_trans_hash.as_slice()),
        frozen_hash: String::new(),
        state: "uninitialized".to_string(),
    };

    if let Some(acc) = loaded_account.0 {
        info.balance = u128::from(acc.balance.tokens).to_string();
        match acc.state {
            AccountState::Active(s) => {
                info.state = "active".to_string();
                if let Some(code) = s.code {
                    info.code = Boc::encode_base64(code);
                }
                if let Some(data) = s.data {
                    info.data = Boc::encode_base64(data);
                }
            }
            AccountState::Frozen(hash) => {
                info.state = "frozen".to_string();
                info.frozen_hash = hex::encode(hash.as_slice());
            }
            AccountState::Uninit => {
                info.state = "uninitialized".to_string();
            }
        }
    }

    Ok(info)
}

fn handle_get_address_information(
    state: &mut WorldState,
    address: String,
) -> anyhow::Result<Value> {
    let info = get_account_info_internal(state, &address)?;

    Ok(serde_json::json!({
        "ok": true,
        "result": {
            "@type": "raw.fullAccountState",
            "balance": info.balance,
            "code": info.code,
            "data": info.data,
            "last_transaction_id": {
                "@type": "internal.transactionId",
                "lt": info.last_trans_lt,
                "hash": info.last_trans_hash
            },
            "block_id": {
                "@type": "ton.blockIdExt",
                "workchain": 0,
                "shard": -9223372036854775808i64,
                "seqno": 0,
                "root_hash": "",
                "file_hash": ""
            },
            "frozen_hash": info.frozen_hash,
            "extra_currencies": [],
            "sync_utime": state.get_now() as i64,
            "state": info.state
        }
    }))
}

fn handle_get_address_balance(state: &mut WorldState, address: String) -> anyhow::Result<Value> {
    let info = get_account_info_internal(state, &address)?;
    Ok(serde_json::json!({
        "ok": true,
        "result": info.balance
    }))
}

fn handle_get_address_state(state: &mut WorldState, address: String) -> anyhow::Result<Value> {
    let info = get_account_info_internal(state, &address)?;
    Ok(serde_json::json!({
        "ok": true,
        "result": info.state
    }))
}

fn handle_get_extended_address_information(
    state: &mut WorldState,
    address: String,
) -> anyhow::Result<Value> {
    let info = get_account_info_internal(state, &address)?;

    let account_state = if info.state == "uninitialized" {
        serde_json::json!({
            "@type": "uninited.accountState"
        })
    } else {
        serde_json::json!({
            "@type": "raw.accountState",
            "code": info.code,
            "data": info.data,
            "frozen_hash": info.frozen_hash
        })
    };

    Ok(serde_json::json!({
        "ok": true,
        "result": {
            "@type": "fullAccountState",
            "address": {
                "@type": "accountAddress",
                "account_address": address
            },
            "balance": info.balance,
            "extra_currencies": [],
            "last_transaction_id": {
                "@type": "internal.transactionId",
                "lt": info.last_trans_lt,
                "hash": info.last_trans_hash
            },
            "block_id": {
                "@type": "ton.blockIdExt",
                "workchain": 0,
                "shard": -9223372036854775808i64,
                "seqno": 0,
                "root_hash": "",
                "file_hash": ""
            },
            "sync_utime": state.get_now() as i64,
            "account_state": account_state,
            "revision": 0
        }
    }))
}
