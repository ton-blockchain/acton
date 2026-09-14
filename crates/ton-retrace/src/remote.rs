//! Asynchronous access to TON Center using the shared API response models.

use crate::Network;
use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use ton_api::toncenter::{v2, v3};
use ton_executor::message::{PrevBlockId, PrevBlocksInfo};
use ton_networks::CustomNetworkUrls;
use toncenter_keys::api_key as toncenter_api_key;
use tycho_types::boc::Boc;
use tycho_types::prelude::Cell;

const USE_PROXY_ENV: &str = "ACTON_USE_PROXY";
const TONCENTER_MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(1200);
static TONCENTER_REQUEST_GATE: LazyLock<Mutex<Option<Instant>>> =
    LazyLock::new(|| Mutex::new(None));

const fn user_agent() -> &'static str {
    concat!("acton/", env!("CARGO_PKG_VERSION"))
}

fn http_client_builder() -> reqwest::ClientBuilder {
    let builder = Client::builder().use_rustls_tls().user_agent(user_agent());
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

fn proxy_enabled() -> bool {
    proxy_enabled_from_value(env::var_os(USE_PROXY_ENV).as_deref())
}

fn proxy_enabled_from_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| {
        let value = value.to_string_lossy();
        let value = value.trim();
        value == "1" || value == "true"
    })
}

/// Client for `TON Center` V2/V3 API.
///
/// Used for fetching transaction metadata, block information, and library cells.
pub(crate) struct TonCenterClient {
    client: Client,
    api_key: Option<String>,
    v2_url: String,
    v3_url: String,
    rate_limited: bool,
}

