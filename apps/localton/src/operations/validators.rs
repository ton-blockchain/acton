use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, ensure};
use num_bigint::{BigInt, Sign};
use serde::{Deserialize, Serialize};
use tonutils::tvm::Address;

use crate::{
    cli::{StateArgs, ValidatorCommand},
    operations::wallets,
    storage::RuntimeState,
    storage::{Layout, NodeSettings, Settings},
    ton::{
        toolchain::Toolchain,
        tools::{
            lite_client::{ElectionStatus, LiteTarget, RunMethodRequest},
            types::OperationContext,
            validator_console::{
                AddAdnl, AddPermanentKey, AddTemporaryKey, AddValidatorAddress, SignRequest,
            },
            validator_engine_config::{ValidatorElectionKeys, ValidatorEngineConfig},
        },
    },
};

const VALIDATOR_KEY_EXPIRY_MARGIN_SECONDS: u32 = 300;
const MAX_VALIDATOR_LAG_SECONDS: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ElectionEntry {
    election_id: u32,
    election_end: u32,
    validator_public_key: String,
    validator_adnl: String,
    signature: String,
}

#[derive(Debug, Clone, Serialize)]
struct ParticipationResult {
    node: String,
    election_id: u32,
    election_end: u32,
    validator_public_key: String,
    validator_adnl: String,
    message: String,
    send_status: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ReapResult {
    node: String,
    available_nano: u64,
    sent: bool,
    send_status: Option<u32>,
}

pub async fn execute(command: ValidatorCommand) -> Result<()> {
    match command {
        ValidatorCommand::Status { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            print_status(&toolchain).await
        }
        ValidatorCommand::Enable { state, node } => {
            let node = resolve_managed_node(&state, node.as_deref())?;
            set_election_mode(&state, &node, true)?;
            println!("validator mode enabled for `{node}`; it will enter future elections");
            Ok(())
        }
        ValidatorCommand::Disable { state, node } => {
            let node = resolve_managed_node(&state, node.as_deref())?;
            set_election_mode(&state, &node, false)?;
            println!(
                "validator mode disabled for `{node}`; it stops entering elections and remains active until a replacement set is elected"
            );
            Ok(())
        }
        ValidatorCommand::Participate {
            state,
            node,
            election_id,
        } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let node = resolve_managed_node_in_layout(
                &toolchain.layout,
                &toolchain.settings()?,
                node.as_deref(),
            )?;
            let result = participate(&toolchain, &node, election_id).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        ValidatorCommand::Reap { state, node } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let node = resolve_managed_node_in_layout(
                &toolchain.layout,
                &toolchain.settings()?,
                node.as_deref(),
            )?;
            let result = reap(&toolchain, &node).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        ValidatorCommand::ParticipateAll { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let settings = toolchain.settings()?;
            let runtime = RuntimeState::load(&toolchain.layout.runtime)?;
            let names: Vec<_> = settings
                .nodes
                .iter()
                .filter(|node| node.enabled && node.validator && node.participate_in_elections)
                .filter(|node| runtime.nodes.contains_key(&node.name))
                .map(|node| node.name.clone())
                .collect();
            let mut results = Vec::new();
            for name in names {
                results.push(participate(&toolchain, &name, None).await?);
            }
            println!("{}", serde_json::to_string_pretty(&results)?);
            Ok(())
        }
        ValidatorCommand::ReapAll { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let settings = toolchain.settings()?;
            let runtime = RuntimeState::load(&toolchain.layout.runtime)?;
            let names: Vec<_> = settings
                .nodes
                .iter()
                .filter(|node| node.enabled && node.validator)
                .filter(|node| runtime.nodes.contains_key(&node.name))
                .map(|node| node.name.clone())
                .collect();
            let mut results = Vec::new();
            for name in names {
                results.push(reap(&toolchain, &name).await?);
            }
            println!("{}", serde_json::to_string_pretty(&results)?);
            Ok(())
        }
    }
}

pub(crate) async fn election_status(toolchain: &Toolchain) -> Result<ElectionStatus> {
    toolchain
        .lite_client_tool
        .election_status(
            &OperationContext::new(Duration::from_secs(30)),
            &LiteTarget::new(&toolchain.layout.global_config).with_label("localton"),
        )
        .await
}

fn resolve_managed_node(state: &StateArgs, requested: Option<&str>) -> Result<String> {
    let layout = Layout::new(crate::ton::toolchain::absolute_path(&state.state_dir)?);
    layout.create_dirs()?;
    let settings = Settings::load_or_create(&layout.settings)?;
    resolve_managed_node_in_layout(&layout, &settings, requested)
}

fn resolve_managed_node_in_layout(
    layout: &Layout,
    settings: &Settings,
    requested: Option<&str>,
) -> Result<String> {
    let runtime = RuntimeState::load(&layout.runtime)?;
    if let Some(name) = requested {
        settings.node(name)?;
        ensure!(
            runtime.nodes.contains_key(name),
            "node `{name}` is not managed by this localton instance"
        );
        return Ok(name.to_owned());
    }

    let managed = settings
        .nodes
        .iter()
        .filter(|node| node.enabled && runtime.nodes.contains_key(&node.name))
        .map(|node| node.name.clone())
        .collect::<Vec<_>>();
    match managed.as_slice() {
        [name] => Ok(name.clone()),
        [] => anyhow::bail!("this localton instance has no managed nodes"),
        _ => anyhow::bail!(
            "this localton instance manages multiple nodes ({}); specify one explicitly",
            managed.join(", ")
        ),
    }
}

fn set_election_mode(state: &StateArgs, node_name: &str, enabled: bool) -> Result<()> {
    let layout = Layout::new(crate::ton::toolchain::absolute_path(&state.state_dir)?);
    layout.create_dirs()?;
    let mut settings = Settings::load_or_create(&layout.settings)?;
    let node = settings.node_mut(node_name)?;
    ensure!(node.enabled, "node `{node_name}` is disabled");
    if enabled {
        node.validator = true;
    } else {
        ensure!(node.validator, "node `{node_name}` is not a validator");
    }
    node.participate_in_elections = enabled;
    settings.save_atomic(&layout.settings)
}

pub async fn auto_tick(state: StateArgs) -> Result<()> {
    let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
    let settings = toolchain.settings()?;
    let runtime = RuntimeState::load(&toolchain.layout.runtime)?;
    let elector = elector_address(&toolchain).await?;
    let election_id = if settings.validation.auto_participate {
        active_election_id(&toolchain, &elector).await?
    } else {
        0
    };

    let names: Vec<_> = settings
        .nodes
        .iter()
        .filter(|node| node.enabled && node.validator)
        .map(|node| node.name.clone())
        .collect();
    for name in names {
        let should_participate = election_id > 0
            && settings.node(&name)?.participate_in_elections
            && !runtime.nodes.get(&name).is_some_and(|node| {
                node.election_id == Some(election_id) && node.participation_message.is_some()
            });
        if should_participate {
            participate(&toolchain, &name, Some(election_id)).await?;
        }
        if settings.validation.auto_reap {
            reap(&toolchain, &name).await?;
        }
    }
    Ok(())
}

async fn print_status(toolchain: &Toolchain) -> Result<()> {
    let settings = toolchain.settings()?;
    let runtime = RuntimeState::load(&toolchain.layout.runtime)?;
    let elector = elector_address(toolchain).await?;
    let active = active_election_id(toolchain, &elector)
        .await
        .unwrap_or_default();
    let election = election_status(toolchain).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "elector": elector,
            "active_election_id": active,
            "election": election,
            "nodes": runtime.nodes,
        }))?
    );
    for node in settings
        .nodes
        .iter()
        .filter(|node| node.enabled && node.validator)
        .filter(|node| runtime.nodes.contains_key(&node.name))
    {
        println!("validator {} stats", node.name);
        let stats = toolchain
            .validator_console_tool
            .health(
                &validator_console_context(node),
                &toolchain.validator_console_endpoint(node),
            )
            .await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "connection_ready": stats.connection_ready(),
                "unix_time": stats.unix_time()?,
                "masterchain_block_time": stats.masterchain_block_time()?,
            }))?
        );
    }
    Ok(())
}

