//! Host-local joiner and supervisor for follower full nodes.
//!
//! On its first run, the agent downloads only public network bootstrap data from
//! the primary Localton launcher. It creates independent databases and keys,
//! starts independent nodes, and keeps private material on this host. With
//! `--validator`, it joins elections through the primary without exporting its
//! validator-engine keys.

use std::{collections::BTreeSet, time::Duration};

use anyhow::{Context, Result, ensure};
use futures_util::StreamExt;
use tokio::select;
use tracing::{info, warn};

use crate::{
    binaries::TonBinaries,
    bootstrap::{LauncherControl, acquire_lock, shutdown_signal, supervise},
    cli::AgentArgs,
    operations::validators::{
        self, ParticipationResult, RemoteValidatorTaskResponse,
    },
    runtime::ProcessRegistry,
    storage::{FullNodeBootstrap, Layout, Settings},
    ton::toolchain::Toolchain,
};

const MAX_BOOTSTRAP_BYTES: u64 = 64 * 1024 * 1024;

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
    let toolchain = Toolchain {
        layout: layout.clone(),
        binaries: binaries.clone(),
    };
    let processes = ProcessRegistry::default();
    let control = LauncherControl::new(
        layout,
        binaries,
        Duration::from_secs(args.startup_timeout),
        processes.clone(),
    );
    for name in &owned_nodes {
        if let Err(error) = control.start_node(name).await {
            if let Err(stop_error) = stop_managed_nodes(&control).await {
                warn!(%stop_error, "failed to stop already started follower nodes");
            }
            return Err(error).context(format!("failed to start follower full node `{name}`"));
        }
    }
    info!(nodes = ?args.nodes, primary = %args.join, validator = args.validator, "localton agent nodes are running");
    let validation_interval = toolchain.settings()?.validation.poll_interval_seconds;
    let validation = remote_validation_loop(
        toolchain,
        args.join.clone(),
        owned_nodes.clone(),
        validation_interval,
        args.validator,
    );
    tokio::pin!(validation);
    let run_result = select! {
        result = supervise(&processes) => result,
        result = shutdown_signal() => result,
        result = &mut validation => result,
    };
    let stop_result = stop_managed_nodes(&control).await;
    run_result.and(stop_result)
}

async fn prepare_follower_state(layout: &Layout, args: &AgentArgs) -> Result<BTreeSet<String>> {
    ensure!(
        !layout.manifest.is_file(),
        "agent requires a separate follower state directory, not a launcher state directory"
    );
    let settings_existed = layout.settings.is_file();
    let mut settings = Settings::load_or_create(&layout.settings)?;
    let mut requested = BTreeSet::new();
    for name in &args.nodes {
        ensure!(name != "genesis", "the agent cannot own the genesis node");
        settings.node(name)?;
        ensure!(
            requested.insert(name.clone()),
            "duplicate agent node `{name}`"
        );
    }
    let static_dir = layout.validator_db.join("static");
    let has_bootstrap = layout.global_config.is_file()
        && static_dir.is_dir()
        && std::fs::read_dir(&static_dir)?.next().is_some();
    if has_bootstrap {
        info!("reusing persisted full-node bootstrap data");
    } else {
        for name in &args.nodes {
            let node = settings.node(name)?;
            ensure!(
                !layout.node(node).config_json().is_file(),
                "agent node `{name}` database exists without complete bootstrap data"
            );
        }
        let bootstrap = fetch_full_node_bootstrap(&args.join).await?;
        bootstrap.install(layout)?;
        info!(primary = %args.join, "installed public full-node bootstrap data");
    }

    for name in &args.nodes {
        let node = settings.node_mut(name)?;
        let node_initialized = layout.node(node).config_json().is_file();
        if settings_existed && node.validator {
            ensure!(
                args.validator,
                "node `{name}` is configured as a validator; restart the agent with --validator"
            );
        }
        if settings_existed && node_initialized {
            ensure!(
                node.public_ip == args.advertise_ip,
                "node `{name}` advertises {}; use the original --advertise-ip {}",
                node.public_ip,
                node.public_ip
            );
        } else {
            node.public_ip = args.advertise_ip;
        }
        node.enabled = true;
        node.validator = args.validator;
        node.participate_in_elections = args.validator;
    }
    settings.validate()?;
    settings.save_atomic(&layout.settings)?;
    Ok(requested)
}

