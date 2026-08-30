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
use serde::Deserialize;
use tokio::select;
use tracing::{info, warn};

use crate::{
    binaries::TonBinaries,
    bootstrap::{NodeController, acquire_lock, shutdown_signal, supervise},
    cli::{JoinArgs, WalletVersion},
    http,
    observability::{ObserverIdentity, SYNC_LAG_TOLERANCE_BLOCKS},
    operations::{validators, wallets},
    runtime::ProcessRegistry,
    storage::{Layout, NodeSettings, RuntimeState, Settings, write_json_atomic},
    ton::{
        global_config::GlobalConfig, lite::LocalLiteClient, toolchain::Toolchain,
        tools::types::TonPublicKey,
    },
};

use self::ports::{HostPortAllocation, DEFAULT_JOIN_PORT_BASE};

const MAX_GLOBAL_CONFIG_BYTES: u64 = 1024 * 1024;
const VALIDATOR_WALLET_WORKCHAIN: i32 = -1;
const VALIDATOR_WALLET_ID: u32 = 42;
const VALIDATOR_FEE_RESERVE_NANO: u64 = 5_000_000_000;
const SYNC_READY_CONFIRMATIONS: usize = 3;
const SYNC_POLL_INTERVAL: Duration = Duration::from_millis(500);
const SYNC_LOG_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
struct FaucetGrant {
    address: String,
    amount_nano: u64,
}

#[derive(Clone)]
struct LocalLiteserver {
    node: String,
    port: u16,
    public_key: String,
}

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
            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                runtime.mark_instance_started();
                for name in &owned_nodes {
                    if let Some(node) = runtime.nodes.get_mut(name) {
                        node.status = "synchronizing".to_owned();
                    }
                }
                Ok(())
            })?;
            let observability_peers = match discover_observability_peer(&args.global_config_url).await {
                Ok(Some(peer)) => vec![peer],
                Ok(None) => Vec::new(),
                Err(error) => {
                    warn!(%error, "could not discover a bootstrap observability peer");
                    Vec::new()
                }
            };
            let services = http::start_observability(
                layout.clone(),
                toolchain.clone(),
                &settings,
                owned_nodes.clone(),
                args.advertise_ip,
                observability_peers,
            )
            .await?;
            for liteserver in &local_liteservers {
                wait_for_network_sync(
                    &layout.global_config,
                    liteserver,
                    Duration::from_secs(args.startup_timeout),
                )
                .await?;
                RuntimeState::update_atomic(&layout.runtime, |runtime| {
                    if let Some(node) = runtime.nodes.get_mut(&liteserver.node) {
                        node.status = "running".to_owned();
                    }
                    Ok(())
                })?;
            }
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
            services.shutdown().await;
            result
        } => result,
        signal_result = shutdown_signal() => signal_result,
    };
    let stop_result = processes.stop_all().await;
    let state_result = RuntimeState::update_atomic(&layout.runtime, |runtime| {
        runtime.mark_instance_stopped();
        Ok(())
    });
    run_result.and(stop_result).and(state_result.map(|_| ()))
}

