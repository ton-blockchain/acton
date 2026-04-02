use crate::commands::common::{error_fmt, select_contract, select_wallet};
use crate::wallets::open_wallets;
use acton_config::color::OwoColorize;
use acton_config::config::ActonConfig;
use anyhow::{Context, anyhow};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_core::types::TonAddress;
use ton_api::{GetMethodResult, Network, TonApiClient};
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::{Boc, BocRepr};
use tycho_types::cell::{Cell, CellSlice, CellSliceParts, HashBytes, Load};
use tycho_types::dict::{Dict, RawDict};
use tycho_types::models::{CurrencyCollection, IntAddr, MsgInfo, OwnedMessage, StdAddr};

const DEFAULT_VERIFIER_ID: &str = "verifier.ton.org";
const MAINNET_SOURCE_REGISTRY: &str = "EQD-BJSVUJviud_Qv7Ymfd3qzXdrmV525e3YDzWQoHIAiInL";
const TESTNET_SOURCE_REGISTRY: &str = "EQCsdKYwUaXkgJkz2l0ol6qT_WxeRbE_wBCwnEybmR0u5TO8";
const MAINNET_VERIFIER_BACKEND: &str = "https://verifier-mainnet.tonstudio.io";
const TESTNET_VERIFIER_BACKEND: &str = "https://verifier-testnet.tonstudio.io";

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
    let contract_path = dunce::canonicalize(contract.src.clone())
        .unwrap_or_else(|_| PathBuf::from(contract.src.clone()));

    let network = Network::from_str(&network)?;
    if !matches!(network, Network::Mainnet | Network::Testnet) {
        anyhow::bail!(
            "Unsupported verification network {network}. Verification backends are available only for mainnet and testnet"
        );
    }

    println!("  {} Contract: {}", "→".blue().bold(), contract_key.cyan());

    if contract_path.extension() == Some("boc".as_ref()) {
        anyhow::bail!(
            "Cannot verify precompiled {} files. Please specify a {} source file.",
            ".boc".yellow(),
            ".tolk".yellow()
        );
    }

    if contract_path.extension() != Some("tolk".as_ref()) {
        anyhow::bail!("Contract source must be a {} file", ".tolk".yellow());
    }

    println!("  {} Compiling contract", "→".blue().bold());
    let compiler = tolkc::Compiler::new(2).with_mappings(&config.mappings);
    let compilation_result = compiler.compile(Path::new(&contract_path), false);

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
        format!("0x{}", hex::encode(code_hash)).dimmed()
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
        format_ton_address(&contract_address, network == Network::Testnet).dimmed()
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
        format_ton_address(&wallet.wallet.address, network == Network::Testnet).dimmed()
    );

    println!("  {} Using built-in verifier backends", "→".blue().bold());
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
        anyhow::bail!("No backends found for network: {network}");
    }

    println!("  {} Collecting source files", "→".blue().bold());
    let source_files = ton_abi::get_file_dependencies(
        contract_path.to_string_lossy().as_ref(),
        true,
        &config.mappings,
    )?;
    println!(
        "  {} Collected {} source file{}",
        "✓".green().bold(),
        source_files.len(),
        if source_files.len() == 1 { "" } else { "s" }
    );

    if source_files.is_empty() {
        anyhow::bail!("No source files found");
    }

    let project_root = acton_config::config::project_root();
    let mut upload_parts: Vec<UploadPart> = Vec::new();
    let mut normalized_source_paths: Vec<String> = Vec::new();

    for path in &source_files {
        let path = PathBuf::from(path);
        let file_content = fs::read(&path).context("Failed to read source file")?;
        let Some(filename) = path.file_name().and_then(|it| it.to_str()) else {
            anyhow::bail!("Failed to get filename from path: {}", path.display());
        };
        let source_path = normalize_source_path_for_verifier(&path, project_root);
        normalized_source_paths.push(source_path.clone());

        upload_parts.push(UploadPart {
            field_name: source_path,
            file_name: filename.to_string(),
            bytes: file_content,
        });
    }

    let sources_meta: Vec<SourceObject> = normalized_source_paths
        .iter()
        .enumerate()
        .map(|(idx, path)| SourceObject {
            include_in_command: true,
            is_entrypoint: idx == normalized_source_paths.len() - 1,
            is_stdlib: false,
            has_include_directives: true,
            folder: source_folder_for_verifier(path),
        })
        .collect();

    let version = compiler_version.unwrap_or_else(|| "1.2.0".to_owned());

    let contract_hash = base64::engine::general_purpose::STANDARD.encode(code_hash);
    let sources_object = SourcesObject {
        known_contract_hash: contract_hash.clone(),
        known_contract_address: format_ton_address(&contract_address, network == Network::Testnet),
        sender_address: format_ton_address(&wallet.wallet.address, network == Network::Testnet),
        sources: sources_meta,
        compiler: CompilerSettings::Tolk {
            compiler_settings: TolkCompilerSettings {
                tolk_version: version.clone(),
            },
        },
    };

    let json_str = serde_json::to_string(&sources_object)?;
    println!(
        "  {} Sending sources to backend for verification",
        "→".blue().bold()
    );

    let sign_client =
        build_verify_http_client().context("Failed to create HTTP client for verifier backend")?;
    let backend_override = std::env::var("ACTON_VERIFY_BACKEND")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty());
    let first_backend = backend_override
        .clone()
        .unwrap_or_else(|| remove_random(&mut backend_info.backends));
    let source_url = format!("{first_backend}/source");
    println!(
        "  {} Using backend: {}",
        "→".blue().bold(),
        source_url.dimmed()
    );

    let verify_debug = std::env::var_os("ACTON_VERIFY_DEBUG").is_some();
    if verify_debug {
        println!(
            "  {} Debug mode enabled via {}",
            "ℹ".blue().bold(),
            "ACTON_VERIFY_DEBUG=1".dimmed()
        );
        println!(
            "    {} Compiler version: {}",
            "→".dimmed(),
            version.dimmed()
        );
        println!(
            "    {} Source file{}:",
            "→".dimmed(),
            if source_files.len() == 1 { "" } else { "s" }
        );
        for file in &source_files {
            println!("      {}", file.dimmed());
        }
        if let Some(backend_override) = &backend_override {
            println!(
                "    {} Backend override: {}",
                "→".dimmed(),
                backend_override.dimmed()
            );
        }
    }

    let source_max_attempts = 8;
    let mut response = None;
    let mut last_send_error = None;

    for attempt in 1..=source_max_attempts {
        let form = build_verify_form(&upload_parts, &json_str)?;
        let source_client = build_verify_http_client()
            .context("Failed to create HTTP client for verifier backend")?;
        match source_client
            .post(&source_url)
            .header(reqwest::header::CONNECTION, "close")
            .multipart(form)
            .send()
        {
            Ok(res) => {
                let status = res.status();
                let http_version = format!("{:?}", res.version());
                let cf_ray = res
                    .headers()
                    .get("cf-ray")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-");
                let should_retry = status.is_server_error() && attempt < source_max_attempts;
                if should_retry {
                    println!(
                        "  {} Backend returned {} ({}, cf-ray={}) on attempt {attempt}/{source_max_attempts}, retrying...",
                        "↻".yellow().bold(),
                        status,
                        http_version,
                        cf_ray
                    );
                    std::thread::sleep(source_retry_delay(attempt));
                    continue;
                }
                response = Some(res);
                break;
            }
            Err(err) => {
                let should_retry = attempt < source_max_attempts;
                if should_retry {
                    println!(
                        "  {} Network error on attempt {attempt}/{source_max_attempts}, retrying...\n    {}",
                        "↻".yellow().bold(),
                        err.to_string().dimmed()
                    );
                    last_send_error = Some(err);
                    std::thread::sleep(source_retry_delay(attempt));
                    continue;
                }
                return Err(err).context("Failed to send request to verification backend");
            }
        }
    }

    let response = response.ok_or_else(|| {
        if let Some(err) = last_send_error {
            anyhow!("Failed to send request to verification backend: {err}")
        } else {
            anyhow!("Failed to get response from verification backend")
        }
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let http_version = format!("{:?}", response.version());
        let headers = response.headers().clone();
        let error_text = response
            .text()
            .unwrap_or_else(|_| "Unknown error".to_string());

        let mut header_parts = Vec::new();
        if let Some(v) = headers.get("x-request-id").and_then(|v| v.to_str().ok()) {
            header_parts.push(format!("x-request-id={v}"));
        }
        if let Some(v) = headers.get("cf-ray").and_then(|v| v.to_str().ok()) {
            header_parts.push(format!("cf-ray={v}"));
        }
        if let Some(v) = headers.get("server").and_then(|v| v.to_str().ok()) {
            header_parts.push(format!("server={v}"));
        }

        let header_suffix = if header_parts.is_empty() {
            String::new()
        } else {
            format!("\nResponse headers: {}", header_parts.join(", "))
        };

        let retry_hint = if status.is_server_error() {
            "\nHint: backend returned a server error (5xx). Retry later; if it persists, run with ACTON_VERIFY_DEBUG=1 and/or ACTON_VERIFY_BACKEND=https://... to test another endpoint."
        } else {
            "\nHint: run with ACTON_VERIFY_DEBUG=1 to print request details."
        };

        let body = truncate_for_display(&error_text, 4_000);
        anyhow::bail!(
            "Backend compilation failed: HTTP {} ({}) at {}{}{}\nResponse body:\n{}",
            status,
            http_version,
            source_url,
            header_suffix,
            retry_hint,
            body
        );
    }

    let source_result: SourceResponse = response
        .json()
        .context("Failed to parse backend response")?;

    if source_result.compile_result.result != "similar" {
        let error_msg = source_result
            .compile_result
            .error
            .unwrap_or_else(|| "Unknown error".to_string());
        if error_msg == "Proof has already been deployed" {
            // This is kinda strange error, trying to show it somehow
            println!(
                "\n  {}: Contract with the hash {} has already been verified previously, no further action is required\n",
                "Warning".yellow().bold(),
                contract_hash.dimmed()
            );
            show_verifier_link(&network, contract_address);
            return Ok(());
        }
        anyhow::bail!("Verification failed: {error_msg}");
    }

    println!("  {} Backend verification successful", "✓".green().bold());

    let config = ActonConfig::load().unwrap_or_default();
    let custom_networks = config.custom_networks();
    let is_testnet = network == Network::Testnet;
    let api_client = TonApiClient::new(network.clone(), custom_networks, api_key.clone())?;

    wait_for_rate_limit(&api_key);
    let registry_address = get_verifier_address(&backend_info, &api_client)?;

    wait_for_rate_limit(&api_key);
    let quorum = usize::from(get_verifier_quorum(
        &api_client,
        &registry_address,
        &backend_info.id,
        is_testnet,
    )?);

    let mut msg_cell = source_result
        .msg_cell
        .ok_or_else(|| anyhow!("No message cell in response"))?;
    let mut acquired_sigs = 1usize;

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

        let sign_url = format!("{cur_backend}/sign");
        let sign_request = serde_json::json!({
            "messageCell": msg_cell,
        });

        let response = sign_client
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
        anyhow::bail!("Failed to collect enough signatures ({acquired_sigs}/{quorum})");
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
    let body_cell = Boc::decode(cell_data)?;

    wait_for_rate_limit(&api_key);

    let (seqno, need_state_init) = wallet.seqno(&api_client)?;

    wait_for_rate_limit(&api_key);

    let expired_at_time = std::time::SystemTime::now() + std::time::Duration::from_secs(600);
    let expire_at = expired_at_time
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as u32;

    let message_info = tycho_types::models::IntMsgInfo {
        ihr_disabled: true,
        bounce: false,
        bounced: false,
        src: IntAddr::Std(wallet.address()),
        dst: IntAddr::Std(ton_address_to_std_addr(&registry_address)),
        value: CurrencyCollection::new(100_000_000u128), // 0.1 TON
        ihr_fee: Default::default(),
        fwd_fee: Default::default(),
        created_lt: 0,
        created_at: 0,
    };

    let message = OwnedMessage {
        info: MsgInfo::Int(message_info),
        init: None,
        body: CellSliceParts::from(body_cell),
        layout: None,
    };

    let message_cell_boc = BocRepr::encode(message)?;
    let message_cell = TonCell::from_boc(message_cell_boc)?;

    let external =
        wallet
            .wallet
            .create_ext_in_msg(vec![message_cell], seqno, expire_at, need_state_init)?;

    api_client
        .send_boc(&external.to_boc_base64()?)
        .map_err(|error| anyhow!("Failed to send verification transaction: {error}"))?;

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
    #[serde(default)]
    verifiers: Vec<VerifierBackends>,
    #[serde(default)]
    backends: Vec<String>,
    #[serde(default)]
    backends_testnet: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerifierBackends {
    id: String,
    network: String,
    backends: Vec<String>,
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

#[derive(Debug)]
struct BackendInfo {
    source_registry: String,
    backends: Vec<String>,
    #[allow(dead_code)]
    id: String,
}

#[derive(Debug, Clone)]
struct UploadPart {
    field_name: String,
    file_name: String,
    bytes: Vec<u8>,
}

fn get_backends() -> anyhow::Result<BackendsConfig> {
    Ok(BackendsConfig {
        verifiers: vec![
            VerifierBackends {
                id: DEFAULT_VERIFIER_ID.to_string(),
                network: "mainnet".to_string(),
                backends: vec![MAINNET_VERIFIER_BACKEND.to_string()],
            },
            VerifierBackends {
                id: DEFAULT_VERIFIER_ID.to_string(),
                network: "testnet".to_string(),
                backends: vec![TESTNET_VERIFIER_BACKEND.to_string()],
            },
        ],
        backends: Vec::new(),
        backends_testnet: Vec::new(),
    })
}

fn get_backend_info(network: &Network, config: &BackendsConfig) -> anyhow::Result<BackendInfo> {
    let network_name = match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
        _ => {
            anyhow::bail!("Unsupported network: {network}. Supported networks: mainnet, testnet")
        }
    };

    // New config style:
    // {
    //   "verifiers": [{ "id": "...", "network": "...", "backends": [...] }]
    // }
    // Prefer TON Verifier entries first, fallback to legacy root fields.
    let mut resolved_backends: Vec<String> = config
        .verifiers
        .iter()
        .filter(|entry| {
            entry.id == DEFAULT_VERIFIER_ID && entry.network.eq_ignore_ascii_case(network_name)
        })
        .flat_map(|entry| entry.backends.clone())
        .collect();

    if resolved_backends.is_empty() {
        resolved_backends = match network {
            Network::Mainnet => config.backends.clone(),
            Network::Testnet => config.backends_testnet.clone(),
            _ => unreachable!("network variants are checked above"),
        };
    }

    match network {
        Network::Mainnet => Ok(BackendInfo {
            source_registry: MAINNET_SOURCE_REGISTRY.to_string(),
            backends: resolved_backends,
            id: DEFAULT_VERIFIER_ID.to_string(),
        }),
        Network::Testnet => Ok(BackendInfo {
            source_registry: TESTNET_SOURCE_REGISTRY.to_string(),
            backends: resolved_backends,
            id: DEFAULT_VERIFIER_ID.to_string(),
        }),
        _ => anyhow::bail!("Unsupported network: {network}. Supported networks: mainnet, testnet"),
    }
}

fn remove_random<T>(els: &mut Vec<T>) -> T {
    let index = (rand::random::<usize>()) % els.len();
    els.remove(index)
}

fn build_verify_http_client() -> anyhow::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .pool_max_idle_per_host(0)
        .build()
        .context("Failed to build verifier HTTP client")
}

