//! Host-local joiner and supervisor for one follower full node.
//!
//! On its first run, the join workflow downloads a standard TON global config. It
//! creates an independent database and keys, starts the node, and keeps private
//! material on this host. With `--validator`, it can fund a host-local
//! wallet from a development faucet and enters elections directly through the
//! TON Elector contract.

mod ports;

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use indicatif::BinaryBytes;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::select;
use tracing::{info, warn};

use crate::{
    binaries::TonBinaries,
    bootstrap::{acquire_lock, shutdown_signal, supervise},
    cli::{JoinArgs, WalletVersion},
    node,
    operations::{validators, wallets},
    runtime::ProcessRegistry,
    storage::{Layout, NodeRole, NodeSettings, RuntimeState, Settings},
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
    public_key: TonPublicKey,
}

/// Joins an existing TON network and owns one follower for this state directory.
///
/// The workflow prepares durable node state before process startup, waits until its
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
            let settings = prepare_follower_state(&layout, &args).await?;
            let node_settings = &settings.node;
            let node_name = node_settings.name.clone();
            let binaries = TonBinaries::resolve(&layout, args.ton_bin_dir.clone()).await?;
            let toolchain = Toolchain::official(layout.clone(), binaries.clone());
            let startup_timeout = Duration::from_secs(args.startup_timeout);

            // Publish instance ownership before any persistent child starts. Every
            // later error converges on the cleanup boundary outside this block.
            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                runtime.mark_instance_started();
                Ok(())
            })?;

            // Start the host-local node and retain the liteserver identity needed
            // to query it without trusting the remote global-config liteserver list.
            let node_layout = layout.joined_node();
            node::initialize_follower(
                &layout,
                &node_layout,
                &toolchain,
                node_settings,
                startup_timeout,
            )
            .await
            .with_context(|| format!("failed to initialize follower full node `{node_name}`"))?;

            let mut runtime = node::start(
                &layout,
                &node_layout,
                &toolchain,
                node_settings,
                startup_timeout,
                &processes,
            )
                .await
                .with_context(|| format!("failed to start follower full node `{node_name}`"))?;

            let local_liteserver = LocalLiteserver {
                node: node_name.clone(),
                port: node_settings.liteserver_port,
                public_key: runtime
                    .liteserver_public_key
                    .context("joined node has no local liteserver identity")?,
            };

            runtime.begin_synchronization();

            RuntimeState::update_atomic(&layout.runtime, |state| {
                state.node = runtime;
                Ok(())
            })?;

            // A node is usable only after several consecutive near-head samples.
            select! {
                result = wait_for_network_sync(
                    &layout,
                    &toolchain,
                    node_settings,
                    &local_liteserver,
                ) => result?,
                result = supervise(&processes) => return result,
            }

            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                runtime.node.status = "running".to_owned();
                Ok(())
            })?;

            // Once synchronized, all wallet and election work must use a node on
            // this host instead of depending on the bootstrap host's liteserver.
            GlobalConfig::load(&layout.global_config)?
                .with_local_liteserver(local_liteserver.port, local_liteserver.public_key)
                .save_atomic(&layout.global_config)?;

            info!(
                endpoint = %format!("127.0.0.1:{}", local_liteserver.port),
                "join operations now use the synchronized local liteserver"
            );

            info!(
                node = node_name,
                validator = node_settings.validator,
                global_config = %args.global_config_url,
                "joined Localton node is running"
            );

            // Election automation is long-lived and supervised alongside the TON
            // processes so either failure tears down the whole owned instance.
            let validation_interval = toolchain.settings()?.validation.poll_interval_seconds;
            let validation = validation_loop(
                toolchain,
                args.faucet.clone(),
                node_name,
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
    let node_layout = layout.joined_node();
    let console_endpoint = toolchain.validator_console_endpoint(&node_layout, node);

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
                liteserver.public_key,
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
                        runtime.node.observe_sync_progress(local_head, network_head);
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
                                runtime.node.observe_sync_progress(local_head, network_head);
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
                            &console_endpoint,
                        )
                        .await;

                    match stats.and_then(|stats| stats.synchronization()) {
                        Ok(ValidatorSynchronization::BlockTime {
                            block_time,
                            target_time,
                        }) => {
                            if let Err(publish_error) =
                                RuntimeState::update_atomic(&layout.runtime, |runtime| {
                                    runtime
                                        .node
                                        .observe_sync_time_progress(block_time, target_time);
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
                                    runtime.node.observe_initial_sync_progress(progress.clone());
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
/// A saved global config is the network-identity commit marker for join state. A
/// completed node without it is rejected because rebuilding against another network
/// could mix durable validator-engine state and keys.
async fn prepare_follower_state(layout: &Layout, args: &JoinArgs) -> Result<Settings> {
    ensure!(
        !layout.manifest.is_file(),
        "join requires a follower state directory, not a bootstrap state directory"
    );

    let mut settings = if layout.settings.is_file() {
        let settings = Settings::load(&layout.settings)?;
        ensure!(
            settings.node.role == NodeRole::Joined,
            "join state cannot contain the bootstrap genesis node; use a new --state-dir"
        );
        if let Some(requested) = args.node.as_deref() {
            ensure!(
                requested == settings.node.name,
                "joined node name is persisted; restart with --node {} or omit --node",
                settings.node.name
            );
        }

        settings
    } else {
        let name = if let Some(requested) = args.node.as_deref() {
            ensure!(requested != "genesis", "join cannot own the genesis node");
            requested.to_owned()
        } else {
            let digest = Sha256::digest(layout.root.to_string_lossy().as_bytes());
            format!("node-{}", hex::encode(&digest[..6]))
        };
        let allocation =
            HostPortAllocation::find(args.port_base.unwrap_or(DEFAULT_JOIN_PORT_BASE))?;
        let mut node = NodeSettings::follower(name, args.advertise_ip, allocation.node);
        node.validator = args.validator;
        node.participate_in_elections = args.validator;

        info!(
            node = node.name,
            port_range_start = allocation.start,
            port_range_end = allocation.end,
            console_port = node.console_port,
            adnl_port = node.adnl_port,
            liteserver_port = node.liteserver_port,
            out_port = node.out_port,
            dht_port = node.dht_port,
            "allocated persistent join node ports"
        );

        Settings::for_join(node)
    };
    let node_name = settings.node.name.clone();

    // Once downloaded, the global config pins this state directory to one TON
    // network. A restart reuses it even if the source URL later changes.
    let global_config_exists = layout.global_config.is_file();
    let global_config = if global_config_exists {
        info!("reusing persisted TON global config");
        GlobalConfig::load(&layout.global_config)?
    } else {
        ensure!(
            !layout.joined_node().manifest.is_file(),
            "joining node `{node_name}` is initialized without a global config"
        );
        fetch_global_config(&args.global_config_url).await?
    };

    global_config.validate_advertise_ip(args.advertise_ip)?;

    if !global_config_exists {
        global_config.save_atomic(&layout.global_config)?;
        info!(url = %args.global_config_url, "installed TON global config");
    }

    // Initialization-time identity fields become immutable once validator-engine
    // has created its database. Validator participation remains independently
    // mutable through the validator commands.
    let node = &mut settings.node;
    let node_initialized = layout.joined_node().manifest.is_file();
    if node_initialized {
        ensure!(
            node.public_ip == args.advertise_ip,
            "node `{node_name}` advertises {}; use the original --advertise-ip {}",
            node.public_ip,
            node.public_ip
        );
    } else {
        node.public_ip = args.advertise_ip;
        node.validator = args.validator;
        node.participate_in_elections = args.validator;
    }
    node.enabled = true;

    // settings.json is written last so it never advertises a network configuration
    // that failed validation or could not be persisted.
    settings.validate()?;
    settings.save_atomic(&layout.settings)?;

    Ok(settings)
}

/// Downloads and validates a standard TON global configuration.
async fn fetch_global_config(source: &str) -> Result<GlobalConfig> {
    let bytes = reqwest::Client::new()
        .get(source)
        .send()
        .await
        .with_context(|| format!("failed to request global config {source}"))?
        .error_for_status()
        .with_context(|| format!("global config request was rejected by {source}"))?
        .bytes()
        .await
        .context("failed to download global config")?;

    let config = GlobalConfig::from_json_bytes(&bytes).context("global config is invalid")?;
    config.validate_for_node_join()?;

    Ok(config)
}

/// Runs election maintenance for the node owned by this join instance.
///
/// A failed tick is logged and the loop remains alive so a temporary faucet,
/// liteserver, or election-contract failure can recover later.
async fn validation_loop(
    toolchain: Toolchain,
    faucet: Option<String>,
    node: String,
    interval_seconds: u64,
) -> Result<()> {
    let client = reqwest::Client::new();

    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        if let Err(error) = validation_tick(&toolchain, &client, faucet.as_deref(), &node).await {
            warn!(node, %error, "validator election tick failed");
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
    faucet: Option<&str>,
    node: &str,
) -> Result<()> {
    let settings = toolchain.settings()?;
    ensure!(
        settings.node.name == node,
        "validator loop belongs to `{}`, not `{node}`",
        settings.node.name
    );
    let node_settings = settings.node.clone();
    if !node_settings.validator {
        return Ok(());
    }

    let wallet_name = validators::validator_wallet_name(&node_settings);
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
                .post(faucet_url)
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

    validators::join_auto_tick(toolchain, &wallet_name).await
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
        let node = NodeSettings {
            role: NodeRole::Joined,
            name: "node2".to_owned(),
            ..NodeSettings::default()
        };
        assert_eq!(
            validators::validator_wallet_name(&node),
            "node2-validator-masterchain"
        );
    }

    #[test]
    fn local_liteserver_replaces_only_liteserver_entries() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let path = root.path().join("global.config.json");
        let mut expected = global_config_fixture();
        crate::storage::write_json_atomic(&path, &expected).unwrap();

        let local_key = TonPublicKey::from_bytes([6_u8; 32]);
        GlobalConfig::load(&path)
            .unwrap()
            .with_local_liteserver(38_007, local_key)
            .save_atomic(&path)
            .unwrap();

        let actual: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        expected["liteservers"] = serde_json::json!([{
            "id": {
                "@type": "pub.ed25519",
                "key": local_key.to_base64(),
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
            node: Some("node2".to_owned()),
            global_config_url: format!("http://{address}/global.config.json"),
            faucet: None,
            advertise_ip: Ipv4Addr::new(10, 0, 0, 2),
            validator: false,
            port_base: Some(41_000),
            ton_bin_dir: None,
            startup_timeout: 1,
        };

        let prepared = prepare_follower_state(&layout, &args).await.unwrap();
        server.abort();

        let settings = Settings::load(&layout.settings).unwrap();
        assert_eq!(prepared, settings);
        let node = &settings.node;
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
            "node": node.name,
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
              "node": "node2",
              "node_ports_are_contiguous": true,
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
        let promoted = &promoted.node;
        assert!(promoted.validator);
        assert!(promoted.participate_in_elections);

        let node_manifest = layout.joined_node().manifest;
        std::fs::create_dir_all(node_manifest.parent().unwrap()).unwrap();
        std::fs::write(&node_manifest, "{}").unwrap();
        let mut disabled = Settings::load(&layout.settings).unwrap();
        disabled.node.participate_in_elections = false;
        disabled.save_atomic(&layout.settings).unwrap();

        prepare_follower_state(&layout, &validator_args)
            .await
            .unwrap();
        let restarted = Settings::load(&layout.settings).unwrap();
        let restarted = &restarted.node;
        assert!(restarted.validator);
        assert!(!restarted.participate_in_elections);
    }
}