async fn wait_for_network_sync(
    global_config: &Path,
    liteserver: &LocalLiteserver,
    timeout: Duration,
) -> Result<()> {
    tokio::time::timeout(timeout, async {
        let mut confirmations = 0;
        let mut last_log = Instant::now()
            .checked_sub(SYNC_LOG_INTERVAL)
            .unwrap_or_else(Instant::now);
        loop {
            let sample: Result<(u32, u32)> = async {
                let mut network = LocalLiteClient::connect(global_config).await?;
                let network_head = network.last().await?.seqno;
                let mut local = LocalLiteClient::connect_node(
                    global_config,
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
                            info!(
                                node = liteserver.node,
                                local_head,
                                network_head,
                                lag_blocks = lag,
                                "follower node synchronized"
                            );
                            return;
                        }
                    } else {
                        confirmations = 0;
                    }
                }
                Err(error) => {
                    confirmations = 0;
                    if last_log.elapsed() >= SYNC_LOG_INTERVAL {
                        warn!(node = liteserver.node, %error, "could not measure follower synchronization");
                        last_log = Instant::now();
                    }
                }
            }
            tokio::time::sleep(SYNC_POLL_INTERVAL).await;
        }
    })
    .await
    .with_context(|| {
        format!(
            "node `{}` did not synchronize before the startup timeout",
            liteserver.node
        )
    })
}

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
    if layout.global_config.is_file() {
        info!("reusing persisted TON global config");
    } else {
        for name in &requested {
            let node = settings.node(name)?;
            ensure!(
                !layout.node(node).config_json().is_file(),
                "joining node `{name}` database exists without a global config"
            );
        }
        let global_config = fetch_global_config(&args.global_config_url).await?;
        write_json_atomic(&layout.global_config, &global_config)?;
        info!(url = %args.global_config_url, "installed TON global config");
    }

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
    settings.services.observability.bind = args.observability_bind;
    if args.no_observability {
        settings.services.observability.enabled = false;
    }
    settings.validate()?;
    settings.save_atomic(&layout.settings)?;
    Ok(requested)
}

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
            persisted.iter().cloned().collect::<Vec<_>>().join(" --node ")
        );
        return Ok(persisted);
    }

    let names = if requested.is_empty() {
        let identity =
            ObserverIdentity::load_or_create(&layout.observability.join("identity.json"))?;
        let name = format!("node-{}", &identity.observer_id()[..12]);
        BTreeSet::from([name])
    } else {
        let mut names = BTreeSet::new();
        for name in requested {
            ensure!(name != "genesis", "join cannot own the genesis node");
            ensure!(names.insert(name.clone()), "duplicate joining node `{name}`");
        }
        names
    };

    let allocation = HostPortAllocation::find(
        port_base.unwrap_or(DEFAULT_JOIN_PORT_BASE),
        names.len(),
    )?;
    settings.services.observability.port = allocation.observability;
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
        observability_port = allocation.observability,
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

async fn discover_observability_peer(config_url: &str) -> Result<Option<String>> {
    let mut root = http_url(config_url, "global config")?;
    root.set_path("/");
    root.set_query(None);
    root.set_fragment(None);
    let document = reqwest::Client::new()
        .get(root.clone())
        .send()
        .await
        .with_context(|| format!("failed to request {root}"))?
        .error_for_status()
        .with_context(|| format!("configuration service rejected {root}"))?
        .json::<serde_json::Value>()
        .await
        .context("configuration service returned invalid JSON")?;
    Ok(document
        .pointer("/endpoints/observability")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

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
            observability_bind: Ipv4Addr::UNSPECIFIED,
            port_base: Some(41_000),
            no_observability: false,
            ton_bin_dir: None,
            startup_timeout: 1,
        };

        let owned_nodes = prepare_follower_state(&layout, &args).await.unwrap();
        server.abort();

        let settings = Settings::load(&layout.settings).unwrap();
        let node = settings.node("node2").unwrap();
        let observability_port = settings.services.observability.port;
        let node_ports_follow_observability = [
            node.console_port,
            node.adnl_port,
            node.liteserver_port,
            node.out_port,
            node.dht_port,
        ] == [
            observability_port + 1,
            observability_port + 2,
            observability_port + 3,
            observability_port + 4,
            observability_port + 5,
        ];
        let actual = serde_json::json!({
            "owned_nodes": owned_nodes,
            "only_requested_node_is_persisted": settings.nodes.len() == 1,
            "allocation_starts_at_requested_base": observability_port >= 41_000,
            "node_ports_follow_observability": node_ports_follow_observability,
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
              "node_ports_follow_observability": true,
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
        assert_eq!(promoted.services.observability.port, observability_port);
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
