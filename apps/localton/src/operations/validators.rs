use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tonutils::tvm::Address;

use crate::{
    cli::{StateArgs, ValidatorCommand},
    operations::wallets,
    storage::RuntimeState,
    storage::{Layout, NodeSettings, Settings},
    ton::toolchain::Toolchain,
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

#[derive(Debug, Deserialize)]
struct EngineConfig {
    #[serde(default)]
    validators: Vec<EngineValidator>,
}

#[derive(Debug, Deserialize)]
struct EngineValidator {
    id: String,
    election_date: u32,
    expire_at: u32,
    #[serde(default)]
    adnl_addrs: Vec<EngineValidatorAdnl>,
}

#[derive(Debug, Deserialize)]
struct EngineValidatorAdnl {
    id: String,
}

struct ElectionKeys {
    signing_key: String,
    adnl: String,
    election_end: u32,
}

pub async fn execute(command: ValidatorCommand) -> Result<()> {
    match command {
        ValidatorCommand::Status { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            print_status(&toolchain).await
        }
        ValidatorCommand::Enable { state, node } => {
            set_election_mode(&state, &node, true)?;
            println!("validator mode enabled for `{node}`; it will enter future elections");
            Ok(())
        }
        ValidatorCommand::Disable { state, node } => {
            set_election_mode(&state, &node, false)?;
            println!(
                "validator mode disabled for `{node}`; it remains active until the current round ends"
            );
            Ok(())
        }
        ValidatorCommand::Participate {
            state,
            node,
            election_id,
        } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let result = participate(&toolchain, &node, election_id).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        ValidatorCommand::Reap { state, node } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
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

pub(crate) async fn agent_auto_tick(
    toolchain: &Toolchain,
    node_name: &str,
    wallet_name: &str,
) -> Result<()> {
    let settings = toolchain.settings()?;
    let node = settings.node(node_name)?.clone();
    ensure!(node.enabled, "node `{node_name}` is disabled");
    ensure!(node.validator, "node `{node_name}` is not a validator");
    let elector = elector_address(toolchain).await?;
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
                tracing::info!(
                    node = result.node,
                    election_id = result.election_id,
                    "validator election entry submitted directly to Elector"
                );
            }
        }
    }
    if settings.validation.auto_reap {
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
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "elector": elector,
            "active_election_id": active,
            "nodes": runtime.nodes,
        }))?
    );
    for parameter in [15, 17, 32, 34, 36] {
        println!("config {parameter}");
        print!(
            "{}",
            toolchain
                .lite_client(&format!("getconfig {parameter}"))
                .await?
        );
    }
    for node in settings
        .nodes
        .iter()
        .filter(|node| node.enabled && node.validator)
        .filter(|node| runtime.nodes.contains_key(&node.name))
    {
        println!("validator {} stats", node.name);
        print!("{}", toolchain.validator_console(node, "getstats").await?);
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
        let signing_key = console_new_key(toolchain, node).await?;
        console_expect(
            toolchain,
            node,
            &format!("addpermkey {signing_key} {election_id} {election_end}"),
        )
        .await?;
        console_expect(
            toolchain,
            node,
            &format!("addtempkey {signing_key} {signing_key} {election_end}"),
        )
        .await?;

        let adnl = console_new_key(toolchain, node).await?;
        console_expect(toolchain, node, &format!("addadnl {adnl} 0")).await?;
        console_expect(
            toolchain,
            node,
            &format!("addvalidatoraddr {signing_key} {adnl} {election_end}"),
        )
        .await?;
        ElectionKeys {
            signing_key,
            adnl,
            election_end,
        }
    };
    let signing_public_key = console_export_public(toolchain, node, &keys.signing_key).await?;
    RuntimeState::update_atomic(&toolchain.layout.runtime, |runtime| {
        let node_runtime = runtime.nodes.entry(node.name.clone()).or_default();
        node_runtime.validator_public_key = Some(signing_public_key.clone());
        node_runtime.validator_adnl = Some(keys.adnl.clone());
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
            keys.adnl.clone(),
            unsigned.to_string_lossy().into_owned(),
        ],
    )
    .await?;
    let signing_payload = hex::encode(
        fs::read(&unsigned).with_context(|| format!("failed to read {}", unsigned.display()))?,
    );
    let signature = console_sign(toolchain, node, &keys.signing_key, &signing_payload).await?;
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

    let send_status = wallets::send(
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
        node_runtime.validator_public_key = Some(entry.validator_public_key.clone());
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
    let output = toolchain.validator_console(node, "getstats").await?;
    let now = parse_validator_stat(&output, "unixtime")?;
    let masterchain_time = parse_validator_stat(&output, "masterchainblocktime")?;
    Ok(masterchain_time > 0 && now.saturating_sub(masterchain_time) <= MAX_VALIDATOR_LAG_SECONDS)
}

