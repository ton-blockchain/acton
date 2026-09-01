//! Host-local joiner and supervisor for follower full nodes.
//!
//! On its first run, the join workflow downloads a standard TON global config. It
//! creates independent databases and keys, starts independent nodes, and keeps
//! private material on this host. With `--validator`, it can fund a host-local
//! wallet from a development faucet and enters elections directly through the
//! TON Elector contract.

mod ports;

use std::{
    collections::BTreeSet,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use futures_util::StreamExt;
use indicatif::BinaryBytes;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::select;
use tracing::{info, warn};

use crate::{
    binaries::TonBinaries,
    bootstrap::{NodeController, acquire_lock, shutdown_signal, supervise},
    cli::{JoinArgs, WalletVersion},
    operations::{validators, wallets},
    runtime::ProcessRegistry,
    storage::{Layout, NodeSettings, RuntimeState, Settings, write_json_atomic},
    ton::{
        global_config::GlobalConfig,
        lite::LocalLiteClient,
        toolchain::Toolchain,
        tools::{
            types::{OperationContext, TonPublicKey},
            validator_console::ValidatorSynchronization,
        },
    },
};

use self::ports::{DEFAULT_JOIN_PORT_BASE, HostPortAllocation};

const MAX_GLOBAL_CONFIG_BYTES: u64 = 1024 * 1024;
const VALIDATOR_WALLET_WORKCHAIN: i32 = -1;
const VALIDATOR_WALLET_ID: u32 = 42;
const VALIDATOR_FEE_RESERVE_NANO: u64 = 5_000_000_000;
const SYNC_READY_CONFIRMATIONS: usize = 3;
const SYNC_POLL_INTERVAL: Duration = Duration::from_millis(500);
const SYNC_LOG_INTERVAL: Duration = Duration::from_secs(5);
const SYNC_STATS_TIMEOUT: Duration = Duration::from_secs(3);
const SYNC_LAG_TOLERANCE_BLOCKS: u32 = 2;

/// Development faucet response used only to verify the requested wallet and amount.
#[derive(Deserialize)]
struct FaucetGrant {
    address: String,
    amount_nano: u64,
}

/// Authenticated endpoint of one liteserver owned by this join invocation.
#[derive(Clone)]
struct LocalLiteserver {
    node: String,
    port: u16,
    public_key: String,
}

/// Joins an existing TON network and owns every follower process for this state directory.
///
/// The workflow prepares durable node state before process startup, waits until each
/// local liteserver follows the network head, and only then enables validator automation.
/// Every exit path converges on process shutdown and runtime-state cleanup.
pub async fn run(args: JoinArgs) -> Result<()> {
    std::fs::create_dir_all(&args.state.state_dir).with_context(|| {
        format!(
            "failed to create join state directory {}",
            args.state.state_dir.display()
        )
    })?;

    let state_root = dunce::canonicalize(&args.state.state_dir).with_context(|| {
        format!(
            "failed to resolve join state directory {}",
            args.state.state_dir.display()
        )
    })?;

    let layout = Layout::new(state_root);
    layout.create_dirs()?;

    let _state_lock = acquire_lock(&layout.lock)?;
    let processes = ProcessRegistry::default();

    // Install signal handling before any TON process starts. Otherwise Ctrl+C
    // during initial synchronization terminates only the Localton instance and skips
    // the managed-process cleanup below.
    let run_result = select! {
        result = async {
            // Commit network identity and node settings before starting processes.
            let owned_nodes = prepare_follower_state(&layout, &args).await?;
            let binaries = TonBinaries::resolve(&layout, args.ton_bin_dir.clone()).await?;
            let toolchain = Toolchain::official(layout.clone(), binaries.clone());
            let control = NodeController::new(
                layout.clone(),
                toolchain.clone(),
                Duration::from_secs(args.startup_timeout),
                processes.clone(),
            );
            let settings = Settings::load(&layout.settings)?;

            // Start every host-local node and retain the liteserver identity needed
            // to query it without trusting the remote global-config liteserver list.
            let mut local_liteservers = Vec::new();
            for name in &owned_nodes {
                let runtime = control
                    .start_node(name)
                    .await
                    .with_context(|| format!("failed to start follower full node `{name}`"))?;
                if let Some(public_key) = runtime.liteserver_public_key {
                    local_liteservers.push(LocalLiteserver {
                        node: name.clone(),
                        port: settings.node(name)?.liteserver_port,
                        public_key,
                    });
                }
            }

            let primary_liteserver = local_liteservers
                .first()
                .cloned()
                .context("join has no running node with a local liteserver")?;

            // Publish instance ownership before the potentially long initial sync.
            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                runtime.mark_instance_started();
                for name in &owned_nodes {
                    if let Some(node) = runtime.nodes.get_mut(name) {
                        node.begin_synchronization();
                    }
                }
                Ok(())
            })?;

            // A node is usable only after several consecutive near-head samples.
            for liteserver in &local_liteservers {
                select! {
                    result = wait_for_network_sync(
                        &layout,
                        &toolchain,
                        settings.node(&liteserver.node)?,
                        liteserver,
                    ) => result?,
                    result = supervise(&processes) => return result,
                }
                RuntimeState::update_atomic(&layout.runtime, |runtime| {
                    if let Some(node) = runtime.nodes.get_mut(&liteserver.node) {
                        node.status = "running".to_owned();
                    }
                    Ok(())
                })?;
            }

            // Once synchronized, all wallet and election work must use a node on
            // this host instead of depending on the bootstrap host's liteserver.
            prefer_local_liteserver(
                &layout.global_config,
                primary_liteserver.port,
                &primary_liteserver.public_key,
            )?;
            info!(
                endpoint = %format!("127.0.0.1:{}", primary_liteserver.port),
                "join operations now use the synchronized local liteserver"
            );

            let validator_nodes: Vec<_> = settings
                .nodes
                .iter()
                .filter(|node| owned_nodes.contains(&node.name) && node.validator)
                .map(|node| node.name.as_str())
                .collect();
            info!(nodes = ?owned_nodes, global_config = %args.global_config_url, validators = ?validator_nodes, "joined Localton nodes are running");

            // Election automation is long-lived and supervised alongside the TON
            // processes so either failure tears down the whole owned instance.
            let validation_interval = toolchain.settings()?.validation.poll_interval_seconds;
            let validation = validation_loop(
                toolchain,
                args.faucet.clone(),
                owned_nodes,
                validation_interval,
            );
            tokio::pin!(validation);
            let result = select! {
                result = supervise(&processes) => result,
                result = &mut validation => result,
            };

            result
        } => result,
        signal_result = shutdown_signal() => signal_result,
    };

    // Cleanup runs even when preparation, synchronization, supervision, or signal
    // handling fails. Preserve all errors by combining the independent results.
    let stop_result = processes.stop_all().await;
    let state_result = RuntimeState::update_atomic(&layout.runtime, |runtime| {
        runtime.mark_instance_stopped();
        Ok(())
    });

    run_result.and(stop_result).and(state_result.map(|_| ()))
}