fn build_verify_form(
    parts: &[UploadPart],
    json_str: &str,
) -> anyhow::Result<reqwest::blocking::multipart::Form> {
    let mut form = reqwest::blocking::multipart::Form::new().percent_encode_noop();

    for part in parts {
        form = form.part(
            part.field_name.clone(),
            reqwest::blocking::multipart::Part::bytes(part.bytes.clone())
                .file_name(part.file_name.clone())
                .mime_str("application/octet-stream")?,
        );
    }

    form = form.part(
        "json",
        reqwest::blocking::multipart::Part::text(json_str.to_owned())
            .file_name("blob")
            .mime_str("application/json")?,
    );

    Ok(form)
}

fn source_retry_delay(attempt: usize) -> std::time::Duration {
    let secs = (attempt as u64).min(10);
    std::time::Duration::from_secs(secs)
}

fn format_ton_address(address: &TonAddress, is_testnet: bool) -> String {
    address.to_base64(!is_testnet, false, true)
}

fn ton_address_to_std_addr(address: &TonAddress) -> StdAddr {
    StdAddr {
        anycast: None,
        address: HashBytes(
            <[u8; 32]>::try_from(address.hash.as_slice())
                .expect("TonAddress hash must be exactly 32 bytes"),
        ),
        workchain: address.workchain as i8,
    }
}