impl TonCenterClient {
    /// Resolves both API endpoints through the shared network configuration.
    /// V2 and V3 may use different origins or path prefixes; neither is derived
    /// from the other. Custom networks must configure both APIs for replay.
    pub(crate) fn new(
        network: Network,
        custom_networks: &HashMap<String, CustomNetworkUrls>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            client: http_client_builder().build()?,
            api_key: toncenter_api_key(&network),
            v2_url: network.toncenter_v2_url(custom_networks)?,
            v3_url: network.toncenter_v3_url(custom_networks)?,
            rate_limited: matches!(network, Network::Mainnet | Network::Testnet),
        })
    }

    /// Applies a simple global rate limit for unauthenticated `TON Center` requests.
    ///
    /// `TON Center` has stricter limits without an API key, so we serialize
    /// requests and keep at least 1 second between request starts.
    async fn maybe_wait_for_rate_limit(&self) {
        if !self.rate_limited || self.api_key.is_some() {
            return;
        }

        let mut last_request = TONCENTER_REQUEST_GATE.lock().await;
        if let Some(last) = *last_request {
            let elapsed = last.elapsed();
            if elapsed < TONCENTER_MIN_REQUEST_INTERVAL {
                let wait_for = TONCENTER_MIN_REQUEST_INTERVAL - elapsed;
                log::debug!("throttle for {wait_for:?}");
                tokio::time::sleep(wait_for).await;
            }
        }
        *last_request = Some(Instant::now());
    }

    /// Sends an authenticated request and decodes a shared TON Center response.
    /// Error envelopes are checked before success deserialization so API errors
    /// retain their endpoint context without including the response payload.
    async fn get<T: DeserializeOwned>(
        &self,
        base_url: &str,
        method: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<T> {
        let url = format!("{}/{method}", base_url.trim_end_matches('/'));
        let mut request = self.client.get(url).query(query);
        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        self.maybe_wait_for_rate_limit().await;
        let response = request
            .send()
            .await
            .with_context(|| format!("TON Center {method} request failed"))?;
        let status = response.status();
        let value: serde_json::Value = response
            .json()
            .await
            .with_context(|| format!("Failed to decode TON Center {method} response ({status})"))?;

        if let Some(error) = value.get("error") {
            anyhow::bail!("TON Center {method} error ({status}): {error}");
        }
        if !status.is_success()
            || value.get("ok").and_then(serde_json::Value::as_bool) == Some(false)
        {
            anyhow::bail!("TON Center {method} request failed ({status})");
        }

        serde_json::from_value(value)
            .with_context(|| format!("Failed to decode TON Center {method} response"))
    }

    /// Unwraps the V2 envelope; result schemas are owned by `ton-api`.
    async fn get_v2<T: DeserializeOwned>(
        &self,
        method: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<T> {
        let response: v2::TonlibResponse<T> = self.get(&self.v2_url, method, query).await?;
        Ok(response.result)
    }

    /// Fetches indexed transaction metadata with V3 filters.
    pub(crate) async fn get_transactions(
        &self,
        query: &[(&str, String)],
    ) -> anyhow::Result<v3::TransactionsResponse> {
        self.get(&self.v3_url, "transactions", query).await
    }

    /// Loads the shard header containing a transaction, including its random seed.
    pub(crate) async fn get_blocks(
        &self,
        block: &v3::BlockId,
    ) -> anyhow::Result<v3::BlocksResponse> {
        self.get(
            &self.v3_url,
            "blocks",
            &[
                ("workchain", block.workchain.to_string()),
                ("shard", block.shard.clone()),
                ("seqno", block.seqno.to_string()),
            ],
        )
        .await
    }

    /// Reconstructs c7 history at the referenced masterchain state, newest first.
    /// The anchor is included, as is zerostate when fewer than 16 blocks exist.
    /// Lookups are sequential to respect public API limits; overlapping lists
    /// share results for the duration of this reconstruction.
    pub(crate) async fn get_prev_blocks_info(
        &self,
        mc_seqno: u32,
        with_100: bool,
    ) -> anyhow::Result<PrevBlocksInfo> {
        async {
            let mut blocks = HashMap::new();
            let key_seqno = if mc_seqno == 0 {
                0
            } else {
                let header: v2::BlockHeader = self
                    .get_v2(
                        "getBlockHeader",
                        &[
                            ("workchain", "-1".to_owned()),
                            ("shard", "8000000000000000".to_owned()),
                            ("seqno", mc_seqno.to_string()),
                        ],
                    )
                    .await?;
                let key_seqno = if header.is_key_block {
                    mc_seqno
                } else {
                    u32::try_from(header.prev_key_block_seqno)
                        .context("Invalid previous key block seqno")?
                };
                anyhow::ensure!(
                    key_seqno <= mc_seqno,
                    "Previous key block is after masterchain block {mc_seqno}"
                );
                blocks.insert(mc_seqno, prev_block_id(header.id, mc_seqno)?);
                key_seqno
            };

            let recent_seqnos = (mc_seqno.saturating_sub(15)..=mc_seqno)
                .rev()
                .collect::<Vec<_>>();
            let hundred_seqnos = if with_100 {
                (0..=mc_seqno / 100)
                    .rev()
                    .take(16)
                    .map(|seqno| seqno * 100)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            for seqno in std::iter::once(key_seqno)
                .chain(recent_seqnos.iter().copied())
                .chain(hundred_seqnos.iter().copied())
            {
                if blocks.contains_key(&seqno) {
                    continue;
                }

                // lookupBlock cannot return zerostate; the network's initial ID
                // is available in getMasterchainInfo instead.
                let id = if seqno == 0 {
                    let info: v2::MasterchainInfo = self.get_v2("getMasterchainInfo", &[]).await?;
                    info.init
                } else {
                    self.get_v2(
                        "lookupBlock",
                        &[
                            ("workchain", "-1".to_owned()),
                            ("shard", "8000000000000000".to_owned()),
                            ("seqno", seqno.to_string()),
                        ],
                    )
                    .await?
                };
                blocks.insert(seqno, prev_block_id(id, seqno)?);
            }

            Ok(PrevBlocksInfo::new(
                recent_seqnos
                    .iter()
                    .map(|seqno| blocks[seqno].clone())
                    .collect(),
                blocks[&key_seqno].clone(),
                with_100.then(|| {
                    hundred_seqnos
                        .iter()
                        .map(|seqno| blocks[seqno].clone())
                        .collect()
                }),
            ))
        }
        .await
        .with_context(|| format!("Failed to load previous blocks at masterchain block {mc_seqno}"))
    }

    /// Loads raw archival transactions in newest-to-oldest order for replay.
    pub(crate) async fn get_account_transactions(
        &self,
        address: &str,
        lt: u64,
        hash: &str,
        to_lt: u64,
        limit: u32,
    ) -> anyhow::Result<Vec<v2::Transaction>> {
        self.get_v2(
            "getTransactions",
            &[
                ("address", address.to_owned()),
                ("lt", lt.to_string()),
                ("hash", hash.to_owned()),
                ("to_lt", to_lt.to_string()),
                ("limit", limit.to_string()),
                ("archival", "true".to_owned()),
            ],
        )
        .await
    }

    /// Fetches the global library cell needed to resolve an exotic code cell.
    pub(crate) async fn get_libraries(&self, hash: &str) -> anyhow::Result<String> {
        let libraries: v2::LibraryResult = self
            .get_v2("getLibraries", &[("libraries", hash.to_owned())])
            .await?;
        libraries
            .result
            .into_iter()
            .next()
            .map(|library| library.data)
            .with_context(|| format!("TON Center library {hash} not found"))
    }

    /// Fetches the configuration in effect at the replayed masterchain block.
    pub(crate) async fn get_config_all(&self, seqno: u32) -> anyhow::Result<Cell> {
        let config: v2::ConfigInfo = self
            .get_v2("getConfigAll", &[("seqno", seqno.to_string())])
            .await?;
        Boc::decode_base64(config.config.bytes)
            .context("Failed to decode blockchain config BOC data")
    }

    /// Loads a serialized `ShardAccount`, preserving its previous transaction reference.
    pub(crate) async fn get_shard_account_cell(
        &self,
        seqno: u32,
        address: &str,
    ) -> anyhow::Result<Cell> {
        let cell: v2::TvmCell = self
            .get_v2(
                "getShardAccountCell",
                &[
                    ("address", address.to_owned()),
                    ("seqno", seqno.to_string()),
                ],
            )
            .await?;
        Boc::decode_base64(cell.bytes).context("Failed to decode shard account cell BOC data")
    }
}

/// Validates API block IDs before exposing their hashes to contract code in c7.
fn prev_block_id(id: v2::TonBlockIdExt, expected_seqno: u32) -> anyhow::Result<PrevBlockId> {
    anyhow::ensure!(
        id.workchain == -1
            && (matches!(
                id.shard.as_str(),
                "-9223372036854775808" | "8000000000000000"
            ) || (expected_seqno == 0 && id.shard == "0"))
            && id.seqno == u64::from(expected_seqno),
        "TON Center returned an unexpected masterchain block for seqno {expected_seqno}"
    );
    let decode_hash = |value: &str| -> anyhow::Result<[u8; 32]> {
        STANDARD
            .decode(value)?
            .try_into()
            .map_err(|bytes: Vec<u8>| {
                anyhow::anyhow!(
                    "Invalid block hash length: expected 32 bytes, got {}",
                    bytes.len()
                )
            })
    };

    Ok(PrevBlockId {
        workchain: -1,
        shard: i64::MIN,
        seqno: expected_seqno,
        root_hash: decode_hash(&id.root_hash).context("Invalid masterchain root hash")?,
        file_hash: decode_hash(&id.file_hash).context("Invalid masterchain file hash")?,
    })
}

#[cfg(test)]
mod tests {
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
    use ton_api::toncenter::v3;
    use tycho_types::cell::{CellBuilder, CellFamily, HashBytes, Lazy};
    use tycho_types::models::{
        AccountStatus, ComputePhase, ComputePhaseSkipReason, HashUpdate, OrdinaryTxInfo,
        SkippedComputePhase, StdAddr, Transaction, TxInfo,
    };

    async fn serve_response(
        status: &str,
        response: Value,
    ) -> (TonCenterClient, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}/api/v3", listener.local_addr().unwrap());
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
        let client = TonCenterClient {
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            api_key: Some("fixture-key".to_owned()),
            v2_url: base_url.replace("/api/v3", "/api/v2"),
            v3_url: base_url,
            rate_limited: false,
        };
        (client, server)
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
        let (client, server) = serve_response("200 OK", json!({
            "ok": true, "@extra": "", "result": {
                "@type": "blocks.masterchainInfo", "last": zero, "init": zero, "state_root_hash": "",
            },
        })).await;
        let info = client.get_prev_blocks_info(0, true).await?;
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
            "@type": "raw.transaction",
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
            let (v2, v2_server) = serve_response("200 OK", json!({
                "ok": true, "@extra": "", "result": {"@type": "tvm.cell", "bytes": Boc::encode_base64(&cell)}
            })).await;
            let (v3, v3_server) =
                serve_response("200 OK", json!({"transactions": [], "address_book": {}})).await;
            let networks = HashMap::from([(
                network.as_str(),
                CustomNetworkUrls {
                    v2_url: v2.v2_url.replace("/api/v2", "/rpc/archive/").into(),
                    v3_url: Some(v3.v3_url.replace("/api/v3", "/indexer/").into()),
                    explorer_url: None,
                },
            )]);
            let mut client = TonCenterClient::new(network.clone(), &networks)?;
            client.client = v2.client;

            let account = client.get_shard_account_cell(42, "0:11").await?;
            let transactions = client
                .get_transactions(&[("hash", "fixture".to_owned())])
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
        let missing = TonCenterClient::new(Network::Custom("archive".into()), &HashMap::new())
            .err()
            .expect("missing network should fail");
        let networks = HashMap::from([(
            "archive".to_owned(),
            CustomNetworkUrls {
                v2_url: "http://localhost/rpc".into(),
                v3_url: None,
                explorer_url: None,
            },
        )]);
        let missing_v3 = TonCenterClient::new(Network::Custom("archive".into()), &networks)
            .err()
            .expect("missing v3 URL should fail");
        expect![[r#"
            (
                "unknown custom network: archive",
                "v3_url not configured for custom network: archive",
            )
        "#]]
        .assert_debug_eq(&(missing.to_string(), missing_v3.to_string()));
    }

    #[tokio::test]
    async fn archive_history_preserves_order_and_excludes_snapshot_transaction()
    -> anyhow::Result<()> {
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
        let (client, server) = serve_response("200 OK", json!({
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
        let (client, server) = serve_response(
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
            let (client, server) = serve_response(
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
            let (client, server) = serve_response(status, response).await;
            let error = client.get_shard_account_cell(1, "0:11").await.unwrap_err();
            server.await?;
            results.push(format!("{error:#}"));
        }
        expect![[r#"
            [
                "TON Center getShardAccountCell error (200 OK): \"archive unavailable\"",
                "TON Center getShardAccountCell error (500 Internal Server Error): \"archive unavailable\"",
                "Failed to decode TON Center getShardAccountCell response: missing field `result`",
                "Failed to decode shard account cell BOC data: unknown BOC tag",
            ]
        "#]].assert_debug_eq(&results);
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

        let data: v3::TransactionsResponse = serde_json::from_value(value)
            .expect("skipped compute phase response should deserialize");
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
}