/// Waits until one local liteserver remains close to the public network head.
///
/// Consecutive confirmations prevent a transient near-head response from marking the
/// node ready. Before the liteserver answers, validator-console statistics provide
/// protocol-specific initial-sync progress for logs and runtime state.
async fn wait_for_network_sync(
    layout: &Layout,
    toolchain: &Toolchain,
    node: &NodeSettings,
    liteserver: &LocalLiteserver,
) -> Result<()> {
    let mut confirmations = 0;
    let mut last_log = Instant::now()
        .checked_sub(SYNC_LOG_INTERVAL)
        .unwrap_or_else(Instant::now);

    loop {
        // Compare the remote network head with the same node through its private
        // local liteserver endpoint. Both clients are recreated because an endpoint
        // may become available while validator-engine is still initializing.
        let sample: Result<(u32, u32)> = async {
            let mut network = LocalLiteClient::connect(&layout.global_config).await?;
            let network_head = network.last().await?.seqno;

            let mut local = LocalLiteClient::connect_node(
                &layout.global_config,
                liteserver.port,
                &liteserver.public_key,
            )
            .await?;
            let local_head = local.last().await?.seqno;
            Ok((network_head, local_head))
        }
        .await;

        match sample {
            Ok((network_head, local_head)) => {
                let lag = network_head.saturating_sub(local_head);

                if last_log.elapsed() >= SYNC_LOG_INTERVAL {
                    if let Err(error) = RuntimeState::update_atomic(&layout.runtime, |runtime| {
                        if let Some(node) = runtime.nodes.get_mut(&liteserver.node) {
                            node.observe_sync_progress(local_head, network_head);
                        }
                        Ok(())
                    }) {
                        warn!(node = liteserver.node, %error, "could not publish follower synchronization progress");
                    }

                    info!(
                        node = liteserver.node,
                        local_head,
                        network_head,
                        lag_blocks = lag,
                        "follower node synchronization progress"
                    );
                    last_log = Instant::now();
                }

                if lag <= SYNC_LAG_TOLERANCE_BLOCKS {
                    confirmations += 1;

                    if confirmations >= SYNC_READY_CONFIRMATIONS {
                        if let Err(error) =
                            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                                if let Some(node) = runtime.nodes.get_mut(&liteserver.node) {
                                    node.observe_sync_progress(local_head, network_head);
                                }
                                Ok(())
                            })
                        {
                            warn!(node = liteserver.node, %error, "could not publish final follower synchronization progress");
                        }

                        info!(
                            node = liteserver.node,
                            local_head,
                            network_head,
                            lag_blocks = lag,
                            "follower node synchronized"
                        );

                        return Ok(());
                    }
                } else {
                    confirmations = 0;
                }
            }
            Err(error) => {
                confirmations = 0;

                if last_log.elapsed() >= SYNC_LOG_INTERVAL {
                    // The local liteserver normally rejects queries during initial
                    // state download, so fall back to validator-console statistics.
                    let stats = toolchain
                        .validator_console_tool
                        .health(
                            &OperationContext::for_node(SYNC_STATS_TIMEOUT, &node.name),
                            &toolchain.validator_console_endpoint(node),
                        )
                        .await;

                    match stats.and_then(|stats| stats.synchronization()) {
                        Ok(ValidatorSynchronization::BlockTime {
                            block_time,
                            target_time,
                        }) => {
                            if let Err(publish_error) =
                                RuntimeState::update_atomic(&layout.runtime, |runtime| {
                                    if let Some(node) = runtime.nodes.get_mut(&liteserver.node) {
                                        node.observe_sync_time_progress(block_time, target_time);
                                    }
                                    Ok(())
                                })
                            {
                                warn!(node = liteserver.node, %publish_error, "could not publish time-based follower synchronization progress");
                            }

                            info!(
                                node = liteserver.node,
                                masterchain_block_time = block_time,
                                target_time,
                                lag_seconds = target_time.saturating_sub(block_time),
                                "follower node synchronization progress"
                            );
                        }
                        Ok(ValidatorSynchronization::Initial(progress)) => {
                            let state_download = progress.state_download.as_ref();
                            if let Err(publish_error) =
                                RuntimeState::update_atomic(&layout.runtime, |runtime| {
                                    if let Some(node) = runtime.nodes.get_mut(&liteserver.node) {
                                        node.observe_initial_sync_progress(progress.clone());
                                    }
                                    Ok(())
                                })
                            {
                                warn!(node = liteserver.node, %publish_error, "could not publish initial follower synchronization progress");
                            }

                            if let Some(download) = state_download {
                                info!(
                                    node = liteserver.node,
                                    stage = ?progress.stage,
                                    masterchain_seqno = ?progress.masterchain_seqno,
                                    current_part = ?progress.current_part,
                                    total_parts = ?progress.total_parts,
                                    downloaded = %BinaryBytes(download.downloaded_bytes),
                                    total = %BinaryBytes(download.total_bytes),
                                    speed = %format!("{}/s", BinaryBytes(download.bytes_per_second)),
                                    eta_seconds = download.remaining_seconds,
                                    "follower node initial synchronization progress"
                                );
                            } else {
                                info!(
                                    node = liteserver.node,
                                    stage = ?progress.stage,
                                    masterchain_seqno = ?progress.masterchain_seqno,
                                    current_part = ?progress.current_part,
                                    total_parts = ?progress.total_parts,
                                    "follower node initial synchronization progress"
                                );
                            }
                        }
                        Ok(ValidatorSynchronization::WaitingForMasterchain) => info!(
                            node = liteserver.node,
                            "follower node is preparing its first masterchain block"
                        ),
                        Err(stats_error) => {
                            warn!(
                                node = liteserver.node,
                                liteserver_error = %format!("{error:#}"),
                                validator_stats_error = %format!("{stats_error:#}"),
                                "could not measure follower synchronization"
                            );
                        }
                    }
                    last_log = Instant::now();
                }
            }
        }

        tokio::time::sleep(SYNC_POLL_INTERVAL).await;
    }
}