fn parse_validator_stat(output: &str, name: &str) -> Result<u64> {
    output
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            (fields.next() == Some(name)).then(|| fields.next())
        })
        .flatten()
        .with_context(|| format!("validator stats do not contain `{name}`"))?
        .parse()
        .with_context(|| format!("validator stat `{name}` is not a u64"))
}

fn existing_election_keys(
    node_layout: &crate::storage::NodeLayout,
    election_id: u32,
) -> Result<Option<ElectionKeys>> {
    let config_path = node_layout.config_json();
    if !config_path.is_file() {
        return Ok(None);
    }
    let config: EngineConfig = serde_json::from_slice(&fs::read(&config_path)?)
        .with_context(|| format!("failed to parse validator config {}", config_path.display()))?;
    config
        .validators
        .into_iter()
        .find(|validator| validator.election_date == election_id)
        .map(|validator| {
            let adnl = validator
                .adnl_addrs
                .first()
                .context("existing election validator has no ADNL address")?;
            Ok(ElectionKeys {
                signing_key: decode_key_id(&validator.id, "validator key")?,
                adnl: decode_key_id(&adnl.id, "validator ADNL")?,
                election_end: validator.expire_at,
            })
        })
        .transpose()
}

fn decode_key_id(value: &str, label: &str) -> Result<String> {
    let bytes = STANDARD
        .decode(value)
        .with_context(|| format!("{label} is not valid base64"))?;
    ensure!(bytes.len() == 32, "{label} must contain 32 bytes");
    Ok(hex::encode(bytes))
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
    let output = toolchain
        .lite_client(&format!(
            "runmethod {elector} compute_returned_stake 0x{}",
            hex::encode(wallet_address.hash_part)
        ))
        .await?;
    let available_nano = parse_first_result_number(&output)?;
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
    let status = wallets::send(
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
    parse_elector_address(&toolchain.lite_client("getconfig 1").await?)
}

fn parse_elector_address(output: &str) -> Result<String> {
    let expression = Regex::new(r"(?i)elector_addr:x([0-9a-f]{64})")?;
    let hash = expression
        .captures(output)
        .and_then(|captures| captures.get(1))
        .context("config parameter 1 does not contain the Elector address")?;
    Ok(format!("-1:{}", hash.as_str().to_uppercase()))
}

async fn active_election_id(toolchain: &Toolchain, elector: &str) -> Result<u32> {
    let output = toolchain
        .lite_client(&format!("runmethod {elector} active_election_id"))
        .await?;
    u32::try_from(parse_first_result_number(&output)?).context("active election id exceeds u32")
}

fn parse_first_result_number(output: &str) -> Result<u64> {
    let expression = Regex::new(r"(?i)result:\s*\[\s*(?:num\s*)?([0-9]+)")?;
    let value = expression
        .captures(output)
        .and_then(|captures| captures.get(1))
        .context("lite-client output does not contain a numeric result")?;
    value.as_str().parse().context("numeric result exceeds u64")
}

async fn console_new_key(toolchain: &Toolchain, node: &NodeSettings) -> Result<String> {
    let output = toolchain.validator_console(node, "newkey").await?;
    let expression = Regex::new(r"(?i)created new key\s+([0-9a-f]{64})")?;
    expression
        .captures(&output)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_lowercase())
        .context("validator-engine-console did not return a new key id")
}

async fn console_export_public(
    toolchain: &Toolchain,
    node: &NodeSettings,
    key: &str,
) -> Result<String> {
    let output = toolchain
        .validator_console(node, &format!("exportpub {key}"))
        .await?;
    let marker = "got public key:";
    let position = output
        .find(marker)
        .context("validator-engine-console did not return a public key")?;
    let value = output[position + marker.len()..]
        .split_whitespace()
        .next()
        .context("public key value is empty")?;
    STANDARD
        .decode(value)
        .context("validator public key is not valid base64")?;
    Ok(value.to_owned())
}