async fn participate(
    toolchain: &Toolchain,
    node_name: &str,
    requested_election_id: Option<u32>,
) -> Result<ParticipationResult> {
    let settings = toolchain.settings()?;
    let node = settings.node(node_name)?.clone();
    ensure!(node.enabled, "node `{node_name}` is disabled");
    ensure!(node.validator, "node `{node_name}` is not a validator");
    let wallet_name = controlling_wallet_name(&settings, &node)?;
    let elector = elector_address(toolchain).await?;
    participate_with_wallet(
        toolchain,
        &settings,
        &node,
        &wallet_name,
        &elector,
        requested_election_id,
    )
    .await
}

async fn participate_with_wallet(
    toolchain: &Toolchain,
    settings: &Settings,
    node: &NodeSettings,
    wallet_name: &str,
    elector: &str,
    requested_election_id: Option<u32>,
) -> Result<ParticipationResult> {
    let wallet = wallets::wallet(&toolchain.layout, wallet_name)?;
    let election_id = match requested_election_id {
        Some(value) => value,
        None => active_election_id(toolchain, elector).await?,
    };
    ensure!(election_id > 0, "there is no active election");
    if let Some(result) = existing_participation(toolchain, node, election_id)? {
        return Ok(result);
    }
    ensure!(
        node_is_synchronized(toolchain, node).await?,
        "node `{}` is more than {MAX_VALIDATOR_LAG_SECONDS} seconds out of sync",
        node.name
    );

    let election_end = election_key_expiry(settings, election_id);
    let entry = prepare_election_entry(
        toolchain,
        node,
        election_id,
        election_end,
        &wallet.address,
        settings.validation.max_factor,
    )
    .await?;
    submit_election_entry(toolchain, settings, node, wallet_name, elector, entry).await
}