/// Prepares persistent follower configuration without starting external processes.
///
/// A saved global config is the network-identity commit marker for join state. An
/// existing node database without it is rejected because rebuilding against another
/// network could mix durable validator-engine state and keys.
async fn prepare_follower_state(layout: &Layout, args: &JoinArgs) -> Result<BTreeSet<String>> {
    ensure!(
        !layout.manifest.is_file(),
        "join requires a follower state directory, not a bootstrap state directory"
    );

    let mut settings = if layout.settings.is_file() {
        Settings::load(&layout.settings)?
    } else {
        Settings::for_join()
    };
    let requested = resolve_join_nodes(
        layout,
        &mut settings,
        &args.nodes,
        args.port_base,
        args.advertise_ip,
        args.validator,
    )?;

    // Once downloaded, the global config pins this state directory to one TON
    // network. A restart reuses it even if the source URL later changes.
    let global_config_exists = layout.global_config.is_file();
    let global_config = if global_config_exists {
        info!("reusing persisted TON global config");
        GlobalConfig::from_json_bytes(&std::fs::read(&layout.global_config).with_context(
            || {
                format!(
                    "failed to read persisted global config {}",
                    layout.global_config.display()
                )
            },
        )?)?
    } else {
        for name in &requested {
            let node = settings.node(name)?;
            ensure!(
                !layout.node(node).config_json().is_file(),
                "joining node `{name}` database exists without a global config"
            );
        }
        fetch_global_config(&args.global_config_url).await?
    };

    global_config.validate_advertise_ip(args.advertise_ip)?;

    if !global_config_exists {
        write_json_atomic(&layout.global_config, &global_config)?;
        info!(url = %args.global_config_url, "installed TON global config");
    }

    // Initialization-time identity fields become immutable once validator-engine
    // has created its database. Validator participation remains independently
    // mutable through the validator commands.
    for name in &requested {
        let node = settings.node_mut(name)?;
        let node_initialized = layout.node(node).config_json().is_file();
        if node_initialized {
            ensure!(
                node.public_ip == args.advertise_ip,
                "node `{name}` advertises {}; use the original --advertise-ip {}",
                node.public_ip,
                node.public_ip
            );
        } else {
            node.public_ip = args.advertise_ip;
            node.validator = args.validator;
            node.participate_in_elections = args.validator;
        }
        node.enabled = true;
    }

    // settings.json is written last so it never advertises a network configuration
    // that failed validation or could not be persisted.
    settings.validate()?;
    settings.save_atomic(&layout.settings)?;

    Ok(requested)
}

