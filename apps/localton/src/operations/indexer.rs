//! Prepares local chain state for the bundled indexer services.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::time::{Instant, sleep};

use crate::{
    cli::{IndexerCommand, WalletVersion},
    operations::wallets,
};

const BASECHAIN_WORKCHAIN: i32 = 0;
const BOOTSTRAP_WALLET_ID: u32 = 42;
const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapResult {
    masterchain_seqno: u32,
    basechain_seqno: u32,
    funded: bool,
}

#[derive(Debug)]
struct BootstrapOptions {
    state_dir: PathBuf,
    endpoint: String,
    wallet: String,
    amount: String,
    seqno_file: Option<PathBuf>,
    timeout: Duration,
}

#[derive(Deserialize)]
struct TonResponse<T> {
    ok: bool,
    result: Option<T>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct MasterchainInfo {
    last: BlockId,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ShardsResult {
    Object { shards: Vec<BlockId> },
    List(Vec<BlockId>),
}

impl ShardsResult {
    fn shards(&self) -> &[BlockId] {
        match self {
            Self::Object { shards } | Self::List(shards) => shards,
        }
    }
}

#[derive(Deserialize)]
struct BlockId {
    workchain: i32,
    seqno: u32,
}

pub async fn execute(command: IndexerCommand) -> Result<()> {
    match command {
        IndexerCommand::BootstrapBasechain {
            state,
            endpoint,
            wallet,
            amount,
            seqno_file,
            timeout,
        } => {
            let result = bootstrap_basechain(BootstrapOptions {
                state_dir: state.state_dir,
                endpoint,
                wallet,
                amount,
                seqno_file,
                timeout: Duration::from_secs(timeout),
            })
            .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
    }
}

async fn bootstrap_basechain(options: BootstrapOptions) -> Result<BootstrapResult> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("failed to create TON HTTP API client")?;

    let (masterchain_seqno, basechain_seqno, funded) = if let Some((masterchain, basechain)) =
        read_indexable_masterchain_seqno(&client, &options.endpoint, options.timeout).await?
    {
        (masterchain, basechain, false)
    } else {
        wallets::ensure_wallet(
            &options.state_dir,
            &options.wallet,
            WalletVersion::V4r2,
            BASECHAIN_WORKCHAIN,
            BOOTSTRAP_WALLET_ID,
        )
        .await?;
        wallets::fund_wallet(&options.state_dir, &options.wallet, &options.amount).await?;
        let (masterchain, basechain) =
            wait_for_indexable_masterchain_seqno(&client, &options.endpoint, options.timeout)
                .await?;
        (masterchain, basechain, true)
    };

    if let Some(path) = options.seqno_file.as_deref() {
        write_seqno_file(path, masterchain_seqno).await?;
    }

    Ok(BootstrapResult {
        masterchain_seqno,
        basechain_seqno,
        funded,
    })
}

async fn read_indexable_masterchain_seqno(
    client: &Client,
    endpoint: &str,
    timeout: Duration,
) -> Result<Option<(u32, u32)>> {
    let deadline = Instant::now() + timeout;
    loop {
        match indexable_masterchain_seqno(client, endpoint).await {
            Ok(seqnos) => return Ok(seqnos),
            Err(error) if Instant::now() < deadline => {
                tracing::debug!(%error, "waiting for TON HTTP API shard data");
                sleep(POLL_INTERVAL).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn wait_for_indexable_masterchain_seqno(
    client: &Client,
    endpoint: &str,
    timeout: Duration,
) -> Result<(u32, u32)> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    loop {
        match indexable_masterchain_seqno(client, endpoint).await {
            Ok(Some(seqnos)) => return Ok(seqnos),
            Ok(None) => {}
            Err(error) => last_error = Some(format!("{error:#}")),
        }
        if Instant::now() >= deadline {
            let detail = last_error
                .map(|error| format!(": {error}"))
                .unwrap_or_default();
            bail!(
                "basechain did not produce an indexable block within {} seconds{detail}",
                timeout.as_secs()
            );
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn indexable_masterchain_seqno(
    client: &Client,
    endpoint: &str,
) -> Result<Option<(u32, u32)>> {
    let endpoint = endpoint.trim_end_matches('/');
    let masterchain: MasterchainInfo = get_result(
        client,
        Url::parse(&format!("{endpoint}/getMasterchainInfo"))?,
    )
    .await?;
    let mut shards_url = Url::parse(&format!("{endpoint}/shards"))?;
    shards_url
        .query_pairs_mut()
        .append_pair("seqno", &masterchain.last.seqno.to_string());
    let shards: ShardsResult = get_result(client, shards_url).await?;
    Ok(shards
        .shards()
        .iter()
        .filter(|shard| shard.workchain == BASECHAIN_WORKCHAIN)
        .map(|shard| shard.seqno)
        .max()
        .filter(|seqno| *seqno > 0)
        .map(|basechain| (masterchain.last.seqno, basechain)))
}

async fn get_result<T: DeserializeOwned>(client: &Client, url: Url) -> Result<T> {
    let response = client
        .get(url.clone())
        .send()
        .await
        .with_context(|| format!("TON HTTP API request failed: {url}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .with_context(|| format!("failed to read TON HTTP API response: {url}"))?;
    let response: TonResponse<T> = serde_json::from_slice(&body).with_context(|| {
        format!(
            "invalid TON HTTP API response from {url}: {}",
            String::from_utf8_lossy(&body)
                .chars()
                .take(512)
                .collect::<String>()
        )
    })?;
    if !status.is_success() || !response.ok {
        bail!(
            "TON HTTP API request to {url} failed with status {status}: {}",
            response
                .error
                .unwrap_or_else(|| "no error details".to_owned())
        );
    }
    response
        .result
        .with_context(|| format!("TON HTTP API response from {url} did not include a result"))
}

async fn write_seqno_file(path: &Path, seqno: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temporary = path.with_extension("tmp");
    tokio::fs::write(&temporary, format!("{seqno}\n"))
        .await
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    tokio::fs::rename(&temporary, path)
        .await
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, routing::get};
    use expect_test::expect;
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::{BootstrapOptions, bootstrap_basechain};

    #[tokio::test]
    async fn existing_basechain_writes_scanner_masterchain_seqno_without_funding() {
        let app = Router::new()
            .route(
                "/api/v2/getMasterchainInfo",
                get(|| async {
                    Json(json!({
                        "ok": true,
                        "result": {
                            "last": {"workchain": -1, "seqno": 44}
                        }
                    }))
                }),
            )
            .route(
                "/api/v2/shards",
                get(|| async {
                    Json(json!({
                        "ok": true,
                        "result": {
                            "shards": [
                                {"workchain": 0, "seqno": 7},
                                {"workchain": -1, "seqno": 44}
                            ]
                        }
                    }))
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let seqno_file = root.path().join("account-scan/target-seqno");

        let result = bootstrap_basechain(BootstrapOptions {
            state_dir: root.path().join("localton"),
            endpoint: format!("http://{address}/api/v2"),
            wallet: "studio-indexer-bootstrap".to_owned(),
            amount: "1".to_owned(),
            seqno_file: Some(seqno_file.clone()),
            timeout: std::time::Duration::from_secs(1),
        })
        .await
        .unwrap();
        let marker = tokio::fs::read_to_string(seqno_file).await.unwrap();
        server.abort();

        expect![[r#"
            result: BootstrapResult {
                masterchain_seqno: 44,
                basechain_seqno: 7,
                funded: false,
            }
            marker: 44
            "#]]
        .assert_eq(&format!("result: {result:#?}\nmarker: {marker}"));
    }
}
