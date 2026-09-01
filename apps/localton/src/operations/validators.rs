use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, ensure};
use num_bigint::{BigInt, Sign};
use serde::{Deserialize, Serialize};
use tonutils::tvm::Address;

use crate::{
    cli::{StateArgs, ValidatorCommand},
    operations::wallets,
    storage::RuntimeState,
    storage::{Layout, NodeRole, NodeSettings, Settings},
    ton::{
        toolchain::Toolchain,
        tools::{
            lite_client::{ElectionStatus, LiteTarget, RunMethodRequest},
            types::{KeyId, OperationContext, TonPublicKey},
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
    validator_public_key: TonPublicKey,
    validator_adnl: KeyId,
    signature: String,
}

#[derive(Debug, Clone, Serialize)]
struct ParticipationResult {
    node: String,
    election_id: u32,
    election_end: u32,
    validator_public_key: TonPublicKey,
    validator_adnl: KeyId,
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
        ValidatorCommand::Enable { state } => {
            let node = set_election_mode(&state, true)?;
            println!("validator mode enabled for `{node}`; it will enter future elections");
            Ok(())
        }
        ValidatorCommand::Disable { state } => {
            let node = set_election_mode(&state, false)?;
            println!(
                "validator mode disabled for `{node}`; it stops entering elections and remains active until a replacement set is elected"
            );
            Ok(())
        }
        ValidatorCommand::Participate { state, election_id } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let result = participate(&toolchain, election_id).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        ValidatorCommand::Reap { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let result = reap(&toolchain).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
    }
}

pub(crate) async fn election_status(toolchain: &Toolchain) -> Result<ElectionStatus> {
    toolchain
        .lite_client_tool
        .election_status(
            &OperationContext::new(Duration::from_secs(30)),
            &LiteTarget::new(toolchain.lite_config()).with_label("localton"),
        )
        .await
}

fn set_election_mode(state: &StateArgs, enabled: bool) -> Result<String> {
    let layout = Layout::new(crate::ton::toolchain::absolute_path(&state.state_dir)?);
    layout.create_dirs()?;
    let mut settings = Settings::load_or_create(&layout.settings)?;
    let runtime = RuntimeState::load(&layout.runtime)?;
    let node = &mut settings.node;
    ensure!(node.enabled, "node `{}` is disabled", node.name);
    ensure!(
        runtime.node.initialized,
        "node `{}` is not initialized",
        node.name
    );
    if enabled {
        node.validator = true;
    } else {
        ensure!(node.validator, "node `{}` is not a validator", node.name);
    }
    node.participate_in_elections = enabled;
    let node_name = node.name.clone();
    settings.save_atomic(&layout.settings)?;

    Ok(node_name)
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

    let node = &settings.node;
    if !node.enabled || !node.validator {
        return Ok(());
    }

    let should_participate = election_id > 0
        && node.participate_in_elections
        && !(runtime.node.election_id == Some(election_id)
            && runtime.node.participation_message.is_some());
    if should_participate {
        participate(&toolchain, Some(election_id)).await?;
    }
    if settings.validation.auto_reap {
        reap(&toolchain).await?;
    }
    Ok(())
}

/// Advances election participation and stake recovery for one joined validator.
///
/// Submission and reaping are intentionally mutually exclusive in one tick: after
/// an election entry is sent, the next poll can observe its on-chain state before
/// attempting to recover any previously frozen stake.
pub(crate) async fn join_auto_tick(toolchain: &Toolchain, wallet_name: &str) -> Result<()> {
    let settings = toolchain.settings()?;
    let node = settings.node.clone();
    ensure!(node.enabled, "node `{}` is disabled", node.name);
    ensure!(node.validator, "node `{}` is not a validator", node.name);

    let elector = elector_address(toolchain).await?;
    let mut submitted = false;

    if settings.validation.auto_participate {
        let election_id = active_election_id(toolchain, &elector).await?;

        if election_id > 0 && node.participate_in_elections {
            let result = participate_with_wallet(
                toolchain,
                &settings,
                &node,
                wallet_name,
                &elector,
                Some(election_id),
            )
            .await?;

            if result.send_status.is_some() {
                submitted = true;
                tracing::info!(
                    node = result.node,
                    election_id = result.election_id,
                    "validator election entry submitted directly to Elector"
                );
            }
        }
    }

    if settings.validation.auto_reap && !submitted {
        reap_node(toolchain, &node, wallet_name, &elector).await?;
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
            "node": runtime.node,
        }))?
    );
    let node = &settings.node;
    if node.enabled && node.validator && runtime.node.initialized {
        println!("validator {} stats", node.name);
        let stats = toolchain
            .validator_console_tool
            .health(
                &validator_console_context(node),
                &validator_console_endpoint(toolchain, node),
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
    requested_election_id: Option<u32>,
) -> Result<ParticipationResult> {
    let settings = toolchain.settings()?;
    let node = settings.node.clone();
    ensure!(node.enabled, "node `{}` is disabled", node.name);
    ensure!(node.validator, "node `{}` is not a validator", node.name);
    let wallet_name = validator_wallet_name(&node);
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
    let node_layout = &toolchain.layout.node;
    let keys = if let Some(keys) = existing_election_keys(node_layout, election_id)? {
        keys
    } else {
        let context = validator_console_context(node);
        let endpoint = validator_console_endpoint(toolchain, node);
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
    let endpoint = validator_console_endpoint(toolchain, node);
    let signing_public_key = toolchain
        .validator_console_tool
        .export_public(&context, &endpoint, &keys.signing_key)
        .await?;
    RuntimeState::update_atomic(&toolchain.layout.runtime, |runtime| {
        runtime.node.set_validator_public_key(signing_public_key);
        runtime.node.validator_adnl = Some(keys.adnl);
        runtime.node.election_id = Some(election_id);
        runtime.node.election_end = Some(keys.election_end);
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
        validator_adnl: keys.adnl,
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
        .node
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
            entry.validator_adnl.to_hex(),
            entry.validator_public_key.to_tl_base64(),
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
        runtime
            .node
            .set_validator_public_key(entry.validator_public_key);
        runtime.node.validator_adnl = Some(entry.validator_adnl);
        runtime.node.election_id = Some(entry.election_id);
        runtime.node.election_end = Some(entry.election_end);
        runtime.node.participation_message = Some(signed.clone());
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
    let state = RuntimeState::load(&toolchain.layout.runtime)?;
    let runtime = &state.node;
    let (Some(message), Some(validator_public_key), Some(validator_adnl)) = (
        runtime.participation_message.as_ref(),
        runtime.validator_public_key,
        runtime.validator_adnl,
    ) else {
        return Ok(None);
    };
    if runtime.election_id != Some(election_id) {
        return Ok(None);
    }

    Ok(Some(ParticipationResult {
        node: node.name.clone(),
        election_id,
        election_end: runtime.election_end.unwrap_or_default(),
        validator_public_key,
        validator_adnl,
        message: message.display().to_string(),
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
            &validator_console_endpoint(toolchain, node),
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

async fn reap(toolchain: &Toolchain) -> Result<ReapResult> {
    let settings = toolchain.settings()?;
    let node = settings.node.clone();
    ensure!(node.validator, "node `{}` is not a validator", node.name);
    let wallet_name = validator_wallet_name(&node);
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

    let reap_dir = toolchain.layout.node.root.join("rewards");
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
        let total = runtime
            .node
            .total_rewards_nano
            .parse::<u128>()
            .unwrap_or_default()
            .saturating_add(u128::from(available_nano));
        runtime.node.last_reward_nano = available_nano.to_string();
        runtime.node.total_rewards_nano = total.to_string();
        Ok(())
    })?;
    Ok(ReapResult {
        node: node.name.clone(),
        available_nano,
        sent: true,
        send_status: Some(status),
    })
}

/// Returns the wallet name reserved for the one validator owned by a state directory.
pub(crate) fn validator_wallet_name(node: &NodeSettings) -> String {
    match node.role {
        NodeRole::Genesis => "validator".to_owned(),
        NodeRole::Joined => format!("{}-validator-masterchain", node.name),
    }
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
            &LiteTarget::new(toolchain.lite_config()).with_label("localton"),
            RunMethodRequest::new(address, method, arguments)?,
        )
        .await?
        .first_u64()
}

/// Applies the ordinary bound and tracing identity to one console operation
fn validator_console_context(node: &NodeSettings) -> OperationContext {
    OperationContext::for_node(Duration::from_secs(20), &node.name)
}

fn validator_console_endpoint(
    toolchain: &Toolchain,
    node: &NodeSettings,
) -> crate::ton::tools::validator_console::ValidatorConsoleEndpoint {
    toolchain.validator_console_endpoint(&toolchain.layout.node, node)
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
    use std::{fs, net::Ipv4Addr};

    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde_json::json;
    use tempfile::tempdir;

    use super::{election_key_expiry, existing_election_keys, nano_to_grams, set_election_mode};
    use crate::cli::StateArgs;
    use crate::storage::{Layout, NodePorts, NodeSettings, RuntimeState, Settings};

    fn joined_settings() -> Settings {
        Settings::for_join(NodeSettings::joined(
            "node2".to_owned(),
            Ipv4Addr::LOCALHOST,
            NodePorts {
                console: 20_000,
                adnl: 20_001,
                liteserver: 20_002,
                out: 20_003,
                dht: 20_004,
            },
        ))
    }

    #[test]
    fn election_mode_can_change_without_disabling_the_node() {
        let root = tempdir().unwrap();
        let layout = Layout::new(root.path().join("state"));
        layout.create_dirs().unwrap();
        let mut settings = joined_settings();
        let node = &mut settings.node;
        node.enabled = true;
        node.validator = false;
        node.participate_in_elections = false;
        settings.save_atomic(&layout.settings).unwrap();
        let mut runtime = RuntimeState::new();
        runtime.node.initialized = true;
        runtime.save_atomic(&layout.runtime).unwrap();
        let state = StateArgs {
            state_dir: layout.root.clone(),
        };

        set_election_mode(&state, true).unwrap();
        let enabled = Settings::load(&layout.settings).unwrap();
        let enabled = &enabled.node;
        assert!(enabled.enabled);
        assert!(enabled.validator);
        assert!(enabled.participate_in_elections);

        set_election_mode(&state, false).unwrap();
        let disabled = Settings::load(&layout.settings).unwrap();
        let disabled = &disabled.node;
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
        let node_layout = layout.node.clone();
        node_layout.create_dirs().unwrap();
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
