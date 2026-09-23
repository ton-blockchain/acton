use super::*;
use crate::BaseTxInfo;
use crate::methods::find_all_transactions_between;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use expect_test::expect;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use toncenter::v3;
use tycho_types::cell::{CellBuilder, CellFamily, HashBytes, Lazy};
use tycho_types::models::{
    AccountStatus, ComputePhase, ComputePhaseSkipReason, HashUpdate, OrdinaryTxInfo,
    SkippedComputePhase, StdAddr, Transaction, TxInfo,
};
use tycho_types::{boc::Boc, prelude::Cell};

async fn serve_response(status: &str, response: Value) -> (Client, JoinHandle<String>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let body = response.to_string();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let count = stream.read(&mut buffer).await.unwrap();
            assert_ne!(count, 0, "request closed before headers");
            request.extend_from_slice(&buffer[..count]);
        }
        stream.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8(request).unwrap()
    });
    let client = Client::builder()
        .system_proxy(false)
        .request_timeout(std::time::Duration::from_secs(5))
        .api_key(Some("fixture-key".to_owned()))
        .v2_url(format!("{base_url}/api/v2"))
        .v3_url(format!("{base_url}/api/v3"))
        .build()
        .unwrap();
    (client, server, base_url)
}

fn transaction_cell(lt: u64, previous: Option<&Cell>) -> anyhow::Result<Cell> {
    let previous_tx = previous
        .map(|cell| cell.parse::<Transaction>())
        .transpose()?;
    Ok(CellBuilder::build_from(Transaction {
        account: HashBytes([0x11; 32]),
        lt,
        prev_trans_lt: previous_tx.map_or(0, |tx| tx.lt),
        prev_trans_hash: previous.map_or(HashBytes::ZERO, |cell| *cell.repr_hash()),
        now: 1,
        out_msg_count: Default::default(),
        orig_status: AccountStatus::Uninit,
        end_status: AccountStatus::Uninit,
        in_msg: None,
        out_msgs: Default::default(),
        total_fees: Default::default(),
        state_update: Lazy::new(&HashUpdate {
            old: HashBytes::ZERO,
            new: HashBytes::ZERO,
        })?,
        info: Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
            credit_first: false,
            storage_phase: None,
            credit_phase: None,
            compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                reason: ComputePhaseSkipReason::NoState,
            }),
            action_phase: None,
            aborted: true,
            bounce_phase: None,
            destroyed: false,
        }))?,
    })?)
}