async fn prepare_election_entry(
    toolchain: &Toolchain,
    node: &NodeSettings,
    election_id: u32,
    election_end: u32,
    wallet_address: &str,
    max_factor: f64,
) -> Result<ElectionEntry> {
    let node_layout = toolchain.layout.node(node);
    let keys = if let Some(keys) = existing_election_keys(&node_layout, election_id)? {
        keys
    } else {
        let context = validator_console_context(node);
        let endpoint = toolchain.validator_console_endpoint(node);
        let signing_key = toolchain
            .validator_console_tool
            .new_key(&context, &endpoint)
            .await?;
        toolchain
            .validator_console_tool
            .add_permanent_key(
                &context,
                &endpoint,
                AddPermanentKey {
                    key: signing_key,
                    election_id,
                    expire_at: election_end,
                },
            )
            .await?;
        toolchain
            .validator_console_tool
            .add_temporary_key(
                &context,
                &endpoint,
                AddTemporaryKey {
                    permanent_key: signing_key,
                    temporary_key: signing_key,
                    expire_at: election_end,
                },
            )
            .await?;

        let adnl = toolchain
            .validator_console_tool
            .new_key(&context, &endpoint)
            .await?;
        toolchain
            .validator_console_tool
            .add_adnl(
                &context,
                &endpoint,
                AddAdnl {
                    key: adnl,
                    category: 0,
                },
            )
            .await?;
        toolchain
            .validator_console_tool
            .add_validator_address(
                &context,
                &endpoint,
                AddValidatorAddress {
                    validator_key: signing_key,
                    adnl_key: adnl,
                    expire_at: election_end,
                },
            )
            .await?;
        ValidatorElectionKeys {
            signing_key,
            adnl,
            election_end,
        }
    };
    let context = validator_console_context(node);
    let endpoint = toolchain.validator_console_endpoint(node);
    let signing_public_key = toolchain
        .validator_console_tool
        .export_public(&context, &endpoint, &keys.signing_key)
        .await?
        .into_base64();
    RuntimeState::update_atomic(&toolchain.layout.runtime, |runtime| {
        let node_runtime = runtime.nodes.entry(node.name.clone()).or_default();
        node_runtime.set_validator_public_key(signing_public_key.clone());
        node_runtime.validator_adnl = Some(keys.adnl.to_hex());
        node_runtime.election_id = Some(election_id);
        node_runtime.election_end = Some(keys.election_end);
        Ok(())
    })?;

    let request_dir = node_layout
        .root
        .join("elections")
        .join(election_id.to_string());
    fs::create_dir_all(&request_dir)?;
    let unsigned = request_dir.join("validator-to-sign.bin");
    run_fift(
        toolchain,
        &request_dir,
        "validator-elect-req.fif",
        vec![
            wallet_address.to_owned(),
            election_id.to_string(),
            max_factor.to_string(),
            keys.adnl.to_hex(),
            unsigned.to_string_lossy().into_owned(),
        ],
    )
    .await?;
    let signing_payload =
        fs::read(&unsigned).with_context(|| format!("failed to read {}", unsigned.display()))?;
    let signature = toolchain
        .validator_console_tool
        .sign(
            &context,
            &endpoint,
            SignRequest {
                key: keys.signing_key,
                payload: signing_payload,
            },
        )
        .await?
        .into_base64();
    let entry = ElectionEntry {
        election_id,
        election_end: keys.election_end,
        validator_public_key: signing_public_key,
        validator_adnl: keys.adnl.to_hex(),
        signature,
    };
    fs::write(
        request_dir.join("validator-entry.json"),
        serde_json::to_vec_pretty(&entry)?,
    )?;
    Ok(entry)
}