async fn fetch_full_node_bootstrap(primary: &str) -> Result<FullNodeBootstrap> {
    let url = primary_base_url(primary)?
        .join("bootstrap/full-node")
        .context("failed to build full-node bootstrap URL")?;
    let response = reqwest::Client::new()
        .get(url.clone())
        .send()
        .await
        .with_context(|| format!("failed to request {url}"))?
        .error_for_status()
        .with_context(|| format!("primary rejected full-node bootstrap request {url}"))?;
    if let Some(length) = response.content_length() {
        ensure!(
            length <= MAX_BOOTSTRAP_BYTES,
            "full-node bootstrap is too large: {length} bytes"
        );
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed to download full-node bootstrap")?;
        ensure!(
            bytes.len() as u64 + chunk.len() as u64 <= MAX_BOOTSTRAP_BYTES,
            "full-node bootstrap is too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    let bootstrap: FullNodeBootstrap =
        serde_json::from_slice(&bytes).context("full-node bootstrap is invalid JSON")?;
    bootstrap.validate()?;
    Ok(bootstrap)
}

async fn remote_validation_loop(
    toolchain: Toolchain,
    primary: String,
    nodes: BTreeSet<String>,
    interval_seconds: u64,
    enabled: bool,
) -> Result<()> {
    if !enabled {
        return std::future::pending().await;
    }

    let primary = primary_base_url(&primary)?;
    let client = reqwest::Client::new();
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        for node in &nodes {
            if let Err(error) = remote_validation_tick(&toolchain, &client, &primary, node).await {
                warn!(node, %error, "remote validator election tick failed");
            }
        }
    }
}

async fn remote_validation_tick(
    toolchain: &Toolchain,
    client: &reqwest::Client,
    primary: &reqwest::Url,
    node: &str,
) -> Result<()> {
    let task_url = primary
        .join(&format!("validators/{node}/task"))
        .context("failed to build remote validator task URL")?;
    let response = client
        .post(task_url.clone())
        .send()
        .await
        .with_context(|| format!("failed to request {task_url}"))?
        .error_for_status()
        .with_context(|| format!("primary rejected remote validator task {task_url}"))?
        .json::<RemoteValidatorTaskResponse>()
        .await
        .context("primary returned an invalid remote validator task")?;
    let Some(task) = response.task else {
        return Ok(());
    };
    ensure!(
        task.node == node,
        "primary returned task for `{}` to `{node}`",
        task.node
    );
    let entry = validators::prepare_remote_validator_entry(toolchain, &task).await?;
    let participate_url = primary
        .join(&format!("validators/{node}/participate"))
        .context("failed to build remote validator participation URL")?;
    let result = client
        .post(participate_url.clone())
        .json(&entry)
        .send()
        .await
        .with_context(|| format!("failed to submit {participate_url}"))?
        .error_for_status()
        .with_context(|| format!("primary rejected election entry {participate_url}"))?
        .json::<ParticipationResult>()
        .await
        .context("primary returned an invalid participation result")?;
    ensure!(result.node == node, "primary confirmed a different validator");
    validators::mark_remote_validator_submitted(toolchain, node, &result)?;
    info!(node, election_id = result.election_id, "remote validator election entry submitted");
    Ok(())
}

fn primary_base_url(primary: &str) -> Result<reqwest::Url> {
    let mut base = reqwest::Url::parse(primary)
        .with_context(|| format!("invalid primary configuration API URL `{primary}`"))?;
    ensure!(
        matches!(base.scheme(), "http" | "https"),
        "primary URL must use http or https"
    );
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    Ok(base)
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
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use expect_test::expect;
    use tokio::net::TcpListener;

    use super::*;
    use crate::{
        cli::StateArgs,
        storage::{FULL_NODE_BOOTSTRAP_SCHEMA_VERSION, StaticStateFile, TON_RELEASE},
    };

    #[tokio::test]
    async fn first_run_fetches_public_bootstrap_and_configures_a_full_node() {
        let bootstrap = FullNodeBootstrap {
            schema_version: FULL_NODE_BOOTSTRAP_SCHEMA_VERSION,
            ton_release: TON_RELEASE.to_owned(),
            global_config: serde_json::json!({
                "validator": {"zero_state": {"file_hash": "network"}}
            }),
            static_states: vec![StaticStateFile {
                name: "state-hash".to_owned(),
                content_base64: BASE64.encode(b"zerostate"),
            }],
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/bootstrap/full-node",
                    get(move || {
                        let bootstrap = bootstrap.clone();
                        async move { Json(bootstrap) }
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
            join: format!("http://{address}"),
            advertise_ip: Ipv4Addr::new(10, 0, 0, 2),
            validator: false,
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
            "global_config_present": layout.global_config.is_file(),
            "static_state": String::from_utf8(
                std::fs::read(layout.validator_db.join("static/state-hash")).unwrap()
            ).unwrap(),
            "private_keys_downloaded": layout.validator_keyring.read_dir().unwrap().next().is_some(),
        });
        expect![[r#"
            {
              "advertise_ip": "10.0.0.2",
              "enabled": true,
              "global_config_present": true,
              "owned_nodes": [
                "node2"
              ],
              "participate_in_elections": false,
              "private_keys_downloaded": false,
              "static_state": "zerostate",
              "validator": false
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
    }
}