async fn console_sign(
    toolchain: &Toolchain,
    node: &NodeSettings,
    key: &str,
    payload_hex: &str,
) -> Result<String> {
    let output = toolchain
        .validator_console(node, &format!("sign {key} {payload_hex}"))
        .await?;
    let expression = Regex::new(r"(?i)signature\s+([A-Za-z0-9_+/=-]+)")?;
    let signature = expression
        .captures(&output)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str())
        .context("validator-engine-console did not return a signature")?;
    ensure!(
        STANDARD.decode(signature)?.len() == 64,
        "validator signature must contain 64 bytes"
    );
    Ok(signature.to_owned())
}

async fn console_expect(toolchain: &Toolchain, node: &NodeSettings, command: &str) -> Result<()> {
    let output = toolchain.validator_console(node, command).await?;
    if output.to_ascii_lowercase().contains("failed")
        || output.to_ascii_lowercase().contains("error")
    {
        bail!("validator-engine-console `{command}` failed: {output}")
    }
    Ok(())
}

async fn run_fift(
    toolchain: &Toolchain,
    current_dir: &Path,
    script: &str,
    args: Vec<String>,
) -> Result<()> {
    let mut command = vec![
        "-s".to_owned(),
        toolchain
            .smartcont_script(script)
            .to_string_lossy()
            .into_owned(),
    ];
    command.extend(args);
    let output = tokio::time::timeout(
        Duration::from_secs(60),
        toolchain.fift(current_dir, command),
    )
    .await
    .context("Fift election command timed out")??;
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
        election_key_expiry, existing_election_keys, nano_to_grams, parse_elector_address,
        parse_first_result_number, parse_validator_stat, set_election_mode,
    };
    use crate::cli::StateArgs;
    use crate::storage::{Layout, NodeSettings, Settings};

    #[test]
    fn election_mode_can_change_without_disabling_the_node() {
        let root = tempdir().unwrap();
        let layout = Layout::new(root.path().join("state"));
        layout.create_dirs().unwrap();
        let mut settings = Settings::default();
        let node = settings.node_mut("node2").unwrap();
        node.enabled = true;
        node.validator = false;
        node.participate_in_elections = false;
        settings.save_atomic(&layout.settings).unwrap();
        let state = StateArgs {
            state_dir: layout.root.clone(),
        };

        set_election_mode(&state, "node2", true).unwrap();
        let enabled = Settings::load(&layout.settings).unwrap();
        let enabled = enabled.node("node2").unwrap();
        assert!(enabled.enabled);
        assert!(enabled.validator);
        assert!(enabled.participate_in_elections);

        set_election_mode(&state, "node2", false).unwrap();
        let disabled = Settings::load(&layout.settings).unwrap();
        let disabled = disabled.node("node2").unwrap();
        assert!(disabled.enabled);
        assert!(disabled.validator);
        assert!(!disabled.participate_in_elections);
    }

    #[test]
    fn parses_elector_result() {
        assert_eq!(
            parse_first_result_number("result: [ num 10001000000000 ]").unwrap(),
            10_001_000_000_000
        );
    }

    #[test]
    fn parses_elector_address_from_on_chain_config() {
        let output = "ConfigParam(1) = ( elector_addr:x3333333333333333333333333333333333333333333333333333333333333333)";
        assert_eq!(
            parse_elector_address(output).unwrap(),
            "-1:3333333333333333333333333333333333333333333333333333333333333333"
        );
    }

    #[test]
    fn parses_validator_sync_times() {
        let stats = "unixtime\t\t1787976776\nmasterchainblocktime\t\t1787976774\n";
        assert_eq!(
            parse_validator_stat(stats, "unixtime").unwrap(),
            1_787_976_776
        );
        assert_eq!(
            parse_validator_stat(stats, "masterchainblocktime").unwrap(),
            1_787_976_774
        );
        assert!(parse_validator_stat(stats, "missing").is_err());
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
        let node = NodeSettings::for_index(1);
        let node_layout = layout.node(&node);
        node_layout.create_dirs().unwrap();
        fs::write(
            node_layout.config_json(),
            serde_json::to_vec(&json!({
                "validators": [{
                    "id": STANDARD.encode([0x11; 32]),
                    "election_date": 1234,
                    "expire_at": 1834,
                    "adnl_addrs": [{ "id": STANDARD.encode([0x22; 32]) }]
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let keys = existing_election_keys(&node_layout, 1234).unwrap().unwrap();
        assert_eq!(keys.signing_key, "11".repeat(32));
        assert_eq!(keys.adnl, "22".repeat(32));
        assert_eq!(keys.election_end, 1834);
    }
}