async fn submit_election_entry(
    toolchain: &Toolchain,
    settings: &Settings,
    node: &NodeSettings,
    wallet_name: &str,
    elector: &str,
    entry: ElectionEntry,
) -> Result<ParticipationResult> {
    let wallet = wallets::wallet(&toolchain.layout, wallet_name)?;
    let request_dir = toolchain
        .layout
        .node(node)
        .root
        .join("elections")
        .join(entry.election_id.to_string());
    fs::create_dir_all(&request_dir)?;
    let signed = request_dir.join("validator-query.boc");
    run_fift(
        toolchain,
        &request_dir,
        "validator-elect-signed.fif",
        vec![
            wallet.address,
            entry.election_id.to_string(),
            settings.validation.max_factor.to_string(),
            entry.validator_adnl.clone(),
            entry.validator_public_key.clone(),
            entry.signature,
            signed.to_string_lossy().into_owned(),
        ],
    )
    .await?;

    let send_status = wallets::send_confirmed(
        toolchain,
        wallets::SendRequest {
            from: wallet_name,
            to: elector,
            amount: &nano_to_grams(node.validator_stake_nano),
            comment: None,
            body: Some(&signed),
            state_init: None,
            mode: 3,
            bounce: true,
        },
    )
    .await?;

    RuntimeState::update_atomic(&toolchain.layout.runtime, |runtime| {
        let node_runtime = runtime.nodes.entry(node.name.clone()).or_default();
        node_runtime.set_validator_public_key(entry.validator_public_key.clone());
        node_runtime.validator_adnl = Some(entry.validator_adnl.clone());
        node_runtime.election_id = Some(entry.election_id);
        node_runtime.election_end = Some(entry.election_end);
        node_runtime.participation_message = Some(signed.clone());
        Ok(())
    })?;

    Ok(ParticipationResult {
        node: node.name.clone(),
        election_id: entry.election_id,
        election_end: entry.election_end,
        validator_public_key: entry.validator_public_key,
        validator_adnl: entry.validator_adnl,
        message: signed.display().to_string(),
        send_status: Some(send_status),
    })
}

fn existing_participation(
    toolchain: &Toolchain,
    node: &NodeSettings,
    election_id: u32,
) -> Result<Option<ParticipationResult>> {
    Ok(RuntimeState::load(&toolchain.layout.runtime)?
        .nodes
        .get(&node.name)
        .filter(|runtime| {
            runtime.election_id == Some(election_id)
                && runtime.participation_message.is_some()
                && runtime.validator_public_key.is_some()
                && runtime.validator_adnl.is_some()
        })
        .map(|runtime| ParticipationResult {
            node: node.name.clone(),
            election_id,
            election_end: runtime.election_end.unwrap_or_default(),
            validator_public_key: runtime.validator_public_key.clone().unwrap_or_default(),
            validator_adnl: runtime.validator_adnl.clone().unwrap_or_default(),
            message: runtime
                .participation_message
                .as_ref()
                .map_or_else(String::new, |path| path.display().to_string()),
            send_status: None,
        }))
}

fn election_key_expiry(settings: &Settings, election_id: u32) -> u32 {
    election_id
        .saturating_add(settings.network.elected_for_seconds)
        .saturating_add(VALIDATOR_KEY_EXPIRY_MARGIN_SECONDS)
}

async fn node_is_synchronized(toolchain: &Toolchain, node: &NodeSettings) -> Result<bool> {
    let stats = toolchain
        .validator_console_tool
        .health(
            &validator_console_context(node),
            &toolchain.validator_console_endpoint(node),
        )
        .await?;
    let now = stats.unix_time()?;
    let Some(masterchain_time) = stats.masterchain_block_time()? else {
        return Ok(false);
    };
    Ok(masterchain_time > 0 && now.saturating_sub(masterchain_time) <= MAX_VALIDATOR_LAG_SECONDS)
}

fn existing_election_keys(
    node_layout: &crate::storage::NodeLayout,
    election_id: u32,
) -> Result<Option<ValidatorElectionKeys>> {
    let config_path = node_layout.config_json();
    if !config_path.is_file() {
        return Ok(None);
    }
    ValidatorEngineConfig::load(&config_path)?.election_keys(election_id)
}

