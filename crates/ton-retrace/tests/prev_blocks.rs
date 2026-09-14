use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use expect_test::expect;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use ton_api::toncenter::v3;
use ton_executor::message::{
    EmulationResult, Executor, PrevBlockId, PrevBlocksInfo, RunTransactionArgs,
};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use ton_retrace::{BaseTxInfo, CustomNetworkUrls, Network, retrace_base_tx};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Lazy};
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntMsgInfo, MsgInfo, OptionalAccount, OwnedMessage,
    ShardAccount, SpecialFlags, StateInit, StdAddr, Transaction,
};

const NOW: u32 = 1_780_000_000;

// Persist every field of every history entry. Native reference transactions
// therefore expose wrong order, hashes, list boundaries and key-block selection.
const CONTRACT: &str = r#"
fun blockInfo(): tuple asm "PREVBLOCKSINFOTUPLE"
fun executeCode(code: slice): tuple asm "BLESS" "CALLX"

fun saveInfo(info: tuple) {
    var result = createEmptyCell();
    var i = 0;
    repeat (info.size()) {
        val group = info.get(i) as tuple;
        i += 1;
        var j = 0;
        repeat (group.size()) {
            val block = group.get(j) as tuple;
            j += 1;
            result = beginCell()
                .storeInt(block.get(0) as int, 32)
                .storeUint(block.get(1) as int, 64)
                .storeUint(block.get(2) as int, 32)
                .storeUint(block.get(3) as int, 256)
                .storeUint(block.get(4) as int, 256)
                .storeRef(result).endCell();
        }
        result = beginCell().storeUint(group.size(), 8).storeRef(result).endCell();
    }
    val old = contract.getData();
    contract.setData(beginCell().storeRef(old).storeRef(result).endCell());
}

fun saveBlocks(info: tuple) {
    val recent = info.get(0) as tuple;
    val key = info.get(1) as tuple;
    val hundreds = info.get(2) as tuple;
    var keys: tuple = [];
    keys.push(key);
    var groups: tuple = [];
    groups.push(recent);
    groups.push(keys);
    groups.push(hundreds);
    saveInfo(groups);
}

fun onInternalMessage(in: InMessage) {
    saveBlocks(blockInfo());
}

fun onRunTickTock(_: bool) {
    saveBlocks(blockInfo());
}
"#;

fn block_id(seqno: u32) -> PrevBlockId {
    let mut root_hash = [0x11; 32];
    let mut file_hash = [0x22; 32];
    root_hash[..4].copy_from_slice(&seqno.to_be_bytes());
    file_hash[..4].copy_from_slice(&seqno.to_be_bytes());
    PrevBlockId {
        workchain: -1,
        shard: i64::MIN,
        seqno,
        root_hash,
        file_hash,
    }
}

fn api_block_id(seqno: u32) -> Value {
    let id = block_id(seqno);
    json!({
        "@type": "ton.blockIdExt", "workchain": -1,
        "shard": if seqno == 0 { "0" } else { "-9223372036854775808" },
        "seqno": seqno, "root_hash": STANDARD.encode(id.root_hash),
        "file_hash": STANDARD.encode(id.file_hash),
    })
}

fn transaction_response(cell: &Cell, workchain: i8) -> anyhow::Result<Value> {
    let tx: Transaction = cell.parse()?;
    Ok(json!({
        "@type": "raw.transaction",
        "address": { "@type": "accountAddress", "account_address": StdAddr::new(workchain, tx.account).to_string() },
        "account": StdAddr::new(workchain, tx.account).to_string(),
        "utime": tx.now, "data": Boc::encode_base64(cell),
        "transaction_id": { "@type": "internal.transactionId", "lt": tx.lt.to_string(), "hash": STANDARD.encode(cell.repr_hash().as_slice()) },
        "fee": "0", "storage_fee": "0", "other_fee": "0", "out_msgs": [],
    }))
}

struct Fixture {
    base_tx: BaseTxInfo,
    account: ShardAccount,
    transactions: Vec<Value>,
    anchor: u32,
    key: u32,
    separate_blocks: bool,
    config: String,
}

