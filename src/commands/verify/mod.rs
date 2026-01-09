use crate::commands::common::{error_fmt, select_contract, select_wallet};
use crate::wallets::open_wallets;
use acton_config::config::ActonConfig;
use anyhow::{Context, anyhow};
use base64::Engine;
use num_bigint::BigUint;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use ton_api::{Network, StackItem, TonApiClient};
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::coins::{CurrencyCollection, Grams};
use tonlib_core::tlb_types::primitives::either::EitherRef;
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::boc::Boc;

#[allow(clippy::too_many_arguments)]
pub fn verify_cmd(
    contract_id: Option<String>,
    address: Option<String>,
    network: String,
    wallet_name: Option<String>,
    compiler_version: Option<String>,
    dry_run: bool,
    api_key: Option<String>,
) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let contract_key = select_contract(contract_id, &config)?;
    let contract = config
        .get_contract(&contract_key)
        .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, &contract_key)))?;
    let contract_path =
        fs::canonicalize(contract.src.clone()).unwrap_or(PathBuf::from(contract.src.clone()));

    Network::from_str(&network)?; // validate

    println!("  {} Contract: {}", "→".blue().bold(), contract_key.cyan());

    if contract_path.extension() == Some("boc".as_ref()) {
        anyhow::bail!(color_print::cformat!(
            "Cannot verify precompiled <yellow>.boc</> files. Please specify a <yellow>.tolk</> source file."
        ));
    }

    if contract_path.extension() != Some("tolk".as_ref()) {
        anyhow::bail!(color_print::cformat!(
            "Contract source must be a <yellow>.tolk</> file"
        ));
    }

    println!("  {} Compiling contract", "→".blue().bold());
    let compilation_result = tolkc::compile(Path::new(&contract_path), false);

    let code_boc64 = match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            println!("  {} Compiled successfully", "✓".green().bold());
            result.code_boc64
        }
        tolkc::CompilerResult::Error(error) => {
            anyhow::bail!(
                "{}\nFix compilation error first to verify contract",
                error.message
            );
        }
    };

    let code = Boc::decode_base64(&code_boc64)?;
    let code_hash = code.repr_hash();

    println!(
        "  {} Code hash: {}",
        "→".blue().bold(),
        hex::encode(code_hash).dimmed()
    );

    let contract_address = if let Some(addr) = address {
        TonAddress::from_str(&addr).with_context(|| error_fmt::invalid_address(&addr))?
    } else {
        let addr_input = inquire::Text::new("Enter deployed contract address:")
            .prompt()
            .context("Failed to read address")?;
        TonAddress::from_str(&addr_input)
            .with_context(|| error_fmt::invalid_address(&addr_input))?
    };

    println!(
        "  {} Contract address: {}",
        "→".blue().bold(),
        contract_address
            .to_base64_url_flags(true, network == "testnet")
            .dimmed()
    );

    let wallet_name = select_wallet(wallet_name, &config)?;

    let mut wallets = open_wallets(&config, Some(&network), true)?;
    let wallet = wallets
        .remove(&wallet_name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &wallet_name)))?;

    println!(
        "  {} Using wallet: {} {}",
        "→".blue().bold(),
        wallet_name.cyan(),
        wallet
            .wallet
            .address
            .to_base64_url_flags(true, network == "testnet")
            .dimmed()
    );

    println!("  {} Fetching backends configuration", "→".blue().bold());
    let backends_config = get_backends()?;
    let mut backend_info = get_backend_info(&network, &backends_config)?;

    println!(
        "  {} Found {} backend{} for {}",
        "✓".green().bold(),
        backend_info.backends.len(),
        if backend_info.backends.len() == 1 {
            ""
        } else {
            "s"
        },
        network
    );

    if backend_info.backends.is_empty() {
        anyhow::bail!("No backends found for network: {}", network);
    }

    println!("  {} Collecting source files", "→".blue().bold());
    let source_files = abi::get_file_dependencies(contract_path.to_string_lossy().as_ref(), true)?;
    println!(
        "  {} Collected {} source file{}",
        "✓".green().bold(),
        source_files.len(),
        if source_files.len() == 1 { "" } else { "s" }
    );

    if source_files.is_empty() {
        anyhow::bail!("No source files found");
    }

    let mut form = reqwest::blocking::multipart::Form::new();

    for path in source_files.iter() {
        let path = PathBuf::from(path);
        let file_content = fs::read(&path).context("Failed to read source file")?;
        let Some(filename) = path.file_name().and_then(|it| it.to_str()) else {
            anyhow::bail!("Failed to get filename from path: {}", path.display());
        };

        form = form.part(
            filename.to_string(),
            reqwest::blocking::multipart::Part::bytes(file_content).file_name(filename.to_string()),
        );
    }

    let sources_meta: Vec<SourceObject> = source_files
        .iter()
        .enumerate()
        .map(|(idx, path)| SourceObject {
            include_in_command: true,
            is_entrypoint: idx == source_files.len() - 1,
            is_stdlib: false,
            has_include_directives: true,
            folder: PathBuf::from(path)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    // TODO: currently hardcoded to 1.1.0 since Verifier doesn't support 1.2.0 yet
    let version = compiler_version.unwrap_or_else(|| "1.1.0".to_owned());

    let contract_hash = base64::engine::general_purpose::STANDARD.encode(code_hash);
    let sources_object = SourcesObject {
        known_contract_hash: contract_hash.clone(),
        known_contract_address: contract_address.to_base64_url_flags(true, network == "testnet"),
        sender_address: wallet
            .wallet
            .address
            .to_base64_url_flags(true, network == "testnet"),
        sources: sources_meta,
        compiler: CompilerSettings::Tolk {
            compiler_settings: TolkCompilerSettings {
                tolk_version: version,
            },
        },
    };

    let json_str = serde_json::to_string(&sources_object)?;
    form = form.part(
        "json",
        reqwest::blocking::multipart::Part::text(json_str)
            .file_name("blob")
            .mime_str("application/json")?,
    );

    println!(
        "  {} Sending sources to backend for verification",
        "→".blue().bold()
    );

    let client = reqwest::blocking::Client::new();
    let first_backend = remove_random(&mut backend_info.backends);
    let source_url = format!("{}/source", first_backend);

    let response = client
        .post(&source_url)
        .multipart(form)
        .send()
        .context("Failed to send request to verification backend")?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!("Backend compilation failed: {}", error_text);
    }

    let source_result: SourceResponse = response
        .json()
        .context("Failed to parse backend response")?;

    if source_result.compile_result.result != "similar" {
        let error_msg = source_result
            .compile_result
            .error
            .unwrap_or_else(|| "Unknown error".to_string());
        if error_msg == "Contract is already deployed" {
            // This is kinda strange error, trying to show it somehow
            println!(
                "  {}: Contract with the hash {} has already been verified previously, no further action is required\n",
                "Warning".yellow().bold(),
                format!("'{contract_hash}'").dimmed()
            );
            show_verifier_link(&network, contract_address);
            return Ok(());
        }
        anyhow::bail!("Verification failed: {}", error_msg);
    }

    println!("  {} Backend verification successful", "✓".green().bold());

    let mut msg_cell = source_result
        .msg_cell
        .ok_or_else(|| anyhow!("No message cell in response"))?;
    let mut acquired_sigs = 1;

    // TODO: fetch quorum from 'get_verifiers' get method
    let quorum = 1;

    println!(
        "  {} Collecting signatures (need {} of {})",
        "→".blue().bold(),
        quorum,
        backend_info.backends.len() + 1
    );

    while acquired_sigs < quorum && !backend_info.backends.is_empty() {
        let cur_backend = remove_random(&mut backend_info.backends);
        println!(
            "    {} Requesting from: {}",
            "→".dimmed(),
            cur_backend.dimmed()
        );

        let sign_url = format!("{}/sign", cur_backend);
        let sign_request = serde_json::json!({
            "messageCell": msg_cell,
        });

        let response = client
            .post(&sign_url)
            .json(&sign_request)
            .send()
            .context("Failed to send sign request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            println!("    {} Signing failed: {}", "✗".red(), error_text.dimmed());
            continue;
        }

        let sign_result: SignResponse = response.json().context("Failed to parse sign response")?;

        msg_cell = sign_result.msg_cell;
        acquired_sigs += 1;
        println!(
            "    {} Collected signature {}/{}",
            "✓".green(),
            acquired_sigs,
            quorum
        );
    }

    if acquired_sigs < quorum {
        anyhow::bail!(
            "Failed to collect enough signatures ({}/{})",
            acquired_sigs,
            quorum
        );
    }

    println!("  {} All signatures collected", "✓".green().bold());

    if dry_run {
        println!(
            "  {} Dry run mode: skipping transaction send",
            "ℹ".blue().bold()
        );
        println!();
        println!(
            "{}",
            "✓ Contract verification prepared successfully!"
                .green()
                .bold()
        );
        println!("  Message body: {}", hex::encode(&msg_cell.data).dimmed());
        println!();
        println!(
            "Run without {} to send the verification transaction.",
            "--dry-run".yellow()
        );
        return Ok(());
    }

    println!("  {} Sending verification transaction", "→".blue().bold());

    let cell_data = &msg_cell.data;
    let cell = Boc::decode(cell_data)?;
    let cell_boc64 = Boc::encode_base64(&cell);

    let api_client = TonApiClient::new(Network::from_str(&network)?, api_key.clone())?;
    let registry_address = get_verifier_address(&backend_info, &api_client)?;

    wait_for_rate_limit(&api_key);

    let (seqno, need_state_init) = wallet.seqno(&network)?;

    wait_for_rate_limit(&api_key);

    let expired_at_time = std::time::SystemTime::now() + std::time::Duration::from_secs(600);
    let expire_at = expired_at_time
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as u32;

    let body_cell = ArcCell::from_boc_b64(&cell_boc64)?;

    let message_info = tonlib_core::tlb_types::block::message::IntMsgInfo {
        ihr_disabled: true,
        bounce: false,
        bounced: false,
        src: wallet.wallet.address.to_msg_address(),
        dest: registry_address.to_msg_address(),
        value: CurrencyCollection::new(BigUint::from(100_000_000u64)), // 0.1 TON
        ihr_fee: Grams::new(BigUint::from(0u64)),
        fwd_fee: Grams::new(BigUint::from(0u64)),
        created_at: 0,
        created_lt: 0,
    };

    let message = tonlib_core::tlb_types::block::message::Message {
        info: tonlib_core::tlb_types::block::message::CommonMsgInfo::Int(message_info),
        init: None,
        body: EitherRef::new(body_cell),
    };

    let message_cell = message.to_cell()?;

    let external = wallet.wallet.create_external_msg(
        expire_at,
        seqno,
        need_state_init,
        vec![message_cell.to_arc()],
    )?;

    api_client
        .send_boc(&external.to_boc_b64(false)?)
        .context("Failed to send verification transaction")?;

    println!("  {} Transaction sent successfully", "✓".green().bold());
    println!();
    println!("{}", "✓ Contract verification completed!".green().bold());
    show_verifier_link(&network, contract_address);

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TolkCompilerSettings {
    tolk_version: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "compiler")]