async fn reap(toolchain: &Toolchain, node_name: &str) -> Result<ReapResult> {
    let settings = toolchain.settings()?;
    let node = settings.node(node_name)?.clone();
    ensure!(node.validator, "node `{node_name}` is not a validator");
    let wallet_name = controlling_wallet_name(&settings, &node)?;
    let elector = elector_address(toolchain).await?;
    reap_node(toolchain, &node, &wallet_name, &elector).await
}

async fn reap_node(
    toolchain: &Toolchain,
    node: &NodeSettings,
    wallet_name: &str,
    elector: &str,
) -> Result<ReapResult> {
    let wallet = wallets::wallet(&toolchain.layout, wallet_name)?;
    let wallet_address = Address::from_str(&wallet.address)?;
    let wallet_hash = BigInt::from_bytes_be(Sign::Plus, &wallet_address.hash_part);
    let available_nano = run_method_u64(
        toolchain,
        elector,
        "compute_returned_stake",
        vec![wallet_hash],
    )
    .await?;
    if available_nano == 0 {
        return Ok(ReapResult {
            node: node.name.clone(),
            available_nano,
            sent: false,
            send_status: None,
        });
    }

    let reap_dir = toolchain.layout.node(node).root.join("rewards");
    fs::create_dir_all(&reap_dir)?;
    let message = reap_dir.join(format!("recover-{}.boc", crate::storage::unix_time()));
    run_fift(
        toolchain,
        &reap_dir,
        "recover-stake.fif",
        vec![message.to_string_lossy().into_owned()],
    )
    .await?;
    let status = wallets::send_confirmed(
        toolchain,
        wallets::SendRequest {
            from: wallet_name,
            to: elector,
            amount: "1",
            comment: None,
            body: Some(&message),
            state_init: None,
            mode: 3,
            bounce: true,
        },
    )
    .await?;
    RuntimeState::update_atomic(&toolchain.layout.runtime, |runtime| {
        let node_runtime = runtime.nodes.entry(node.name.clone()).or_default();
        let total = node_runtime
            .total_rewards_nano
            .parse::<u128>()
            .unwrap_or_default()
            .saturating_add(u128::from(available_nano));
        node_runtime.last_reward_nano = available_nano.to_string();
        node_runtime.total_rewards_nano = total.to_string();
        Ok(())
    })?;
    Ok(ReapResult {
        node: node.name.clone(),
        available_nano,
        sent: true,
        send_status: Some(status),
    })
}

fn controlling_wallet_name(settings: &Settings, node: &NodeSettings) -> Result<String> {
    let index = settings
        .nodes
        .iter()
        .position(|candidate| candidate.name == node.name)
        .with_context(|| format!("node `{}` is not in settings", node.name))?;
    Ok(if index == 0 {
        "validator".to_owned()
    } else {
        format!("validator-{index}")
    })
}

async fn elector_address(toolchain: &Toolchain) -> Result<String> {
    Ok(election_status(toolchain).await?.elector_address)
}

async fn active_election_id(toolchain: &Toolchain, elector: &str) -> Result<u32> {
    u32::try_from(run_method_u64(toolchain, elector, "active_election_id", vec![]).await?)
        .context("active election id exceeds u32")
}

/// Runs one integer-only get method through the typed native liteserver adapter.
async fn run_method_u64(
    toolchain: &Toolchain,
    address: &str,
    method: &str,
    arguments: Vec<BigInt>,
) -> Result<u64> {
    toolchain
        .lite_client_tool
        .run_method(
            &OperationContext::new(Duration::from_secs(30)),
            &LiteTarget::new(&toolchain.layout.global_config).with_label("localton"),
            RunMethodRequest::new(address, method, arguments)?,
        )
        .await?
        .first_u64()
}

/// Applies the ordinary bound and tracing identity to one console operation
fn validator_console_context(node: &NodeSettings) -> OperationContext {
    OperationContext::for_node(Duration::from_secs(20), &node.name)
}

async fn run_fift(
    toolchain: &Toolchain,
    current_dir: &Path,
    script: &str,
    args: Vec<String>,
) -> Result<()> {
    let output = toolchain
        .run_fift_script(
            current_dir,
            toolchain.smartcont_script(script),
            args.into_iter().map(Into::into).collect(),
            Duration::from_secs(60),
        )
        .await
        .context("Fift election command failed")?;
    if !output.stdout.is_empty() {
        tracing::debug!("{}", output.stdout.trim());
    }
    Ok(())
}

