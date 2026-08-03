//! Validator-engine initialization, configuration, and console operations.
//!
//! Each local validator has an engine database, an ADNL identity, a control
//! console, and optionally a liteserver. This module creates the engine command,
//! installs console and liteserver keys into its generated config, and uses the
//! console to register the permanent genesis-validator identities.

use std::{
    fs,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use serde_json::{Value, json};
use tokio::{process::Command, time::sleep};
use tracing::{info, warn};

use crate::{
    binaries::TonBinaries,
    runtime::{ManagedProcess, run_checked},
    storage::NodeSettings,
    storage::{Layout, write_json_atomic},
};

use super::{engine_config::patch_out_port, keys::GeneratedKey};

const YEAR_SECONDS: u64 = 365 * 24 * 60 * 60;

/// Runs validator-engine once to create its database and generated config.
///
/// The non-persistent command performs initialization and exits. The launcher
/// then verifies the expected config and replaces only `out_port`; control,
/// liteserver, and validator identities are installed in later steps.
pub(super) async fn initialize(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    timeout: Duration,
) -> Result<()> {
    info!("initializing validator-engine database");
    let command = command(layout, binaries, node, false);
    let output = run_checked("validator-engine initialization", command, timeout).await?;
    if !output.stderr.trim().is_empty() {
        warn!(
            stderr = output.stderr.trim(),
            "validator initialization wrote to stderr"
        );
    }
    let config_path = layout.validator_db.join("config.json");
    ensure!(
        config_path.is_file(),
        "validator initialization did not create {}",
        config_path.display()
    );
    patch_out_port(&config_path, node.out_port)?;
    Ok(())
}

/// Adds authenticated control-console and liteserver endpoints to engine config.
///
/// The server key identifies validator-engine itself. The client key receives
/// control permissions and is used by `validator-engine-console`. The separate
/// liteserver key is published to consumers through global config.
pub(super) fn configure_local_services(
    layout: &Layout,
    node: &NodeSettings,
    server: &GeneratedKey,
    client: &GeneratedKey,
    liteserver: &GeneratedKey,
) -> Result<()> {
    let path = layout.node(node).config_json();
    let mut config: Value = serde_json::from_slice(&fs::read(&path)?)
        .with_context(|| format!("invalid validator config {}", path.display()))?;
    let object = config
        .as_object_mut()
        .context("validator config root is not an object")?;
    object.insert(
        "control".to_owned(),
        json!([{
            "id": server.id_base64,
            "port": node.console_port,
            "allowed": [{
                "id": client.id_base64,
                "permissions": 15,
            }],
        }]),
    );
    object.insert(
        "liteservers".to_owned(),
        if node.liteserver {
            json!([{
                "id": liteserver.id_base64,
                "port": node.liteserver_port.to_string(),
            }])
        } else {
            json!([])
        },
    );
    write_json_atomic(&path, &config)
}

/// Registers the identities that let the genesis validator produce blocks.
///
/// These entries live inside validator-engine's private database and can only be
/// changed through its authenticated console. A temporary engine is therefore
/// started during bootstrap, configured, and stopped before the persistent
/// network process is launched.
pub(super) async fn configure_genesis_identity(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    validator: &GeneratedKey,
    timeout: Duration,
) -> Result<()> {
    info!("registering genesis validator keys");
    let mut temporary = ManagedProcess::spawn(
        "temporary validator-engine",
        command(layout, binaries, node, false),
        &layout.logs.join("validator-bootstrap.stdout.log"),
        &layout.logs.join("validator-bootstrap.stderr.log"),
    )?;
    let result = async {
        wait_for_console(layout, binaries, node, &mut temporary, timeout).await?;

        // The full-node ADNL key identifies this node in peer-to-peer traffic.
        // `exportpub` writes its public half where other local tooling can use it.
        let node_key = console_new_key(layout, binaries, node).await?;
        console(layout, binaries, node, &format!("exportpub {node_key}")).await?;

        // Validator ADNL is distinct from full-node ADNL: it associates the
        // permanent validator key with consensus traffic for the configured term.
        let validator_adnl = console_new_key(layout, binaries, node).await?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let end = now + YEAR_SECONDS;

        // Register key roles before importing the external private validator key.
        // The one-year interval avoids election-key expiry during long test runs.
        for remote_command in [
            format!("addpermkey {} 0 {end}", validator.id_hex),
            format!("addtempkey {} {} {end}", validator.id_hex, validator.id_hex),
            format!("addadnl {validator_adnl} 0"),
            format!("addadnl {} 0", validator.id_hex),
            format!(
                "addvalidatoraddr {} {validator_adnl} {end}",
                validator.id_hex
            ),
            format!("addadnl {node_key} 0"),
            format!("changefullnodeaddr {node_key}"),
        ] {
            console_retry(layout, binaries, node, &remote_command).await?;
        }
        import_validator_key(layout, binaries, node, validator, timeout, &mut temporary).await
    }
    .await;
    temporary.stop().await?;
    result
}

/// Imports the generated validator private key with bounded engine restarts.
///
/// `importf` is not reliably accepted by a freshly mutated bootstrap engine on
/// every supported TON build. Restarting reopens the updated keyring/config and
/// makes the retry deterministic while keeping the number of restarts bounded.
async fn import_validator_key(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    validator: &GeneratedKey,
    timeout: Duration,
    temporary: &mut ManagedProcess,
) -> Result<()> {
    let import_command = format!(
        "importf {}",
        layout.validator_keyring.join(&validator.id_hex).display()
    );
    let mut last_error = None;
    for attempt in 1..=3 {
        match console(layout, binaries, node, &import_command).await {
            Ok(_) => return Ok(()),
            Err(error) => {
                warn!(attempt, %error, "validator key import failed; restarting temporary validator");
                last_error = Some(error);
                temporary.stop().await?;
                *temporary = ManagedProcess::spawn(
                    "temporary validator-engine",
                    command(layout, binaries, node, false),
                    &layout.logs.join("validator-bootstrap.stdout.log"),
                    &layout.logs.join("validator-bootstrap.stderr.log"),
                )?;
                wait_for_console(layout, binaries, node, temporary, timeout).await?;
            }
        }
    }
    Err(last_error.context("validator key import failed without an error")?)
}

/// Waits until a live engine answers authenticated console requests.
///
/// Both conditions matter: polling only the socket could miss an engine that
/// exited during initialization, while checking only the child process would not
/// prove that its control endpoint and credentials are usable.
pub(super) async fn wait_for_console(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    process: &mut ManagedProcess,
    timeout: Duration,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(status) = process.try_status()? {
            bail!("temporary validator-engine exited early with {status}");
        }
        match console(layout, binaries, node, "getstats").await {
            Ok(output) if output.contains("conn ready") || output.contains("unixtime") => {
                return Ok(());
            }
            _ if tokio::time::Instant::now() < deadline => {
                sleep(Duration::from_millis(500)).await;
            }
            _ => bail!(
                "validator-engine-console was not ready within {}s",
                timeout.as_secs()
            ),
        }
    }
}