impl Fixture {
    fn new(anchor: u32, key: u32, kind: &str) -> anyhow::Result<Self> {
        let mut source = CONTRACT.to_owned();
        if kind == "dynamic" || kind == "recent_dynamic" {
            source = source.replace(
                "saveBlocks(blockInfo());",
                if kind == "dynamic" {
                    "saveBlocks(executeCode(in.body.loadRef().beginParse()));"
                } else {
                    "saveBlocks(blockInfo()); saveBlocks(executeCode(in.body.loadRef().beginParse()));"
                },
            );
            source.truncate(
                source
                    .find("fun onRunTickTock")
                    .expect("tick-tock entrypoint in fixture"),
            );
        }
        if kind == "recent" || kind == "recent_dynamic" {
            source = source.replace(
                "asm \"PREVBLOCKSINFOTUPLE\"",
                "asm \"PREVMCBLOCKS\" \"PREVKEYBLOCK\" \"NIL\" \"3 TUPLE\"",
            );
        } else if kind == "hundreds" {
            source = source.replace(
                "asm \"PREVBLOCKSINFOTUPLE\"",
                "asm \"PREVMCBLOCKS\" \"PREVKEYBLOCK\" \"PREVMCBLOCKS_100\" \"3 TUPLE\"",
            );
        } else if kind == "unused" {
            source = source.replace(
                "saveBlocks(blockInfo());",
                "contract.setData(beginCell().storeRef(contract.getData()).endCell());",
            );
        }
        if kind == "legacy" {
            source = source.replace("saveBlocks(blockInfo());", "var info = blockInfo(); assert(info.size() == 2, 77); info.push([] as tuple); saveBlocks(info);");
        }
        let mut config = tycho_types::models::BlockchainConfigParams::from_raw(Boc::decode_base64(
            DEFAULT_CONFIG,
        )?);
        if kind == "legacy" {
            let mut version = config.get_global_version()?;
            version.version = 8;
            config.set_global_version(&version)?;
        }
        let config = Boc::encode_base64(
            config
                .as_dict()
                .root()
                .as_ref()
                .context("non-empty config")?,
        );
        let path = Path::new("/retrace-tests/prev-blocks.tolk");
        let compiler =
            tolk_compiler::Compiler::new(2).with_source_overrides([(path, source.as_str())]);
        let compiled = match compiler.compile(path, false) {
            tolk_compiler::CompilerResult::Success(compiled) => compiled,
            other @ tolk_compiler::CompilerResult::Error(_) => {
                anyhow::bail!("Cannot compile history fixture: {other:?}")
            }
        };
        // Tick-tock avoids the modern INMSG_BOUNCED dispatch emitted by Tolk,
        // allowing the legacy case to test c7 using instructions available in TVM 8.
        let tick_tock = kind == "tick" || kind == "legacy";
        let workchain = if tick_tock { -1 } else { 0 };
        let address = StdAddr::new(workchain, HashBytes([0x33; 32]));
        let account = ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(Account {
                address: address.clone().into(),
                storage_stat: Default::default(),
                last_trans_lt: 0,
                balance: CurrencyCollection::new(10_000_000_000),
                state: AccountState::Active(StateInit {
                    code: Some(Boc::decode_base64(compiled.code_boc64)?),
                    data: Some(Cell::empty_cell()),
                    special: tick_tock.then_some(SpecialFlags {
                        tick: true,
                        tock: true,
                    }),
                    ..Default::default()
                }),
            })))?,
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        };
        let history = PrevBlocksInfo::new(
            (0..16)
                .filter_map(|offset| anchor.checked_sub(offset))
                .map(block_id)
                .collect(),
            block_id(key),
            (kind != "legacy").then(|| {
                (0..16)
                    .filter_map(|offset| (anchor / 100).checked_sub(offset))
                    .map(|value| block_id(value * 100))
                    .collect()
            }),
        );
        let body = CellBuilder::build_from(CellBuilder::build_from(0xf82du16)?)?;
        let message = Boc::encode_base64(CellBuilder::build_from(OwnedMessage {
            info: MsgInfo::Int(IntMsgInfo {
                src: StdAddr::new(workchain, HashBytes([0x44; 32])).into(),
                dst: address.clone().into(),
                value: CurrencyCollection::new(1_000_000_000),
                ..Default::default()
            }),
            init: None,
            body: body.into(),
            layout: None,
        })?);
        let executor = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, Some(&config))?;
        let mut state = Boc::encode_base64(CellBuilder::build_from(&account)?);
        let mut cells = Vec::new();
        // Both the predecessor and the target read c7. Replay must restart both
        // after a dynamic-code mismatch rather than reusing the wrong state.
        for lt in [1_000_000, 2_000_000] {
            let earlier_block = kind == "separate_blocks" && lt == 1_000_000;
            let mut history = history.clone();
            if earlier_block {
                history.last_mc_blocks = (0..16)
                    .map(|offset| block_id(anchor - 1 - offset))
                    .collect();
            }
            let (result, _) = executor.run_transaction(
                &message,
                &RunTransactionArgs {
                    shard_account: state,
                    now: NOW,
                    lt,
                    random_seed: Some([if earlier_block { 0x41 } else { 0x42 }; 32]),
                    prev_blocks_info: Some(history.clone()),
                    is_tick_tock: tick_tock.then_some(true),
                    is_tock: tick_tock.then_some(false),
                    ..Default::default()
                },
            )?;
            let EmulationResult::Success(result) = result else {
                anyhow::bail!("Native fixture failed: {result:?}");
            };
            state = result.shard_account.to_string();
            let cell = Boc::decode_base64(result.transaction.as_ref())?;
            let tx: Transaction = cell.parse()?;
            let tycho_types::models::ComputePhase::Executed(compute) = (match tx.load_info()? {
                tycho_types::models::TxInfo::Ordinary(info) => info.compute_phase,
                tycho_types::models::TxInfo::TickTock(info) => info.compute_phase,
            }) else {
                anyhow::bail!("Reference compute skipped");
            };
            anyhow::ensure!(
                compute.success,
                "Reference compute failed: {}\n{}",
                compute.exit_code,
                result
                    .vm_log
                    .lines()
                    .rev()
                    .take(12)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            cells.push(cell);
        }
        let target = cells.last().expect("two reference transactions");
        Ok(Self {
            base_tx: BaseTxInfo {
                address,
                hash: target.repr_hash().0,
                lt: 2_000_000,
                block: v3::BlockId {
                    workchain: i32::from(workchain),
                    shard: "8000000000000000".to_owned(),
                    seqno: anchor + 1,
                },
            },
            account,
            transactions: cells
                .iter()
                .rev()
                .map(|cell| transaction_response(cell, workchain))
                .collect::<anyhow::Result<_>>()?,
            anchor,
            key,
            separate_blocks: kind == "separate_blocks",
            config,
        })
    }

    fn response(&self, path: &str, corrupt: bool) -> Value {
        let url =
            reqwest::Url::parse(&format!("http://localhost{path}")).expect("valid request URL");
        let seqno = url
            .query_pairs()
            .find(|(name, _)| name == "seqno")
            .map_or(0, |(_, value)| {
                value.parse::<u32>().expect("numeric block seqno")
            });
        let result = match url.path() {
            "/v3/transactions" => {
                let tx = &self.transactions[1];
                return json!({"transactions": [{
                    "account": tx["account"], "hash": tx["transaction_id"]["hash"],
                    "lt": tx["transaction_id"]["lt"], "now": NOW,
                    "block_ref": {"workchain": self.base_tx.block.workchain, "shard": "8000000000000000", "seqno": self.base_tx.block.seqno - u32::from(self.separate_blocks)},
                    "mc_block_seqno": self.anchor + 1, "emulated": false, "finality": "finalized",
                    "prev_trans_hash": STANDARD.encode([0; 32]), "prev_trans_lt": "0",
                    "orig_status": "active", "end_status": "active", "total_fees": "0",
                    "description": {"type": "ord"},
                    "account_state_before": {"hash": STANDARD.encode([0; 32])},
                    "account_state_after": {"hash": STANDARD.encode([0; 32])},
                }]});
            }
            "/v3/blocks" => {
                let earlier = self.separate_blocks && seqno < self.base_tx.block.seqno;
                return json!({ "blocks": [{
                "workchain": self.base_tx.block.workchain, "shard": "8000000000000000", "seqno": seqno,
                "root_hash": STANDARD.encode([0; 32]), "file_hash": STANDARD.encode([0; 32]),
                "start_lt": "1", "end_lt": "3000000", "gen_utime": NOW.to_string(),
                "masterchain_block_ref": {"workchain": -1, "shard": "8000000000000000", "seqno": self.anchor + 1},
                "after_merge": false, "after_split": false, "before_split": false, "created_by": "", "flags": 0,
                "gen_catchain_seqno": 0, "global_id": -239, "key_block": false, "master_ref_seqno": self.anchor - u32::from(earlier),
                "min_ref_mc_seqno": 0, "prev_key_block_seqno": self.key, "rand_seed": STANDARD.encode([if earlier { 0x41 } else { 0x42 }; 32]),
                "tx_count": 2, "validator_list_hash_short": 0, "version": 0, "vert_seqno": 0,
                "vert_seqno_incr": false, "want_merge": false, "want_split": false,
            }] });
            }
            "/v2/getConfigAll" => {
                json!({"@type": "configInfo", "config": {"@type": "tvm.cell", "bytes": self.config}})
            }
            "/v2/getShardAccountCell" => {
                json!({"@type": "tvm.cell", "bytes": Boc::encode_base64(CellBuilder::build_from(&self.account).expect("valid fixture account"))})
            }
            "/v2/getTransactions" => json!(self.transactions),
            "/v2/getBlockHeader" => {
                let mut id = api_block_id(seqno);
                if corrupt {
                    id["root_hash"] = json!(STANDARD.encode([0; 31]));
                }
                json!({
                    "@type": "blocks.header", "id": id, "global_id": -239, "version": 0,
                    "after_merge": false, "after_split": false, "before_split": false,
                    "want_merge": false, "want_split": false, "validator_list_hash_short": 0,
                    "catchain_seqno": 0, "min_ref_mc_seqno": 0, "is_key_block": seqno == self.key,
                    "prev_key_block_seqno": if seqno == self.key { 0 } else { self.key },
                    "start_lt": "0", "end_lt": "0", "gen_utime": NOW, "prev_blocks": [],
                })
            }
            "/v2/lookupBlock" => api_block_id(seqno),
            "/v2/getMasterchainInfo" => {
                json!({"@type": "blocks.masterchainInfo", "last": api_block_id(self.anchor), "state_root_hash": "", "init": api_block_id(0)})
            }
            _ => panic!("Unexpected request: {path}"),
        };
        json!({"ok": true, "@extra": "", "result": result})
    }
}