/// Resolves the node set owned by this join state and assigns first-run ports.
///
/// Node names and ports are persistent validator-engine identity inputs. Existing
/// settings therefore win on restart, and conflicting CLI names are rejected
/// instead of silently creating or renaming nodes.
fn resolve_join_nodes(
    layout: &Layout,
    settings: &mut Settings,
    requested: &[String],
    port_base: Option<u16>,
    advertise_ip: std::net::Ipv4Addr,
    validator: bool,
) -> Result<BTreeSet<String>> {
    let persisted = settings
        .nodes
        .iter()
        .filter(|node| node.enabled)
        .map(|node| node.name.clone())
        .collect::<BTreeSet<_>>();
    if !persisted.is_empty() {
        ensure!(
            !persisted.contains("genesis"),
            "join state cannot contain the bootstrap genesis node; use a new --state-dir"
        );
        if requested.is_empty() {
            return Ok(persisted);
        }

        let requested = requested.iter().cloned().collect::<BTreeSet<_>>();
        ensure!(
            requested == persisted,
            "joined node names are persisted; restart with --node {} or omit --node",
            persisted
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" --node ")
        );
        return Ok(persisted);
    }

    let names = if requested.is_empty() {
        // The state path gives a joining instance a deterministic name without
        // introducing another persisted identity solely for display purposes.
        let digest = Sha256::digest(layout.root.to_string_lossy().as_bytes());
        let name = format!("node-{}", hex::encode(&digest[..6]));
        BTreeSet::from([name])
    } else {
        let mut names = BTreeSet::new();
        for name in requested {
            ensure!(name != "genesis", "join cannot own the genesis node");
            ensure!(
                names.insert(name.clone()),
                "duplicate joining node `{name}`"
            );
        }
        names
    };

    // Probe one complete range before assigning any node so partial allocation
    // cannot leave settings with overlapping or half-selected ports.
    let allocation =
        HostPortAllocation::find(port_base.unwrap_or(DEFAULT_JOIN_PORT_BASE), names.len())?;
    settings.nodes = names
        .iter()
        .cloned()
        .zip(allocation.nodes.iter().copied())
        .map(|(name, ports)| {
            let mut node = NodeSettings::follower(name, advertise_ip, ports);
            node.validator = validator;
            node.participate_in_elections = validator;
            node
        })
        .collect();

    info!(
        port_range_start = allocation.start,
        port_range_end = allocation.end,
        nodes = names.len(),
        "allocated persistent join port range"
    );

    for node in &settings.nodes {
        info!(
            node = node.name,
            console_port = node.console_port,
            adnl_port = node.adnl_port,
            liteserver_port = node.liteserver_port,
            out_port = node.out_port,
            dht_port = node.dht_port,
            "allocated persistent node ports"
        );
    }

    Ok(names)
}