fn nano_to_grams(nano: u64) -> String {
    let whole = nano / 1_000_000_000;
    let fraction = nano % 1_000_000_000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:09}")
            .trim_end_matches('0')
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        election_key_expiry, existing_election_keys, nano_to_grams, resolve_managed_node_in_layout,
        set_election_mode,
    };
    use crate::cli::StateArgs;
    use crate::storage::{Layout, NodeRuntime, RuntimeState, Settings};

    fn genesis_settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn validator_command_defaults_to_the_locally_managed_node() {
        let root = tempdir().unwrap();
        let layout = Layout::new(root.path().join("state"));
        layout.create_dirs().unwrap();
        let settings = genesis_settings();
        settings.save_atomic(&layout.settings).unwrap();
        let mut runtime = RuntimeState::new();
        runtime
            .nodes
            .insert("genesis".to_owned(), NodeRuntime::default());
        runtime.save_atomic(&layout.runtime).unwrap();

        assert_eq!(
            resolve_managed_node_in_layout(&layout, &settings, None).unwrap(),
            "genesis"
        );
        assert!(resolve_managed_node_in_layout(&layout, &settings, Some("other")).is_err());
    }

    #[test]
    fn election_mode_can_change_without_disabling_the_node() {
        let root = tempdir().unwrap();
        let layout = Layout::new(root.path().join("state"));
        layout.create_dirs().unwrap();
        let mut settings = genesis_settings();
        let node = settings.node_mut("genesis").unwrap();
        node.enabled = true;
        node.validator = false;
        node.participate_in_elections = false;
        settings.save_atomic(&layout.settings).unwrap();
        let state = StateArgs {
            state_dir: layout.root.clone(),
        };

        set_election_mode(&state, "genesis", true).unwrap();
        let enabled = Settings::load(&layout.settings).unwrap();
        let enabled = enabled.node("genesis").unwrap();
        assert!(enabled.enabled);
        assert!(enabled.validator);
        assert!(enabled.participate_in_elections);

        set_election_mode(&state, "genesis", false).unwrap();
        let disabled = Settings::load(&layout.settings).unwrap();
        let disabled = disabled.node("genesis").unwrap();
        assert!(disabled.enabled);
        assert!(disabled.validator);
        assert!(!disabled.participate_in_elections);
    }

    #[test]
    fn formats_nano_without_precision_loss() {
        assert_eq!(nano_to_grams(10_001_000_000_000), "10001");
        assert_eq!(nano_to_grams(1_250_000_001), "1.250000001");
    }

    #[test]
    fn validator_keys_cover_the_full_round_and_margin() {
        let settings = Settings::default();
        assert_eq!(election_key_expiry(&settings, 1_000), 1_420);
    }

    #[test]
    fn recovers_partially_configured_election_keys() {
        let directory = tempdir().unwrap();
        let layout = Layout::new(directory.path().to_owned());
        layout.create_dirs().unwrap();
        let node = genesis_settings().nodes.remove(0);
        let node_layout = layout.node(&node);
        fs::write(
            node_layout.config_json(),
            serde_json::to_vec(&json!({
                "@type": "engine.validator.config",
                "out_port": 3272,
                "addrs": [],
                "adnl": [],
                "dht": [],
                "validators": [{
                    "@type": "engine.validator",
                    "id": STANDARD.encode([0x11; 32]),
                    "temp_keys": [],
                    "election_date": 1234,
                    "expire_at": 1834,
                    "adnl_addrs": [{
                        "@type": "engine.validatorAdnlAddress",
                        "id": STANDARD.encode([0x22; 32]),
                        "expire_at": 1834,
                    }]
                }],
                "collators": [],
                "fullnode": STANDARD.encode([0x33; 32]),
                "fullnodeslaves": [],
                "fullnodemasters": [],
                "liteservers": [],
                "control": [],
                "shards_to_monitor": [],
                "gc": { "@type": "engine.gc", "ids": [] },
            }))
            .unwrap(),
        )
        .unwrap();

        let keys = existing_election_keys(&node_layout, 1234).unwrap().unwrap();
        assert_eq!(keys.signing_key.to_hex(), "11".repeat(32));
        assert_eq!(keys.adnl.to_hex(), "22".repeat(32));
        assert_eq!(keys.election_end, 1834);
    }
}
