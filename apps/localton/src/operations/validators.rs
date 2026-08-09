use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tonutils::tvm::Address;

use crate::{
    cli::{StateArgs, ValidatorCommand},
    operations::wallets,
    storage::Layout,
    storage::RuntimeState,
    storage::{NodeSettings, Settings},
    ton::toolchain::Toolchain,
};

#[derive(Debug, Serialize)]
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
            let names: Vec<_> = settings
                .nodes
                .iter()
                .filter(|node| node.enabled && node.validator && node.participate_in_elections)
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
            let names: Vec<_> = settings
                .nodes
                .iter()
                .filter(|node| node.enabled && node.validator)
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

pub async fn auto_tick(state: StateArgs) -> Result<()> {
    let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
    let settings = toolchain.settings()?;
    let runtime = RuntimeState::load(&toolchain.layout.runtime)?;
    let elector = elector_address(&toolchain.layout)?;
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
        if settings.validation.auto_reap {
            reap(&toolchain, &name).await?;
        }
        let should_participate = election_id > 0
            && settings.node(&name)?.participate_in_elections
            && !runtime.nodes.get(&name).is_some_and(|node| {
                node.election_id == Some(election_id) && node.participation_message.is_some()
            });
        if should_participate {
            participate(&toolchain, &name, Some(election_id)).await?;
        }
    }
    Ok(())
}

async fn print_status(toolchain: &Toolchain) -> Result<()> {
    let settings = toolchain.settings()?;
    let runtime = RuntimeState::load(&toolchain.layout.runtime)?;
    let elector = elector_address(&toolchain.layout)?;
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
    let wallet = wallets::wallet(&toolchain.layout, &wallet_name)?;
    let elector = elector_address(&toolchain.layout)?;
    let election_id = match requested_election_id {
        Some(value) => value,
        None => active_election_id(toolchain, &elector).await?,
    };
    ensure!(election_id > 0, "there is no active election");
    if let Some(runtime) = RuntimeState::load(&toolchain.layout.runtime)?
        .nodes
        .get(&node.name)
        .filter(|runtime| {
            runtime.election_id == Some(election_id)
                && runtime.participation_message.is_some()
                && runtime.validator_public_key.is_some()
                && runtime.validator_adnl.is_some()
        })
    {
        return Ok(ParticipationResult {
            node: node.name,
            election_id,
            election_end: runtime.election_end.unwrap_or_default(),
            validator_public_key: runtime.validator_public_key.clone().unwrap_or_default(),
            validator_adnl: runtime.validator_adnl.clone().unwrap_or_default(),
            message: runtime
                .participation_message
                .as_ref()
                .map_or_else(String::new, |path| path.display().to_string()),
            send_status: None,
        });
    }

    let node_layout = toolchain.layout.node(&node);
    let election_end = election_id.saturating_add(settings.network.election_end_before_seconds);
    let keys = if let Some(keys) = existing_election_keys(&node_layout, election_id)? {
        keys
    } else {
        let signing_key = console_new_key(toolchain, &node).await?;
        console_expect(
            toolchain,
            &node,
            &format!("addpermkey {signing_key} {election_id} {election_end}"),
        )
        .await?;
        console_expect(
            toolchain,
            &node,
            &format!("addtempkey {signing_key} {signing_key} {election_end}"),
        )
        .await?;

        let adnl = console_new_key(toolchain, &node).await?;
        console_expect(toolchain, &node, &format!("addadnl {adnl} 0")).await?;
        console_expect(
            toolchain,
            &node,
            &format!("addvalidatoraddr {signing_key} {adnl} {election_end}"),
        )
        .await?;
        ElectionKeys {
            signing_key,
            adnl,
            election_end,
        }
    };
    let signing_public_key = console_export_public(toolchain, &node, &keys.signing_key).await?;
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
            wallet.address.clone(),
            election_id.to_string(),
            settings.validation.max_factor.to_string(),
            keys.adnl.clone(),
            unsigned.to_string_lossy().into_owned(),
        ],
    )
    .await?;
    let signing_payload = hex::encode(
        fs::read(&unsigned).with_context(|| format!("failed to read {}", unsigned.display()))?,
    );
    let signature = console_sign(toolchain, &node, &keys.signing_key, &signing_payload).await?;
    let signed = request_dir.join("validator-query.boc");
    run_fift(
        toolchain,
        &request_dir,
        "validator-elect-signed.fif",
        vec![
            wallet.address,
            election_id.to_string(),
            settings.validation.max_factor.to_string(),
            keys.adnl.clone(),
            signing_public_key.clone(),
            signature,
            signed.to_string_lossy().into_owned(),
        ],
    )
    .await?;

    let send_status = wallets::send(
        toolchain,
        wallets::SendRequest {
            from: &wallet_name,
            to: &elector,
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
        node_runtime.validator_public_key = Some(signing_public_key.clone());
        node_runtime.validator_adnl = Some(keys.adnl.clone());
        node_runtime.election_id = Some(election_id);
        node_runtime.election_end = Some(keys.election_end);
        node_runtime.participation_message = Some(signed.clone());
        Ok(())
    })?;

    Ok(ParticipationResult {
        node: node.name,
        election_id,
        election_end: keys.election_end,
        validator_public_key: signing_public_key,
        validator_adnl: keys.adnl,
        message: signed.display().to_string(),
        send_status: Some(send_status),
    })
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
    let wallet = wallets::wallet(&toolchain.layout, &wallet_name)?;
    let wallet_address = Address::from_str(&wallet.address)?;
    let elector = elector_address(&toolchain.layout)?;
    let output = toolchain
        .lite_client(&format!(
            "runmethod {elector} compute_returned_stake 0x{}",
            hex::encode(wallet_address.hash_part)
        ))
        .await?;
    let available_nano = parse_first_result_number(&output)?;
    if available_nano == 0 {
        return Ok(ReapResult {
            node: node.name,
            available_nano,
            sent: false,
            send_status: None,
        });
    }

    let reap_dir = toolchain.layout.node(&node).root.join("rewards");
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
            from: &wallet_name,
            to: &elector,
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
        node: node.name,
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

fn elector_address(layout: &Layout) -> Result<String> {
    Ok(wallets::read_address(&layout.zerostate.join("elector.addr"))?.to_raw())
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
            .layout
            .smartcont
            .join(script)
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

    use super::{existing_election_keys, nano_to_grams, parse_first_result_number};
    use crate::storage::{Layout, NodeSettings};

    #[test]
    fn parses_elector_result() {
        assert_eq!(
            parse_first_result_number("result: [ num 10001000000000 ]").unwrap(),
            10_001_000_000_000
        );
    }

    #[test]
    fn formats_nano_without_precision_loss() {
        assert_eq!(nano_to_grams(10_001_000_000_000), "10001");
        assert_eq!(nano_to_grams(1_250_000_001), "1.250000001");
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