/// Executes one authenticated command against a node's loopback control port.
///
/// The local client private key and engine public key form the control-channel
/// trust pair created during node initialization. Stdout and stderr are combined
/// because different TON builds print successful console data to either stream.
pub(super) async fn console(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    remote_command: &str,
) -> Result<String> {
    let node_layout = layout.node(node);
    let mut command = Command::new(binaries.command("validator-engine-console"));
    command
        .args(["-t", "10", "-k"])
        .arg(node_layout.client_private_key())
        .arg("-p")
        .arg(node_layout.server_public_key())
        .args([
            "-v",
            "0",
            "-a",
            &format!("127.0.0.1:{}", node.console_port),
            "-rc",
            remote_command,
        ]);
    let output = run_checked(
        &format!("validator-engine-console {remote_command}"),
        command,
        Duration::from_secs(15),
    )
    .await?;
    Ok(format!("{}\n{}", output.stdout, output.stderr))
}

/// Retries console mutations that can race engine startup or keyring reloads.
///
/// `changefullnodeaddr` is special: applying it immediately replaces the ADNL
/// identity and intentionally drops the current console connection. Some engine
/// versions report that successful transition as exit code 2, so the matching
/// disconnect text is accepted as success.
pub(super) async fn console_retry(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    remote_command: &str,
) -> Result<String> {
    let mut last_error = None;
    for attempt in 1..=5 {
        match console(layout, binaries, node, remote_command).await {
            Ok(output) => return Ok(output),
            Err(error) => {
                let rendered = error.to_string();
                if remote_command.starts_with("changefullnodeaddr ")
                    && rendered.contains("conn ready")
                {
                    // The engine applies the command and immediately replaces its
                    // full-node ADNL identity. The expected disconnect is reported
                    // by validator-engine-console as exit code 2.
                    return Ok(rendered);
                }
                warn!(attempt, command = remote_command, %error, "validator console command failed; retrying");
                last_error = Some(error);
                sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Err(last_error.context("validator console command failed without an error")?)
}

/// Creates an engine-owned key and extracts the returned 256-bit identifier.
///
/// Console output may contain other hashes and log text; the final hexadecimal
/// identifier is the value produced for the `newkey` command itself.
pub(super) async fn console_new_key(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
) -> Result<String> {
    let output = console(layout, binaries, node, "newkey").await?;
    let regex = Regex::new(r"(?i)\b[0-9a-f]{64}\b")?;
    regex
        .find_iter(&output)
        .last()
        .map(|value| value.as_str().to_ascii_lowercase())
        .context("validator-engine-console newkey returned no key id")
}

/// Builds validator-engine command line for initialization or normal operation.
///
/// Both modes use the same node-specific database, config, logs, and ADNL bind.
/// Persistent mode additionally enables immediate synchronization and applies
/// retention periods for states, blocks, archives, and key proofs. Initialization
/// omits those runtime policies so the binary creates its database and exits.
pub(super) fn command(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    persistent: bool,
) -> Command {
    let node_layout = layout.node(node);
    let mut command = Command::new(binaries.command("validator-engine"));
    command
        .args([
            "--verbosity",
            &node.verbosity.to_string(),
            "--threads",
            &node.threads.to_string(),
            "--global-config",
        ])
        .arg(&node_layout.global_config)
        .arg("--db")
        .arg(&node_layout.db)
        .arg("--logname")
        .arg(node_layout.logs.join(if persistent {
            "validator-engine"
        } else {
            "validator-init"
        }))
        .args(["--session-logs", "", "--celldb-preload-all"]);
    if persistent {
        // A local network has no remote history to catch up with. Start sync
        // immediately and use the explicit retention policy from node settings.
        command.args([
            "--initial-sync-delay",
            "0.0",
            "--sync-before",
            &node.sync_before_seconds.to_string(),
            "--state-ttl",
            &node.state_ttl_seconds.to_string(),
            "--block-ttl",
            &node.block_ttl_seconds.to_string(),
            "--archive-ttl",
            &node.archive_ttl_seconds.to_string(),
            "--key-proof-ttl",
            &node.key_proof_ttl_seconds.to_string(),
        ]);
    }
    command
        .arg("false")
        .args(["--ip", &format!("{}:{}", node.public_ip, node.adnl_port)]);
    command
}