async fn replay(fixture: Fixture, corrupt: bool) -> anyhow::Result<(String, Vec<String>)> {
    let base_tx = fixture.base_tx.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.expect("mock connection");
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).await.expect("mock request");
                assert_ne!(count, 0);
                request.extend_from_slice(&buffer[..count]);
            }
            let request = String::from_utf8(request).expect("UTF-8 request");
            let path = request.split_whitespace().nth(1).expect("request path");
            captured
                .lock()
                .expect("request capture lock")
                .push(path.to_owned());
            let body = fixture.response(path, corrupt).to_string();
            stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.expect("mock response");
        }
    });
    let networks = HashMap::from([(
        "fixture".to_owned(),
        CustomNetworkUrls {
            v2_url: format!("{origin}/v2").into(),
            v3_url: Some(format!("{origin}/v3").into()),
            explorer_url: None,
        },
    )]);
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        retrace_base_tx(
            Network::Custom("fixture".into()),
            base_tx,
            HashMap::new(),
            &networks,
        ),
    )
    .await;
    server.abort();
    let outcome = match result.context("Retrace fixture timed out")? {
        Ok(result) => format!("state_matches={}", result.state_update_hash_ok),
        Err(error) => format!("{error:#}"),
    };
    let paths = requests
        .lock()
        .expect("request capture lock")
        .iter()
        .filter(|path| {
            path.contains("getBlockHeader")
                || path.contains("lookupBlock")
                || path.contains("getMasterchainInfo")
        })
        .cloned()
        .collect();
    Ok((outcome, paths))
}