/// Downloads and validates a bounded standard TON global configuration.
///
/// The streaming size check does not trust `Content-Length`; chunked responses are
/// subject to the same limit before any data is parsed or written to durable state.
async fn fetch_global_config(source: &str) -> Result<GlobalConfig> {
    let url = http_url(source, "global config")?;
    let response = reqwest::Client::new()
        .get(url.clone())
        .send()
        .await
        .with_context(|| format!("failed to request {url}"))?
        .error_for_status()
        .with_context(|| format!("global config request was rejected by {url}"))?;

    if let Some(length) = response.content_length() {
        ensure!(
            length <= MAX_GLOBAL_CONFIG_BYTES,
            "global config is too large: {length} bytes"
        );
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed to download global config")?;
        ensure!(
            bytes.len() as u64 + chunk.len() as u64 <= MAX_GLOBAL_CONFIG_BYTES,
            "global config is too large"
        );
        bytes.extend_from_slice(&chunk);
    }

    let config = GlobalConfig::from_json_bytes(&bytes).context("global config is invalid")?;
    config.validate_for_node_join()?;

    Ok(config)
}

/// Replaces remote liteserver entries with the synchronized host-local endpoint.
///
/// DHT and validator network identity stay unchanged; only subsequent Localton
/// client operations are redirected through the node whose key was verified at startup.
fn prefer_local_liteserver(path: &Path, port: u16, public_key: &str) -> Result<()> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read global config {}", path.display()))?;
    let mut config = GlobalConfig::from_json_bytes(&bytes)
        .with_context(|| format!("invalid global config {}", path.display()))?;
    let public_key =
        TonPublicKey::from_base64(public_key).context("local liteserver public key is invalid")?;

    config.use_local_liteserver(port, public_key);
    write_json_atomic(path, &config)
}