enum CompilerSettings {
    #[serde(rename = "tolk")]
    Tolk {
        #[serde(rename = "compilerSettings")]
        compiler_settings: TolkCompilerSettings,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceObject {
    include_in_command: bool,
    is_entrypoint: bool,
    #[serde(rename = "isStdLib")]
    is_stdlib: bool,
    has_include_directives: bool,
    folder: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourcesObject {
    known_contract_hash: String,
    known_contract_address: String,
    sender_address: String,
    sources: Vec<SourceObject>,
    #[serde(flatten)]
    compiler: CompilerSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendsConfig {
    backends: Vec<String>,
    backends_testnet: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CompilerSettingsResponse {
    Object {
        #[serde(rename = "tolkVersion")]
        #[allow(dead_code)]
        tolk_version: String,
    },
    #[allow(dead_code)]
    String(String),
}

#[derive(Debug, Deserialize)]
struct SourceFileInfo {
    #[allow(dead_code)]
    filename: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileResult {
    result: String,
    #[allow(dead_code)]
    compiler_settings: Option<CompilerSettingsResponse>,
    error: Option<String>,
    #[allow(dead_code)]
    hash: Option<String>,
    #[allow(dead_code)]
    sources: Option<Vec<SourceFileInfo>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceResponse {
    compile_result: CompileResult,
    msg_cell: Option<MsgCell>,
}

#[derive(Debug, Deserialize, Serialize)]
struct MsgCell {
    data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignResponse {
    msg_cell: MsgCell,
}

struct BackendInfo {
    source_registry: String,
    backends: Vec<String>,
    #[allow(dead_code)]
    id: String,
}

fn get_backends() -> anyhow::Result<BackendsConfig> {
    let url =
        "https://raw.githubusercontent.com/ton-community/contract-verifier-config/main/config.json";

    let response =
        reqwest::blocking::get(url).context("Failed to fetch verifier backends config")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch backends: HTTP {}", response.status());
    }

    let config: BackendsConfig = response.json().context("Failed to parse backends config")?;

    Ok(config)
}

fn get_backend_info(network: &str, config: &BackendsConfig) -> anyhow::Result<BackendInfo> {
    match network {
        "mainnet" => Ok(BackendInfo {
            source_registry: "EQD-BJSVUJviud_Qv7Ymfd3qzXdrmV525e3YDzWQoHIAiInL".to_string(),
            backends: config.backends.clone(),
            id: "orbs.com".to_string(),
        }),
        "testnet" => Ok(BackendInfo {
            source_registry: "EQCsdKYwUaXkgJkz2l0ol6qT_WxeRbE_wBCwnEybmR0u5TO8".to_string(),
            backends: config.backends_testnet.clone(),
            id: "orbs-testnet".to_string(),
        }),
        _ => anyhow::bail!(
            "Unsupported network: {}. Supported networks: mainnet, testnet",
            network
        ),
    }
}

fn remove_random<T>(els: &mut Vec<T>) -> T {
    let index = (rand::random::<usize>()) % els.len();
    els.remove(index)
}

fn wait_for_rate_limit(api_key: &Option<String>) {
    if api_key.is_none() {
        // rate limit
        println!("  {} Waiting for Toncenter rate limit", "→".blue().bold());
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn show_verifier_link(network: &str, contract_address: TonAddress) {
    println!(
        "View at: {}",
        format!(
            "https://verifier.ton.org/{}{}",
            contract_address.to_base64_url_flags(true, network == "testnet"),
            if network == "testnet" { "?testnet" } else { "" }
        )
        .blue()
    );
}

fn get_verifier_address(
    backend_info: &BackendInfo,
    api_client: &TonApiClient,
) -> anyhow::Result<TonAddress> {
    let result = api_client.run_get_method(
        &backend_info.source_registry,
        "get_verifier_registry_address",
        vec![],
    )?;

    let Some(address_stack) = result.stack.first() else {
        anyhow::bail!("Stack from 'get_verifier_registry_address' is empty")
    };

    let registry_address = if address_stack.len() >= 2 {
        match &address_stack[1] {
            StackItem::Obj(obj) => {
                if let Some(bytes) = obj.get("bytes").and_then(|v| v.as_str()) {
                    let cell = ArcCell::from_boc_b64(bytes)?;
                    let mut slice = cell.parser();
                    slice
                        .load_address()
                        .context("Failed to parse registry address from object")?
                } else {
                    anyhow::bail!("No bytes field in registry address response");
                }
            }
            _ => {
                anyhow::bail!("Unexpected response in registry address stack");
            }
        }
    } else {
        anyhow::bail!("Invalid registry address stack format");
    };

    Ok(registry_address)
}