#[tokio::test]
async fn replays_previous_and_target_transactions_with_block_history() -> anyhow::Result<()> {
    let mut results = String::new();
    for (kind, anchor, key) in [
        ("full", 2345, 2300),
        ("recent", 2345, 2300),
        ("dynamic", 2345, 2300),
        ("recent_dynamic", 2345, 2300),
        ("hundreds", 2345, 2300),
        ("tick", 200, 200),
        ("full", 5, 0),
        ("unused", 2345, 2300),
    ] {
        let (outcome, paths) = replay(Fixture::new(anchor, key, kind)?, false).await?;
        let mut requests: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
        for path in &paths {
            let url = reqwest::Url::parse(&format!("http://localhost{path}"))?;
            let seqno = url
                .query_pairs()
                .find(|(name, _)| name == "seqno")
                .map(|(_, value)| value.parse::<u32>())
                .transpose()?
                .unwrap_or(0);
            let method = path
                .split('?')
                .next()
                .context("request method")?
                .trim_start_matches("/v2/");
            requests.entry(method).or_default().push(seqno);
        }
        writeln!(results, "{kind} anchor={anchor}: {outcome}")?;
        for (method, seqnos) in requests {
            writeln!(results, "  {method}: {seqnos:?}")?;
        }
    }
    expect![[r"
        full anchor=2345: state_matches=true
          getBlockHeader: [2345]
          lookupBlock: [2300, 2344, 2343, 2342, 2341, 2340, 2339, 2338, 2337, 2336, 2335, 2334, 2333, 2332, 2331, 2330, 2200, 2100, 2000, 1900, 1800, 1700, 1600, 1500, 1400, 1300, 1200, 1100, 1000, 900, 800]
        recent anchor=2345: state_matches=true
          getBlockHeader: [2345]
          lookupBlock: [2300, 2344, 2343, 2342, 2341, 2340, 2339, 2338, 2337, 2336, 2335, 2334, 2333, 2332, 2331, 2330]
        dynamic anchor=2345: state_matches=true
          getBlockHeader: [2345]
          lookupBlock: [2300, 2344, 2343, 2342, 2341, 2340, 2339, 2338, 2337, 2336, 2335, 2334, 2333, 2332, 2331, 2330, 2200, 2100, 2000, 1900, 1800, 1700, 1600, 1500, 1400, 1300, 1200, 1100, 1000, 900, 800]
        recent_dynamic anchor=2345: state_matches=true
          getBlockHeader: [2345, 2345]
          lookupBlock: [2300, 2344, 2343, 2342, 2341, 2340, 2339, 2338, 2337, 2336, 2335, 2334, 2333, 2332, 2331, 2330, 2300, 2344, 2343, 2342, 2341, 2340, 2339, 2338, 2337, 2336, 2335, 2334, 2333, 2332, 2331, 2330, 2200, 2100, 2000, 1900, 1800, 1700, 1600, 1500, 1400, 1300, 1200, 1100, 1000, 900, 800]
        hundreds anchor=2345: state_matches=true
          getBlockHeader: [2345]
          lookupBlock: [2300, 2344, 2343, 2342, 2341, 2340, 2339, 2338, 2337, 2336, 2335, 2334, 2333, 2332, 2331, 2330, 2200, 2100, 2000, 1900, 1800, 1700, 1600, 1500, 1400, 1300, 1200, 1100, 1000, 900, 800]
        tick anchor=200: state_matches=true
          getBlockHeader: [200]
          getMasterchainInfo: [0]
          lookupBlock: [199, 198, 197, 196, 195, 194, 193, 192, 191, 190, 189, 188, 187, 186, 185, 100]
        full anchor=5: state_matches=true
          getBlockHeader: [5]
          getMasterchainInfo: [0]
          lookupBlock: [4, 3, 2, 1]
        unused anchor=2345: state_matches=true
    "]].assert_eq(&results);
    Ok(())
}

#[tokio::test]
async fn rejects_invalid_block_hashes_before_replay() -> anyhow::Result<()> {
    let (outcome, _) = replay(Fixture::new(2345, 2300, "full")?, true).await?;
    expect!["Failed to load previous blocks at masterchain block 2345: Invalid masterchain root hash: Invalid block hash length: expected 32 bytes, got 31"].assert_eq(&outcome);
    Ok(())
}

#[tokio::test]
async fn replays_predecessor_with_its_own_masterchain_reference() -> anyhow::Result<()> {
    let (outcome, _) = replay(Fixture::new(2345, 2300, "separate_blocks")?, false).await?;
    expect!["state_matches=true"].assert_eq(&outcome);
    Ok(())
}

#[tokio::test]
async fn preserves_pre_tvm9_c7_tuple_layout() -> anyhow::Result<()> {
    let (outcome, paths) = replay(Fixture::new(2345, 2300, "legacy")?, false).await?;
    expect![[r#"
        (
            "state_matches=true",
            17,
        )
    "#]]
    .assert_debug_eq(&(outcome, paths.len()));
    Ok(())
}