fn normalize_source_path_for_verifier(path: &Path, project_root: &Path) -> String {
    let relative = path.strip_prefix(project_root).unwrap_or(path);
    relative.to_string_lossy().replace('\\', "/")
}

fn source_folder_for_verifier(path: &str) -> String {
    let folder = Path::new(path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_string_lossy()
        .replace('\\', "/");

    if folder.is_empty() {
        ".".to_string()
    } else {
        folder
    }
}

fn truncate_for_display(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }

    let mut out = String::with_capacity(max_chars + 64);
    out.extend(text.chars().take(max_chars));
    out.push_str(&format!("\n... (truncated, total {total} chars)"));
    out
}

fn wait_for_rate_limit(api_key: &Option<String>) {
    if api_key.is_none() {
        // rate limit
        println!("  {} Waiting for Toncenter rate limit", "→".blue().bold());
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn show_verifier_link(network: &Network, contract_address: TonAddress) {
    let is_testnet = network == &Network::Testnet;
    println!(
        "View at: {}",
        format!(
            "https://verifier.ton.org/{}{}",
            format_ton_address(&contract_address, is_testnet),
            if is_testnet { "?testnet" } else { "" }
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
        &[],
    )?;

    parse_verifier_registry_address(&result).with_context(|| {
        format!(
            "Failed to parse verifier registry address from source registry {}",
            backend_info.source_registry
        )
    })
}

fn get_verifier_quorum(
    api_client: &TonApiClient,
    registry_address: &TonAddress,
    verifier_id: &str,
    is_testnet: bool,
) -> anyhow::Result<u8> {
    let registry_address = format_ton_address(registry_address, is_testnet);
    let result = api_client.run_get_method(&registry_address, "get_verifiers", &[])?;
    parse_verifier_quorum_from_get_method(&result, verifier_id)
}

fn parse_verifier_registry_address(result: &GetMethodResult) -> anyhow::Result<TonAddress> {
    let cell = parse_stack_cell(result, "get_verifier_registry_address")?;
    TonAddress::from_cell(&TonCell::from_boc(Boc::encode(cell))?)
        .context("Failed to parse registry address from object")
}

fn parse_verifier_quorum_from_get_method(
    result: &GetMethodResult,
    verifier_id: &str,
) -> anyhow::Result<u8> {
    let cell = parse_stack_cell(result, "get_verifiers")?;
    let mut parser = cell
        .as_slice()
        .context("Failed to parse verifier cell slice")?;
    let verifiers = Dict::<HashBytes, CellSlice>::load_from(&mut parser)
        .context("Failed to parse verifier dictionary")?;

    let mut available_verifiers = Vec::new();
    for verifier_entry in verifiers.iter() {
        let (_, mut verifier) = verifier_entry.context("Failed to iterate verifier dictionary")?;
        let _admin =
            IntAddr::load_from(&mut verifier).context("Failed to parse verifier admin address")?;
        let quorum = verifier
            .load_u8()
            .context("Failed to parse verifier quorum")?;
        let _pub_key_endpoints = RawDict::<256>::load_from(&mut verifier)
            .context("Failed to parse verifier endpoints")?;
        let name = parse_string_ref(&mut verifier).context("Failed to parse verifier name")?;
        let _url = parse_string_ref(&mut verifier).context("Failed to parse verifier URL")?;

        available_verifiers.push(name.clone());
        if name == verifier_id {
            if quorum == 0 {
                anyhow::bail!("Verifier '{verifier_id}' returned zero quorum");
            }
            return Ok(quorum);
        }
    }

    available_verifiers.sort();
    available_verifiers.dedup();
    anyhow::bail!(
        "Verifier '{verifier_id}' is not registered in verifier registry. Available verifiers: {}",
        if available_verifiers.is_empty() {
            "none".to_string()
        } else {
            available_verifiers.join(", ")
        }
    );
}

fn parse_stack_cell(result: &GetMethodResult, method_name: &str) -> anyhow::Result<Cell> {
    if result.exit_code != 0 {
        anyhow::bail!(
            "{method_name} returned non-zero exit code: {}",
            result.exit_code
        );
    }

    let tuple = result.parse_stack_tuple().with_context(|| {
        format!("Failed to parse stack from '{method_name}' with tvmffi JSON stack parser")
    })?;
    let Some(item) = tuple.first() else {
        anyhow::bail!("Stack from '{method_name}' is empty");
    };

    match item {
        TupleItem::Cell(cell) | TupleItem::Slice(cell) => Ok(cell.clone()),
        _ => {
            anyhow::bail!("Unexpected stack item type for '{method_name}'");
        }
    }
}

fn parse_string_ref(parser: &mut CellSlice<'_>) -> anyhow::Result<String> {
    let string_cell = parser
        .load_reference_cloned()
        .context("Expected string reference")?;
    Tuple::parse_snake_string(&string_cell)
        .ok_or_else(|| anyhow!("String reference is not valid UTF-8"))
}