/// Runs election maintenance for every node owned by this join instance.
///
/// A failed tick is isolated to its node and logged; the loop remains alive so a
/// temporary faucet, liteserver, or election-contract failure can recover later.
async fn validation_loop(
    toolchain: Toolchain,
    faucet: Option<String>,
    nodes: BTreeSet<String>,
    interval_seconds: u64,
) -> Result<()> {
    let faucet = faucet
        .as_deref()
        .map(|url| http_url(url, "development faucet"))
        .transpose()?;
    let client = reqwest::Client::new();

    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        for node in &nodes {
            if let Err(error) = validation_tick(&toolchain, &client, faucet.as_ref(), node).await {
                warn!(node, %error, "validator election tick failed");
            }
        }
    }
}

/// Ensures one validator has funding and delegates election state transitions.
///
/// Wallet creation is idempotent. Faucet use is limited to bringing the wallet up
/// to the configured stake plus fee reserve; normal election participation then
/// proceeds entirely through TON contracts and validator-console operations.
async fn validation_tick(
    toolchain: &Toolchain,
    client: &reqwest::Client,
    faucet: Option<&reqwest::Url>,
    node: &str,
) -> Result<()> {
    let settings = toolchain.settings()?;
    let node_settings = settings.node(node)?.clone();
    if !node_settings.validator {
        return Ok(());
    }

    let wallet_name = validator_wallet_name(node);
    let wallet = wallets::ensure_wallet_for_toolchain(
        toolchain,
        &wallet_name,
        WalletVersion::V4r2,
        VALIDATOR_WALLET_WORKCHAIN,
        VALIDATOR_WALLET_ID,
    )
    .await?;

    if settings.validation.auto_participate && node_settings.participate_in_elections {
        let minimum_balance = node_settings
            .validator_stake_nano
            .saturating_add(VALIDATOR_FEE_RESERVE_NANO);
        let balance = wallets::wallet_balance_nano(toolchain, &wallet_name).await?;

        // The faucet is a bootstrap convenience only and is contacted when the
        // persisted wallet cannot cover the configured stake plus fee reserve.
        if balance < u128::from(minimum_balance) {
            let faucet_url = faucet.with_context(|| {
                format!(
                    "validator wallet {} needs at least {minimum_balance} nanotons; fund it or pass --faucet",
                    wallet.address
                )
            })?;

            let grant = client
                .post(faucet_url.clone())
                .json(&serde_json::json!({"address": &wallet.address}))
                .send()
                .await
                .with_context(|| format!("failed to request {faucet_url}"))?
                .error_for_status()
                .with_context(|| format!("development faucet rejected {faucet_url}"))?
                .json::<FaucetGrant>()
                .await
                .context("development faucet returned an invalid grant")?;

            ensure!(
                grant.address == wallet.address,
                "development faucet funded a different address"
            );

            wallets::wait_for_wallet_balance(
                toolchain,
                &wallet.address,
                balance.saturating_add(u128::from(grant.amount_nano)),
            )
            .await?;
            info!(node, wallet = %wallet.address, amount_nano = grant.amount_nano, "validator wallet funded");
        }

        wallets::ensure_wallet_deployed(toolchain, &wallet_name).await?;
    }

    validators::join_auto_tick(toolchain, node, &wallet_name).await
}

