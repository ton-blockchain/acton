//! Host-local joiner and supervisor for follower full nodes.
//!
//! On its first run, the agent downloads a standard TON global config. It
//! creates independent databases and keys, starts independent nodes, and keeps
//! private material on this host. With `--validator`, it can fund a host-local
//! wallet from a development faucet and enters elections directly through the
//! TON Elector contract.

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
    bootstrap::{LauncherControl, acquire_lock, shutdown_signal, supervise},
    cli::{AgentArgs, WalletVersion},
    http,
    observability::{ObserverIdentity, SYNC_LAG_TOLERANCE_BLOCKS},
    operations::{validators, wallets},
    runtime::ProcessRegistry,
    storage::{Layout, RuntimeState, Settings, write_json_atomic},
    ton::{
        global_config::GlobalConfig, lite::LocalLiteClient, toolchain::Toolchain,
        tools::types::TonPublicKey,
    },
};

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

pub async fn run(args: AgentArgs) -> Result<()> {
    std::fs::create_dir_all(&args.state.state_dir).with_context(|| {
        format!(
            "failed to create agent state directory {}",
            args.state.state_dir.display()
        )
    })?;
    let state_root = dunce::canonicalize(&args.state.state_dir).with_context(|| {
        format!(
            "failed to resolve agent state directory {}",
            args.state.state_dir.display()
        )
    })?;
    let layout = Layout::new(state_root);
    layout.create_dirs()?;
    let _state_lock = acquire_lock(&layout.lock)?;
    let owned_nodes = prepare_follower_state(&layout, &args).await?;
    let binaries = TonBinaries::resolve(&layout, args.ton_bin_dir.clone()).await?;
    let toolchain = Toolchain::official(layout.clone(), binaries.clone());
    let processes = ProcessRegistry::default();
    let control = LauncherControl::new(
        layout.clone(),
        toolchain.clone(),
        Duration::from_secs(args.startup_timeout),
        processes.clone(),
    );
    let settings = Settings::load(&layout.settings)?;
    let mut local_liteservers = Vec::new();
    for name in &owned_nodes {
        match control.start_node(name).await {
            Ok(runtime) => {
                if let Some(public_key) = runtime.liteserver_public_key {
                    local_liteservers.push(LocalLiteserver {
                        node: name.clone(),
                        port: settings.node(name)?.liteserver_port,
                        public_key,
                    });
                }
            }
            Err(error) => {
                if let Err(stop_error) = stop_managed_nodes(&control).await {
                    warn!(%stop_error, "failed to stop already started follower nodes");
                }
                return Err(error).context(format!("failed to start follower full node `{name}`"));
            }
        }
    }
    let primary_liteserver = local_liteservers
        .first()
        .cloned()
        .context("agent has no running node with a local liteserver")?;
    RuntimeState::update_atomic(&layout.runtime, |runtime| {
        runtime.mark_launcher_started();
        Ok(())
    })?;
    let observability_peers = match discover_observability_peer(&args.join).await {
        Ok(Some(peer)) => vec![peer],
        Ok(None) => Vec::new(),
        Err(error) => {
            warn!(%error, "could not discover a bootstrap observability peer");
            Vec::new()
        }
    };
    let services = match http::start_observability(
        layout.clone(),
        toolchain.clone(),
        &settings,
        owned_nodes.clone(),
        args.advertise_ip,
        observability_peers,
    )
    .await
    {
        Ok(services) => services,
        Err(error) => {
            if let Err(stop_error) = stop_managed_nodes(&control).await {
                warn!(%stop_error, "failed to stop follower nodes after observability startup failed");
            }
            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                runtime.mark_launcher_stopped();
                Ok(())
            })?;
            return Err(error);
        }
    };
    for liteserver in &local_liteservers {
        if let Err(error) = wait_for_network_sync(
            &layout.global_config,
            liteserver,
            Duration::from_secs(args.startup_timeout),
        )
        .await
        {
            services.shutdown().await;
            if let Err(stop_error) = stop_managed_nodes(&control).await {
                warn!(%stop_error, "failed to stop follower nodes after synchronization failed");
            }
            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                runtime.mark_launcher_stopped();
                Ok(())
            })?;
            return Err(error);
        }
    }
    if let Err(error) = prefer_local_liteserver(
        &layout.global_config,
        primary_liteserver.port,
        &primary_liteserver.public_key,
    ) {
        services.shutdown().await;
        if let Err(stop_error) = stop_managed_nodes(&control).await {
            warn!(%stop_error, "failed to stop follower nodes after liteserver setup failed");
        }
        RuntimeState::update_atomic(&layout.runtime, |runtime| {
            runtime.mark_launcher_stopped();
            Ok(())
        })?;
        return Err(error);
    }
    info!(
        endpoint = %format!("127.0.0.1:{}", primary_liteserver.port),
        "agent chain operations now use its synchronized local liteserver"
    );
    let validator_nodes: Vec<_> = settings
        .nodes
        .iter()
        .filter(|node| owned_nodes.contains(&node.name) && node.validator)
        .map(|node| node.name.as_str())
        .collect();
    info!(nodes = ?owned_nodes, global_config = %args.join, validators = ?validator_nodes, "localton agent nodes are running");
    let validation_interval = toolchain.settings()?.validation.poll_interval_seconds;
    let validation = validation_loop(
        toolchain,
        args.faucet.clone(),
        owned_nodes.clone(),
        validation_interval,
    );
    tokio::pin!(validation);
    let run_result = select! {
        result = supervise(&processes) => result,
        result = shutdown_signal() => result,
        result = &mut validation => result,
    };
    services.shutdown().await;
    let stop_result = stop_managed_nodes(&control).await;
    let state_result = RuntimeState::update_atomic(&layout.runtime, |runtime| {
        runtime.mark_launcher_stopped();
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

async fn prepare_follower_state(layout: &Layout, args: &AgentArgs) -> Result<BTreeSet<String>> {
    ensure!(
        !layout.manifest.is_file(),
        "agent requires a separate follower state directory, not a launcher state directory"
    );
    let settings_existed = layout.settings.is_file();
    let mut settings = Settings::load_or_create(&layout.settings)?;
    let requested = resolve_agent_nodes(layout, &mut settings, &args.nodes)?;
    if layout.global_config.is_file() {
        info!("reusing persisted TON global config");
    } else {
        for name in &requested {
            let node = settings.node(name)?;
            ensure!(
                !layout.node(node).config_json().is_file(),
                "agent node `{name}` database exists without a global config"
            );
        }
        let global_config = fetch_global_config(&args.join).await?;
        write_json_atomic(&layout.global_config, &global_config)?;
        info!(url = %args.join, "installed TON global config");
    }

    for name in &requested {
        let node = settings.node_mut(name)?;
        let node_initialized = layout.node(node).config_json().is_file();
        if settings_existed && node_initialized {
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
    if let Some(port) = args.observability_port {
        settings.services.observability.port = port;
    }
    if args.no_observability {
        settings.services.observability.enabled = false;
    }
    settings.validate()?;
    settings.save_atomic(&layout.settings)?;
    Ok(requested)
}

fn resolve_agent_nodes(
    layout: &Layout,
    settings: &mut Settings,
    requested: &[String],
) -> Result<BTreeSet<String>> {
    if requested.is_empty() {
        let persisted = settings
            .nodes
            .iter()
            .filter(|node| node.name != "genesis" && node.enabled)
            .map(|node| node.name.clone())
            .collect::<BTreeSet<_>>();
        if !persisted.is_empty() {
            return Ok(persisted);
        }

        let identity =
            ObserverIdentity::load_or_create(&layout.observability.join("identity.json"))?;
        let name = format!("node-{}", &identity.observer_id()[..12]);
        let slot = settings
            .nodes
            .iter_mut()
            .find(|node| node.name != "genesis" && !node.enabled)
            .context("agent settings have no free full-node slot")?;
        slot.name = name.clone();
        return Ok(BTreeSet::from([name]));
    }

    let mut names = BTreeSet::new();
    for name in requested {
        ensure!(name != "genesis", "the agent cannot own the genesis node");
        settings.node(name)?;
        ensure!(names.insert(name.clone()), "duplicate agent node `{name}`");
    }
    Ok(names)
}

async fn discover_observability_peer(join: &str) -> Result<Option<String>> {
    let mut root = http_url(join, "global config")?;
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
    validators::agent_auto_tick(toolchain, node, &wallet_name).await
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

async fn stop_managed_nodes(control: &LauncherControl) -> Result<()> {
    let names: Vec<_> = control
        .process_info()
        .await
        .into_iter()
        .map(|process| process.name)
        .collect();
    let mut first_error = None;
    for name in names {
        if let Err(error) = control.stop_node(&name).await
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(())
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
    fn default_agent_alias_is_stable_in_its_state_directory() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let layout = Layout::new(root.path().join("agent"));
        layout.create_dirs().unwrap();
        let mut settings = Settings::default();

        let first = resolve_agent_nodes(&layout, &mut settings, &[])
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        settings.node_mut(&first).unwrap().enabled = true;
        let second = resolve_agent_nodes(&layout, &mut settings, &[])
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
        let layout = Layout::new(root.path().join("agent"));
        layout.create_dirs().unwrap();
        let args = AgentArgs {
            state: StateArgs {
                state_dir: layout.root.clone(),
            },
            nodes: vec!["node2".to_owned()],
            join: format!("http://{address}/global.config.json"),
            faucet: None,
            advertise_ip: Ipv4Addr::new(10, 0, 0, 2),
            validator: false,
            observability_bind: Ipv4Addr::UNSPECIFIED,
            observability_port: None,
            no_observability: false,
            ton_bin_dir: None,
            startup_timeout: 1,
        };

        let owned_nodes = prepare_follower_state(&layout, &args).await.unwrap();
        server.abort();

        let settings = Settings::load(&layout.settings).unwrap();
        let node = settings.node("node2").unwrap();
        let actual = serde_json::json!({
            "owned_nodes": owned_nodes,
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
              "enabled": true,
              "global_config_is_valid": true,
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