#[tokio::test]
async fn prev_blocks_at_zerostate_use_the_network_initial_id() -> anyhow::Result<()> {
    let zero = json!({
        "@type": "ton.blockIdExt", "workchain": -1, "shard": "0", "seqno": 0,
        "root_hash": STANDARD.encode([0x11; 32]), "file_hash": STANDARD.encode([0x22; 32]),
    });
    let (client, server, _) = serve_response("200 OK", json!({
        "ok": true, "@extra": "", "result": {
            "@type": "blocks.masterchainInfo", "last": zero, "init": zero, "state_root_hash": "",
        },
    })).await;
    let info = get_prev_blocks_info(&client, 0, true).await?;
    expect![[r#"
        (
            "GET /api/v2/getMasterchainInfo HTTP/1.1",
            1,
            1,
            0,
            -9223372036854775808,
            true,
            true,
        )
    "#]]
    .assert_debug_eq(&(
        server.await?.lines().next().unwrap(),
        info.last_mc_blocks.len(),
        info.last_mc_blocks_100.as_ref().unwrap().len(),
        info.prev_key_block.seqno,
        info.prev_key_block.shard,
        info.last_mc_blocks == vec![info.prev_key_block.clone()],
        info.last_mc_blocks_100 == Some(vec![info.prev_key_block]),
    ));
    Ok(())
}

fn transaction_response(cell: &Cell) -> Value {
    let tx: Transaction = cell.parse().unwrap();
    let account = StdAddr::new(0, tx.account).to_string();
    json!({
        "@type": "ext.transaction",
        "address": { "@type": "accountAddress", "account_address": account },
        "account": account,
        "utime": tx.now,
        "data": Boc::encode_base64(cell),
        "transaction_id": { "@type": "internal.transactionId", "lt": tx.lt.to_string(), "hash": STANDARD.encode(cell.repr_hash().as_slice()) },
        "fee": "0", "storage_fee": "0", "other_fee": "0", "out_msgs": []
    })
}

#[tokio::test]
async fn localnet_and_custom_network_use_independent_api_urls() -> anyhow::Result<()> {
    let mut results = Vec::new();
    for network in [Network::Localnet, Network::Custom("archive".into())] {
        let cell = Cell::empty_cell();
        let (_, v2_server, v2_url) = serve_response("200 OK", json!({
            "ok": true, "@extra": "", "result": {"@type": "tvm.cell", "bytes": Boc::encode_base64(&cell)}
        })).await;
        let (_, v3_server, v3_url) =
            serve_response("200 OK", json!({"transactions": [], "address_book": {}})).await;
        let networks = HashMap::from([(
            network.as_str(),
            CustomNetworkUrls {
                v2_url: format!("{v2_url}/rpc/archive/").into(),
                v3_url: Some(format!("{v3_url}/indexer/").into()),
                explorer_url: None,
            },
        )]);
        let client = client(network.clone(), &networks)?;

        let account: v2::stack::TvmCell = client
            .v2_request(
                V2Transport::Get,
                "getShardAccountCell",
                &[("address", "0:11"), ("seqno", "42")],
            )
            .await?;
        let account = Boc::decode_base64(account.bytes)?;
        let transactions = client
            .v3_get::<v3::TransactionsResponse>("transactions", &[("hash", "fixture".to_owned())])
            .await?;
        let v2_request = v2_server.await?;
        let v3_request = v3_server.await?;
        results.push((
            network.as_str(),
            v2_request.lines().next().unwrap().to_owned(),
            v3_request.lines().next().unwrap().to_owned(),
            account == cell,
            transactions.transactions.len(),
        ));
    }
    expect![[r#"
        [
            (
                "localnet",
                "GET /rpc/archive/getShardAccountCell?address=0%3A11&seqno=42 HTTP/1.1",
                "GET /indexer/transactions?hash=fixture HTTP/1.1",
                true,
                0,
            ),
            (
                "archive",
                "GET /rpc/archive/getShardAccountCell?address=0%3A11&seqno=42 HTTP/1.1",
                "GET /indexer/transactions?hash=fixture HTTP/1.1",
                true,
                0,
            ),
        ]
    "#]]
    .assert_debug_eq(&results);
    Ok(())
}

#[test]
fn custom_network_requires_configured_v2_and_v3() {
    let missing = client(Network::Custom("archive".into()), &HashMap::new())
        .expect_err("missing network should fail");
    let networks = HashMap::from([(
        "archive".to_owned(),
        CustomNetworkUrls {
            v2_url: "http://localhost/rpc".into(),
            v3_url: None,
            explorer_url: None,
        },
    )]);
    let missing_v3 = client(Network::Custom("archive".into()), &networks)
        .expect_err("missing v3 URL should fail");
    expect![[r#"
        (
            "unknown custom network: archive",
            "v3_url not configured for custom network: archive",
        )
    "#]]
    .assert_debug_eq(&(missing.to_string(), missing_v3.to_string()));
}

#[tokio::test]
async fn archive_history_preserves_order_and_excludes_snapshot_transaction() -> anyhow::Result<()> {
    let snapshot = transaction_cell(10, None)?;
    let predecessor = transaction_cell(20, Some(&snapshot))?;
    let target = transaction_cell(30, Some(&predecessor))?;
    let base_tx = BaseTxInfo {
        lt: 30,
        hash: target.repr_hash().0,
        address: StdAddr::new(0, HashBytes([0x11; 32])),
        block: v3::BlockId {
            workchain: 0,
            shard: "8000000000000000".to_owned(),
            seqno: 1,
        },
    };
    let (client, server, _) = serve_response("200 OK", json!({
        "ok": true, "@extra": "", "result": [transaction_response(&target), transaction_response(&predecessor)]
    })).await;

    let transactions = find_all_transactions_between(&client, &base_tx, 10).await?;
    let request = server.await?;
    let uri = request.split_whitespace().nth(1).unwrap();
    let url = reqwest::Url::parse(&format!("http://localhost{uri}"))?;
    let query = url
        .query_pairs()
        .into_owned()
        .collect::<std::collections::BTreeMap<_, _>>();
    expect![[r#"
        (
            "/api/v2/getTransactions",
            Some(
                "10",
            ),
            Some(
                "true",
            ),
            [
                30,
                20,
            ],
        )
    "#]]
    .assert_debug_eq(&(
        url.path(),
        query.get("to_lt"),
        query.get("archival"),
        transactions.iter().map(|tx| tx.lt).collect::<Vec<_>>(),
    ));

    // The first-block approximation uses an arbitrary LT cutoff just before the
    // target because a true zerostate snapshot is unavailable.
    let (client, server, _) = serve_response(
        "200 OK",
        json!({
            "ok": true, "@extra": "", "result": [transaction_response(&target)]
        }),
    )
    .await;
    let transactions = find_all_transactions_between(&client, &base_tx, 29).await?;
    server.await?;
    expect![[r"
        [
            30,
        ]
    "]]
    .assert_debug_eq(&transactions.iter().map(|tx| tx.lt).collect::<Vec<_>>());
    Ok(())
}

#[tokio::test]
async fn archive_history_rejects_incomplete_or_inconsistent_responses() -> anyhow::Result<()> {
    let first = transaction_cell(10, None)?;
    let second = transaction_cell(20, Some(&first))?;
    let target = transaction_cell(30, Some(&second))?;
    let base_tx = BaseTxInfo {
        lt: 30,
        hash: target.repr_hash().0,
        address: StdAddr::new(0, HashBytes([0x11; 32])),
        block: v3::BlockId {
            workchain: 0,
            shard: "8000000000000000".to_owned(),
            seqno: 1,
        },
    };
    let wrong_hash = transaction_cell(20, None)?;
    let mut malformed = transaction_response(&target);
    malformed["data"] = json!(Boc::encode_base64(Cell::empty_cell()));
    let mut results = Vec::new();
    for (name, rows) in [
        ("empty", vec![]),
        ("short page", vec![transaction_response(&target)]),
        (
            "gap",
            vec![transaction_response(&target), transaction_response(&first)],
        ),
        (
            "duplicate",
            vec![transaction_response(&target), transaction_response(&target)],
        ),
        (
            "wrong hash",
            vec![
                transaction_response(&target),
                transaction_response(&wrong_hash),
            ],
        ),
        (
            "snapshot included",
            vec![
                transaction_response(&target),
                transaction_response(&second),
                transaction_response(&first),
            ],
        ),
        ("malformed BOC", vec![malformed]),
    ] {
        let (client, server, _) = serve_response(
            "200 OK",
            json!({ "ok": true, "@extra": "", "result": rows }),
        )
        .await;
        let result = find_all_transactions_between(&client, &base_tx, 10).await;
        server.await?;
        results.push((name, result.unwrap_err().to_string()));
    }
    expect![[r#"
        [
            (
                "empty",
                "Incomplete TON Center history: missing transaction at LT 30",
            ),
            (
                "short page",
                "Incomplete TON Center history: missing transaction at LT 20",
            ),
            (
                "gap",
                "TON Center history does not match expected transaction at LT 20",
            ),
            (
                "duplicate",
                "TON Center history does not match expected transaction at LT 20",
            ),
            (
                "wrong hash",
                "TON Center history does not match expected transaction at LT 20",
            ),
            (
                "snapshot included",
                "TON Center history contains a transaction outside the requested account or LT range",
            ),
            (
                "malformed BOC",
                "Failed to parse TON Center transaction at LT 30",
            ),
        ]
    "#]].assert_debug_eq(&results);
    Ok(())
}

#[tokio::test]
async fn cell_requests_preserve_api_errors_and_reject_missing_results() -> anyhow::Result<()> {
    let mut results = Vec::new();
    for (status, response) in [
        (
            "200 OK",
            json!({"ok": false, "error": "archive unavailable", "code": 500}),
        ),
        (
            "500 Internal Server Error",
            json!({"ok": false, "error": "archive unavailable", "code": 500}),
        ),
        ("200 OK", json!({"ok": true, "@extra": ""})),
        (
            "200 OK",
            json!({"ok": true, "@extra": "", "result": {"@type": "tvm.cell", "bytes": "invalid"}}),
        ),
    ] {
        let (client, server, _) = serve_response(status, response).await;
        let error = async {
            let cell: v2::stack::TvmCell = client
                .v2_request(
                    V2Transport::Get,
                    "getShardAccountCell",
                    &[("address", "0:11"), ("seqno", "1")],
                )
                .await?;
            Boc::decode_base64(cell.bytes).context("Failed to decode shard account cell BOC data")
        }
        .await
        .unwrap_err();
        server.await?;
        results.push(format!("{error:#}"));
    }
    expect![[r#"
        [
            "TON Center getShardAccountCell: Api (HTTP 200) (API 500)",
            "TON Center getShardAccountCell: Api (HTTP 500) (API 500)",
            "TON Center getShardAccountCell: Decode (HTTP 200): response field .",
            "Failed to decode shard account cell BOC data: unknown BOC tag",
        ]
    "#]]
    .assert_debug_eq(&results);
    Ok(())
}

#[test]
fn deserializes_toncenter_v3_tick_tock_without_credit_phase() {
    // Tick-tock has neither credit_first nor credit_ph in TonCenter v3.
    let value = serde_json::json!({
        "type": "tick_tock",
        "aborted": false,
        "destroyed": false,
        "is_tock": false,
        "storage_ph": {
            "storage_fees_collected": "0",
            "status_change": "unchanged"
        },
        "compute_ph": {
            "skipped": false,
            "success": true,
            "exit_code": 0
        }
    });

    let description: v3::TransactionDescr = serde_json::from_value(value).unwrap();
    let compute = description.compute_ph.unwrap();
    expect_test::expect![[r#"
        (
            "tick_tock",
            None,
            Some(
                true,
            ),
            Some(
                0,
            ),
        )
    "#]]
    .assert_debug_eq(&(
        description.kind,
        description.credit_first,
        compute.success,
        compute.exit_code,
    ));
}

#[test]
fn deserializes_toncenter_v3_transaction_with_skipped_compute_phase() {
    let value = serde_json::json!({
        "transactions": [{
            "account": "0:F7E97472D4849F481F339A5490281B1AE5B99E8B1016C03EAF51484E5D7BABF1",
            "hash": "6BOT/kLF43JNLlC5hACgFtni2TjHC9s2Beaig0EDe0w=",
            "lt": "66023973000007",
            "now": 1777378799,
            "mc_block_seqno": 123,
            "trace_id": "HVJuGGDRxhB6vGFpyLDfa7o0qlHIrSd3RRuLYG5falo=",
            "prev_trans_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "prev_trans_lt": "0",
            "orig_status": "active",
            "end_status": "active",
            "total_fees": "0",
            "total_fees_extra_currencies": {},
            "description": {
                "type": "ord",
                "aborted": true,
                "destroyed": false,
                "credit_first": true,
                "storage_ph": {
                    "storage_fees_collected": "0",
                    "status_change": "unchanged"
                },
                "credit_ph": {
                    "credit": "1"
                },
                "compute_ph": {
                    "skipped": true,
                    "reason": "no_gas"
                }
            },
            "block_ref": {
                "workchain": 0,
                "shard": "8000000000000000",
                "seqno": 1
            },
            "account_state_before": {
                "hash": "before"
            },
            "account_state_after": {
                "hash": "after"
            },
            "emulated": false,
            "finality": "finalized"
        }],
        "address_book": {}
    });

    let data: v3::TransactionsResponse =
        serde_json::from_value(value).expect("skipped compute phase response should deserialize");
    let compute = data.transactions[0]
        .description
        .compute_ph
        .as_ref()
        .expect("compute phase should be present");

    assert_eq!(compute.skipped, Some(true));
    assert_eq!(compute.reason.as_deref(), Some("no_gas"));
    assert_eq!(compute.success, None);
    assert_eq!(compute.exit_code, None);
}