fn validator_wallet_name(node: &str) -> String {
    format!("{node}-validator-masterchain")
}

/// Parses a user-supplied HTTP endpoint without accepting local file or custom schemes.
fn http_url(source: &str, label: &str) -> Result<reqwest::Url> {
    let url =
        reqwest::Url::parse(source).with_context(|| format!("invalid {label} URL `{source}`"))?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "{label} URL must use http or https"
    );

    Ok(url)
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use axum::{Json, Router, routing::get};
    use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
    use expect_test::expect;
    use tokio::net::TcpListener;

    use super::*;
    use crate::cli::StateArgs;

    fn global_config_fixture() -> serde_json::Value {
        let block = serde_json::json!({
            "@type": "ton.blockIdExt",
            "workchain": -1,
            "shard": i64::MIN,
            "seqno": 0,
            "root_hash": BASE64.encode([3_u8; 32]),
            "file_hash": BASE64.encode([4_u8; 32]),
        });
        serde_json::json!({
            "@type": "config.global",
            "dht": {
                "@type": "dht.config.global",
                "k": 3,
                "a": 3,
                "static_nodes": {
                    "@type": "dht.nodes",
                    "nodes": [{
                        "@type": "dht.node",
                        "id": {
                            "@type": "pub.ed25519",
                            "key": BASE64.encode([1_u8; 32]),
                        },
                        "addr_list": {
                            "@type": "adnl.addressList",
                            "addrs": [{
                                "@type": "adnl.address.udp",
                                "ip": 2_130_706_433_i32,
                                "port": 6302,
                            }],
                            "version": 0,
                            "reinit_date": 0,
                            "priority": 0,
                            "expire_at": 0,
                        },
                        "version": 0,
                        "signature": BASE64.encode([2_u8; 64]),
                    }],
                },
            },
            "liteservers": [{
                "id": {
                    "@type": "pub.ed25519",
                    "key": BASE64.encode([5_u8; 32]),
                },
                "ip": 1,
                "port": 2,
            }],
            "validator": {
                "@type": "validator.config.global",
                "zero_state": block.clone(),
                "init_block": block,
            },
        })
    }

    #[test]
    fn validator_wallet_is_masterchain_scoped() {
        assert_eq!(VALIDATOR_WALLET_WORKCHAIN, -1);
        assert_eq!(
            validator_wallet_name("node2"),
            "node2-validator-masterchain"
        );
    }

    #[test]
    fn default_joined_node_alias_is_stable_in_its_state_directory() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let layout = Layout::new(root.path().join("join"));
        layout.create_dirs().unwrap();
        let mut settings = Settings::for_join();

        let first = resolve_join_nodes(
            &layout,
            &mut settings,
            &[],
            Some(40_000),
            Ipv4Addr::LOCALHOST,
            false,
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        let second = resolve_join_nodes(
            &layout,
            &mut settings,
            &[],
            None,
            Ipv4Addr::LOCALHOST,
            false,
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

        assert!(first.starts_with("node-"));
        assert_eq!(first.len(), 17);
        assert_eq!(second, first);
    }

    #[test]
    fn local_liteserver_replaces_only_liteserver_entries() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let path = root.path().join("global.config.json");
        let mut expected = global_config_fixture();
        write_json_atomic(&path, &expected).unwrap();

        let local_key = BASE64.encode([6_u8; 32]);
        prefer_local_liteserver(&path, 38_007, &local_key).unwrap();

        let actual: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        expected["liteservers"] = serde_json::json!([{
            "id": {
                "@type": "pub.ed25519",
                "key": local_key,
            },
            "ip": 2_130_706_433_i32,
            "port": 38_007,
        }]);
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn first_run_fetches_standard_global_config_and_configures_a_full_node() {
        let global_config = global_config_fixture();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/global.config.json",
                    get(move || {
                        let global_config = global_config.clone();
                        async move { Json(global_config) }
                    }),
                ),
            )
            .await
            .unwrap();
        });
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let layout = Layout::new(root.path().join("join"));
        layout.create_dirs().unwrap();
        let args = JoinArgs {
            state: StateArgs {
                state_dir: layout.root.clone(),
            },
            nodes: vec!["node2".to_owned()],
            global_config_url: format!("http://{address}/global.config.json"),
            faucet: None,
            advertise_ip: Ipv4Addr::new(10, 0, 0, 2),
            validator: false,
            port_base: Some(41_000),
            ton_bin_dir: None,
            startup_timeout: 1,
        };

        let owned_nodes = prepare_follower_state(&layout, &args).await.unwrap();
        server.abort();

        let settings = Settings::load(&layout.settings).unwrap();
        let node = settings.node("node2").unwrap();
        let node_ports_are_contiguous = [
            node.console_port,
            node.adnl_port,
            node.liteserver_port,
            node.out_port,
            node.dht_port,
        ] == [
            node.console_port,
            node.console_port + 1,
            node.console_port + 2,
            node.console_port + 3,
            node.console_port + 4,
        ];
        let actual = serde_json::json!({
            "owned_nodes": owned_nodes,
            "only_requested_node_is_persisted": settings.nodes.len() == 1,
            "allocation_starts_at_requested_base": node.console_port >= 41_000,
            "node_ports_are_contiguous": node_ports_are_contiguous,
            "enabled": node.enabled,
            "validator": node.validator,
            "participate_in_elections": node.participate_in_elections,
            "advertise_ip": node.public_ip,
            "global_config_is_valid": GlobalConfig::from_json_bytes(
                &std::fs::read(&layout.global_config).unwrap()
            ).is_ok(),
            "zerostate_bundle_downloaded": layout.validator_db.join("static").exists(),
            "private_keys_downloaded": layout.validator_keyring.read_dir().unwrap().next().is_some(),
        });
        expect![[r#"
            {
              "advertise_ip": "10.0.0.2",
              "allocation_starts_at_requested_base": true,
              "enabled": true,
              "global_config_is_valid": true,
              "node_ports_are_contiguous": true,
              "only_requested_node_is_persisted": true,
              "owned_nodes": [
                "node2"
              ],
              "participate_in_elections": false,
              "private_keys_downloaded": false,
              "validator": false,
              "zerostate_bundle_downloaded": false
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&actual).unwrap());

        let mut validator_args = args;
        validator_args.validator = true;
        validator_args.port_base = Some(50_000);
        prepare_follower_state(&layout, &validator_args)
            .await
            .unwrap();
        let promoted = Settings::load(&layout.settings).unwrap();
        let promoted = promoted.node("node2").unwrap();
        assert!(promoted.validator);
        assert!(promoted.participate_in_elections);

        let config_json = layout.node(promoted).config_json();
        std::fs::create_dir_all(config_json.parent().unwrap()).unwrap();
        std::fs::write(&config_json, "{}").unwrap();
        let mut disabled = Settings::load(&layout.settings).unwrap();
        disabled.node_mut("node2").unwrap().participate_in_elections = false;
        disabled.save_atomic(&layout.settings).unwrap();

        prepare_follower_state(&layout, &validator_args)
            .await
            .unwrap();
        let restarted = Settings::load(&layout.settings).unwrap();
        let restarted = restarted.node("node2").unwrap();
        assert!(restarted.validator);
        assert!(!restarted.participate_in_elections);
    }
}
