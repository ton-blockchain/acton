use anyhow::Context;
use base64::Engine;
use crc::{CRC_16_XMODEM, Crc};
use serde_json::Value;
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
use tycho_types::models::AccountState;

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
}

fn run_node_loop(mut rx: mpsc::Receiver<Request>) -> anyhow::Result<()> {
    let accounts_state = AccountsState::Local(LocalAccountsState::new());
    let mut world_state =
        WorldState::new(accounts_state, None).context("Failed to create world state")?;

    let emulator =
        Emulator::new(ExecutorVerbosity::Short, None).context("Failed to create emulator")?;

    tracing::info!("LiteNode started");

    while let Some(req) = rx.blocking_recv() {
        match req {
            Request::SendBoc { boc, resp } => {
                let res = handle_send_boc(&emulator, &mut world_state, boc);
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

                tx_summaries.push(serde_json::json!({
                    "success": true,
                    "lt": s.transaction.lt,
                    "hash": tx_cell.repr_hash().to_string(),
                    "vm_log": s.vm_log,
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

fn handle_run_get_method(
    state: &mut WorldState,
    address: String,
    method: String,
    stack_json: Vec<Value>,
) -> anyhow::Result<Value> {
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

fn handle_get_address_information(
    state: &mut WorldState,
    address: String,
) -> anyhow::Result<Value> {
    let account = state.get_account(&address);
    let loaded_account = account.account.load().context("Failed to load account")?;

    let mut result = serde_json::json!({
        "@type": "raw.fullAccountState",
        "balance": "0",
        "code": "",
        "data": "",
        "last_transaction_id": {
            "@type": "internal.transactionId",
            "lt": account.last_trans_lt.to_string(),
            "hash": base64::engine::general_purpose::STANDARD.encode(account.last_trans_hash.as_slice())
        },
        "block_id": {
            "@type": "ton.blockIdExt",
            "workchain": 0,
            "shard": "-9223372036854775808", // 0x8000000000000000
            "seqno": 0,
            "root_hash": "",
            "file_hash": ""
        },
        "frozen_hash": "",
        "extra_currencies": [],
        "sync_utime": state.get_now() as i64, // TODO: what is it?
        "state": "uninitialized",
        "suspended": false, // TODO
    });

    if let Some(acc) = loaded_account.0 {
        result["balance"] = serde_json::json!(u128::from(acc.balance.tokens).to_string());

        match acc.state {
            AccountState::Active(s) => {
                result["state"] = serde_json::json!("active");
                if let Some(code) = s.code {
                    result["code"] = serde_json::json!(Boc::encode_base64(code));
                }
                if let Some(data) = s.data {
                    result["data"] = serde_json::json!(Boc::encode_base64(data));
                }
            }
            AccountState::Frozen(hash) => {
                result["state"] = serde_json::json!("frozen");
                result["frozen_hash"] = serde_json::json!(hex::encode(hash.as_slice()));
            }
            AccountState::Uninit => {
                result["state"] = serde_json::json!("uninitialized");
            }
        }
    }

    Ok(serde_json::json!({
        "ok": true,
        "result": result
    }))
}
