use crate::commands::common::{error_fmt, format_nanograms, select_contract, select_wallet};
use crate::contract_interface::is_boc_path;
use crate::ffi::emulation;
use crate::tonconnect::{TonConnectContext, TonConnectSession};
use crate::wallets::open_wallets;
use crate::{http, tonconnect, transaction_hash};
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, project_root as configured_project_root};
use anyhow::{Context, anyhow};
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use ton::ton_core::types::TonAddress;
use ton_api::{Network, TonApiClient, toncenter::v3};
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellSliceParts, HashBytes};
use tycho_types::models::{
    Base64StdAddrFlags, CurrencyCollection, DisplayBase64StdAddr, IntAddr, MsgInfo, OwnedMessage,
    StdAddr,
};

const VERIFIER_BACKEND: &str = "https://verifier-staging.ton.org";
const VERIFY_BACKEND_ENV: &str = "ACTON_VERIFY_BACKEND";
const VERIFIER_PAYMENT_COMMENT_PREFIX: &str = "acton-verify:v1:";
const SOURCE_UPLOAD_ATTEMPTS: usize = 8;

pub fn verify_cmd(
    contract_id: Option<String>,
    address: Option<String>,
    wallet_name: Option<String>,
    compiler_version: Option<String>,
    dry_run: bool,
    payment_tx_hash: Option<String>,
    tonconnect: bool,
) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let contract_key = select_contract(contract_id, &config)?;
    let contract = config
        .get_contract(&contract_key)
        .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, &contract_key)))?;
    let contract_path = contract.absolute_source_path(configured_project_root());
    let contract_address = address
        .map(|addr| TonAddress::from_str(&addr).with_context(|| error_fmt::invalid_address(&addr)))
        .transpose()?;
    if tonconnect {
        tonconnect::ensure_supported_network(&Network::Testnet)?;
        if wallet_name.is_some() {
            anyhow::bail!(
                "{} cannot be used with {}; TON Connect uses the externally connected wallet",
                "--wallet".yellow(),
                "--tonconnect".yellow()
            );
        }
    }
    println!("  {} Contract: {}", "→".blue().bold(), contract_key.cyan());

    if is_boc_path(&contract_path) {
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
    let compiler = tolk_compiler::Compiler::new(2).with_mappings(&config.mappings());
    let compilation_result = compiler.compile(Path::new(&contract_path), false);

    let (code_boc64, source_map) = match compilation_result {
        tolk_compiler::CompilerResult::Success(result) => {
            println!("  {} Compiled successfully", "✓".green().bold());
            let source_map = result
                .source_map
                .ok_or_else(|| anyhow!("Compiler did not produce symbol types for verification"))?;
            (result.code_boc64, source_map)
        }
        tolk_compiler::CompilerResult::Error(error) => {
            anyhow::bail!(
                "{}\nFix compilation error first to verify contract",
                error.message
            );
        }
    };

    let code = Boc::decode_base64(&code_boc64)?;
    let code_hash = code.repr_hash();
    let code_hash_hex = hex::encode(code_hash);

    println!(
        "  {} Code hash: {}",
        "→".blue().bold(),
        format!("0x{code_hash_hex}").dimmed()
    );

    let Some(payment_quote) = take_verifier_ticket(&code_hash_hex)? else {
        return Ok(());
    };
    let payment_tx_hash = payment_tx_hash
        .as_deref()
        .map(normalize_verifier_transaction_hash)
        .transpose()?;

    if let Some(contract_address) = &contract_address {
        println!(
            "  {} Contract address: {}",
            "→".blue().bold(),
            format_ton_address(contract_address, true).dimmed()
        );

        validate_verifier_address_code_hash(&config, contract_address, code_hash)?;
    }

    println!("  {} Collecting source files", "→".blue().bold());

    let source_files = source_files_from_source_map(&source_map);

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
    let mut normalized_source_paths: Vec<(String, bool)> = Vec::new();

    for (path, is_entrypoint) in &source_files {
        let path = dunce::canonicalize(path).unwrap_or_else(|_| path.clone());
        let file_content = fs::read(&path).context("Failed to read source file")?;
        let source_path = normalize_source_path_for_verifier(&path, project_root);
        normalized_source_paths.push((source_path.clone(), *is_entrypoint));

        upload_parts.push(UploadPart {
            field_name: source_path,
            bytes: file_content,
        });
    }

    let version = compiler_version.unwrap_or_else(|| "1.4.2".to_owned());

    println!("  {} Using TON verifier", "→".blue().bold());

    let backend = verifier_backend();
    let verify_url = format!("{backend}/api/v1/verify");

    println!(
        "  {} Using backend: {}",
        "→".blue().bold(),
        verify_url.dimmed()
    );

    let sources = verifier_sources(&normalized_source_paths)?;
    let compile_params = verifier_compile_params(&config, &version)?;
    let sources_json = serde_json::to_string(&sources)?;
    let compile_params_json = serde_json::to_string(&compile_params)?;

    let payment_amount_nano = payment_quote
        .amount_nano
        .parse::<BigInt>()
        .context("TON verifier returned an invalid payment amount")?;
    if payment_amount_nano <= BigInt::from(0) {
        anyhow::bail!("TON verifier returned a zero payment amount");
    }
    let payment_address = TonAddress::from_str(&payment_quote.payment_address)
        .context("Verifier returned an invalid payment address")?;
    let payment_amount = format_nanograms(&payment_amount_nano);
    let payment_address_display = format_std_address(
        &ton_address_to_std_addr(&payment_address),
        &Network::Testnet,
        true,
    );

    println!("  {} Payment network: TON testnet", "→".blue().bold());
    println!(
        "  {} Payment amount: {}",
        "→".blue().bold(),
        payment_amount.cyan()
    );
    println!(
        "  {} Payment address: {}",
        "→".blue().bold(),
        payment_address_display.dimmed()
    );
    println!(
        "  {} Payment comment: {}",
        "→".blue().bold(),
        payment_quote.comment.dimmed()
    );

    if dry_run {
        println!(
            "  {} Dry run mode: skipping testnet payment and source upload",
            "ℹ".blue().bold()
        );
        println!();
        println!(
            "{}",
            "✓ TON verifier request prepared successfully!"
                .green()
                .bold()
        );
        println!("  Backend: {}", verify_url.dimmed());
        println!(
            "  Source files: {}",
            upload_parts.len().to_string().dimmed()
        );
        return Ok(());
    }

    let tx_hash = match payment_tx_hash {
        Some(tx_hash) => {
            println!(
                "  {} Reusing testnet payment transaction: {}",
                "→".blue().bold(),
                tx_hash.dimmed()
            );
            tx_hash
        }
        None => send_verifier_payment(
            &config,
            &payment_quote,
            &payment_amount_nano,
            &payment_address,
            wallet_name,
            tonconnect,
        )?,
    };

    println!("  {} Sending sources to TON verifier", "→".blue().bold());

    let mut response = None;

    for attempt in 1..=SOURCE_UPLOAD_ATTEMPTS {
        let form = build_verify_form(
            &upload_parts,
            &code_hash_hex,
            &tx_hash,
            &sources_json,
            &compile_params_json,
        )?;

        let source_client = build_verify_http_client()
            .context("Failed to create HTTP client for verifier backend")?;

        match source_client
            .post(&verify_url)
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
                    .unwrap_or("-")
                    .to_owned();

                if status.is_success() {
                    response = Some(res);
                    break;
                }

                let error_text = res.text().unwrap_or_else(|_| "Unknown error".to_string());
                let known_error = VerifierKnownError::from_response_body(&error_text);
                let should_retry = known_error.is_some_and(VerifierKnownError::is_transient)
                    && attempt < SOURCE_UPLOAD_ATTEMPTS;

                if should_retry {
                    println!(
                        "  {} TON verifier returned {} ({}, cf-ray={}) on attempt {attempt}/{SOURCE_UPLOAD_ATTEMPTS}, retrying...",
                        "↻".yellow().bold(),
                        status,
                        http_version,
                        cf_ray
                    );
                    std::thread::sleep(source_retry_delay(attempt));
                    continue;
                }

                if let Some(known_error) = known_error {
                    anyhow::bail!(known_error.friendly_message());
                }
                let body = truncate_for_display(&error_text, 4_000);
                anyhow::bail!(
                    "TON verifier request failed: HTTP {status} ({http_version}) at {verify_url}\nResponse body:\n{body}"
                );
            }
            Err(err) => {
                let should_retry = attempt < SOURCE_UPLOAD_ATTEMPTS;
                if should_retry {
                    println!(
                        "  {} Network error on attempt {attempt}/{SOURCE_UPLOAD_ATTEMPTS}, retrying...\n    {}",
                        "↻".yellow().bold(),
                        err.to_string().dimmed()
                    );
                    std::thread::sleep(source_retry_delay(attempt));
                    continue;
                }
                return Err(err).context("Failed to send request to verifier backend");
            }
        }
    }

    let response =
        response.ok_or_else(|| anyhow!("Failed to get response from verifier backend"))?;

    let verify_result: VerifyResponse = response
        .json()
        .context("Failed to parse verifier response")?;

    if verify_result.code_hash != code_hash_hex {
        anyhow::bail!(
            "TON verifier returned a result for a different code hash: expected {code_hash_hex}, received {}",
            verify_result.code_hash
        );
    }
    if matches!(verify_result.verification_result, VerificationResult::Match)
        && verify_result.compiled_code_hash.as_deref() != Some(code_hash_hex.as_str())
    {
        anyhow::bail!("TON verifier reported a match without a matching compiled code hash");
    }

    match verify_result.verification_result {
        VerificationResult::AlreadyVerified => {
            println!("  {} Contract was already verified", "✓".green().bold());
            if let Some(source_bundle_hash) = &verify_result.source_bundle_hash {
                println!(
                    "  {} Source bundle: {}",
                    "→".blue().bold(),
                    source_bundle_hash.dimmed()
                );
            }
            if let Some(storage_revision) = &verify_result.storage_revision {
                println!(
                    "  {} Storage revision: {}",
                    "→".blue().bold(),
                    storage_revision.dimmed()
                );
            }
            println!();
            show_verifier_link(&backend, &code_hash_hex);
            return Ok(());
        }
        VerificationResult::Mismatch => {
            anyhow::bail!(
                "Verification failed: compiled code hash {} does not match target code hash {}",
                verify_result
                    .compiled_code_hash
                    .as_deref()
                    .unwrap_or("<unknown>"),
                verify_result.code_hash
            );
        }
        VerificationResult::Match => {}
    }

    println!(
        "  {} TON verifier accepted source bundle",
        "✓".green().bold()
    );
    if let Some(source_bundle_hash) = &verify_result.source_bundle_hash {
        println!(
            "  {} Source bundle: {}",
            "→".blue().bold(),
            source_bundle_hash.dimmed()
        );
    }
    if let Some(storage_revision) = &verify_result.storage_revision {
        println!(
            "  {} Storage revision: {}",
            "→".blue().bold(),
            storage_revision.dimmed()
        );
    }

    println!();
    println!("{}", "✓ Contract verification completed!".green().bold());
    show_verifier_link(&backend, &code_hash_hex);

    Ok(())
}

#[derive(Debug, Serialize)]
struct VerifierSource {
    path: String,
    is_entrypoint: bool,
    include_in_command: Option<bool>,
    is_stdlib: Option<bool>,
    has_include_directives: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct VerifyResponse {
    code_hash: String,
    compiled_code_hash: Option<String>,
    verification_result: VerificationResult,
    #[serde(default)]
    source_bundle_hash: Option<String>,
    #[serde(default)]
    storage_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum VerifierTicketResponse {
    AlreadyVerified {
        code_hash: String,
        source_bundle_hash: String,
        storage_revision: String,
    },
    PaymentRequired {
        code_hash: String,
        payment_address: String,
        amount_nano: String,
        comment: String,
    },
}

#[derive(Debug, Deserialize)]
struct VerifierErrorResponse {
    error: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VerifierKnownError {
    RecoveryInProgress,
    VerificationRetryable,
    InvalidTransactionHash,
    NotFound,
    Invalid,
    Insufficient,
    CodeHashMismatch,
    Used,
    InProgress,
}

impl VerifierKnownError {
    fn from_response_body(body: &str) -> Option<Self> {
        let response = serde_json::from_str::<VerifierErrorResponse>(body).ok()?;
        let code = response
            .error
            .split_once(':')
            .map_or(response.error.as_str(), |(code, _)| code);

        match code {
            "payment_recovery_in_progress" => Some(Self::RecoveryInProgress),
            "verification_retryable" => Some(Self::VerificationRetryable),
            "payment_tx_hash_invalid" => Some(Self::InvalidTransactionHash),
            "payment_not_found" => Some(Self::NotFound),
            "payment_invalid" => Some(Self::Invalid),
            "payment_insufficient" => Some(Self::Insufficient),
            "payment_code_hash_mismatch" => Some(Self::CodeHashMismatch),
            "payment_used" => Some(Self::Used),
            "payment_in_progress" => Some(Self::InProgress),
            _ => None,
        }
    }

    const fn friendly_message(self) -> &'static str {
        match self {
            Self::RecoveryInProgress => {
                "TON verifier is rebuilding payment history. Try again shortly"
            }
            Self::VerificationRetryable => {
                "Verifier source storage is temporarily unavailable. The payment remains reusable; try again shortly"
            }
            Self::InvalidTransactionHash => {
                "Payment transaction hash is invalid. Use a 64-character hexadecimal or 32-byte base64 hash"
            }
            Self::NotFound => {
                "Payment transaction was not found on TON testnet. Check the transaction hash and try again"
            }
            Self::Invalid => {
                "Payment transaction is not a finalized incoming payment to the verifier wallet"
            }
            Self::Insufficient => {
                "Payment amount is too small. Send at least the amount shown in the verification ticket"
            }
            Self::CodeHashMismatch => {
                "Payment transaction is for a different code hash. Request a new ticket and pay with the exact payment comment"
            }
            Self::Used => "Payment transaction was already used for a verification",
            Self::InProgress => {
                "Payment transaction is already being processed. Wait for the current verification to finish"
            }
        }
    }

    const fn is_transient(self) -> bool {
        matches!(
            self,
            Self::RecoveryInProgress | Self::VerificationRetryable | Self::InProgress
        )
    }
}

fn normalize_verifier_transaction_hash(transaction_hash: &str) -> anyhow::Result<String> {
    transaction_hash::toncenter_transaction_hash_hex(transaction_hash.trim()).map_err(|_| {
        anyhow!(
            "Invalid --payment-tx-hash: expected a 64-character hexadecimal or 32-byte base64 TON transaction hash"
        )
    })
}

fn friendly_verifier_error(body: &str) -> Option<&'static str> {
    VerifierKnownError::from_response_body(body).map(VerifierKnownError::friendly_message)
}

struct VerifierPaymentQuote {
    payment_address: String,
    amount_nano: String,
    comment: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum VerificationResult {
    AlreadyVerified,
    Match,
    Mismatch,
}

#[derive(Debug, Clone)]
struct UploadPart {
    field_name: String,
    bytes: Vec<u8>,
}

fn take_verifier_ticket(code_hash: &str) -> anyhow::Result<Option<VerifierPaymentQuote>> {
    println!("  {} Requesting verification ticket", "→".blue().bold());
    let backend = verifier_backend();
    let ticket_url = format!("{backend}/api/v1/take_ticket");
    let client =
        build_verify_http_client().context("Failed to create HTTP client for verifier backend")?;
    let response = client
        .post(&ticket_url)
        .json(&serde_json::json!({ "code_hash": code_hash }))
        .send()
        .with_context(|| format!("Failed to request verification ticket from {ticket_url}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .unwrap_or_else(|_| "Unknown error".to_owned());
        if let Some(message) = friendly_verifier_error(&body) {
            anyhow::bail!(message);
        }
        anyhow::bail!(
            "Verification ticket request failed: HTTP {status} at {ticket_url}\nResponse body:\n{}",
            truncate_for_display(&body, 4_000)
        );
    }

    match response
        .json::<VerifierTicketResponse>()
        .context("Failed to parse verification ticket response")?
    {
        VerifierTicketResponse::AlreadyVerified {
            code_hash: returned_code_hash,
            source_bundle_hash,
            storage_revision,
        } => {
            ensure_ticket_code_hash(code_hash, &returned_code_hash)?;
            println!("  {} Contract was already verified", "✓".green().bold());
            println!(
                "  {} Source bundle: {}",
                "→".blue().bold(),
                source_bundle_hash.dimmed()
            );
            println!(
                "  {} Storage revision: {}",
                "→".blue().bold(),
                storage_revision.dimmed()
            );
            println!();
            show_verifier_link(&backend, code_hash);
            Ok(None)
        }
        VerifierTicketResponse::PaymentRequired {
            code_hash: returned_code_hash,
            payment_address,
            amount_nano,
            comment,
        } => {
            ensure_ticket_code_hash(code_hash, &returned_code_hash)?;
            ensure_ticket_payment_details(code_hash, &payment_address, &comment)?;
            Ok(Some(VerifierPaymentQuote {
                payment_address,
                amount_nano,
                comment,
            }))
        }
    }
}

fn ensure_ticket_code_hash(expected: &str, actual: &str) -> anyhow::Result<()> {
    if expected != actual {
        anyhow::bail!(
            "Verifier ticket returned a different code hash: expected {expected}, received {actual}"
        );
    }
    Ok(())
}

fn ensure_ticket_payment_details(
    code_hash: &str,
    payment_address: &str,
    comment: &str,
) -> anyhow::Result<()> {
    let expected_comment = format!("{VERIFIER_PAYMENT_COMMENT_PREFIX}{code_hash}");
    if comment != expected_comment {
        anyhow::bail!("Verifier ticket returned a payment comment for a different code hash");
    }

    let payment_address = TonAddress::from_str(payment_address)
        .context("Verifier ticket returned an invalid payment address")?;
    if payment_address.workchain != 0 {
        anyhow::bail!(
            "Verifier ticket returned a non-basechain payment address; expected workchain 0"
        );
    }

    Ok(())
}

fn send_verifier_payment(
    config: &ActonConfig,
    quote: &VerifierPaymentQuote,
    amount_nano: &BigInt,
    payment_address: &TonAddress,
    wallet_name: Option<String>,
    tonconnect: bool,
) -> anyhow::Result<String> {
    let network = Network::Testnet;
    let payment_address_display = format_std_address(
        &ton_address_to_std_addr(payment_address),
        &Network::Testnet,
        true,
    );

    let normalized_external_hash = if tonconnect {
        let storage_path = tonconnect::session_storage_path(configured_project_root(), &network)?;
        let session = Arc::new(TonConnectSession::start(storage_path)?);
        let connected_wallet = session.connect(&network)?;
        let sender_address = format_std_address(&connected_wallet.address, &network, false);
        println!(
            "  {} Using TON Connect testnet wallet: {}",
            "→".blue().bold(),
            sender_address.dimmed()
        );
        let message = build_verifier_payment_message(
            connected_wallet.address.clone(),
            payment_address,
            amount_nano,
            &quote.comment,
        )?;
        let context = TonConnectContext {
            session,
            wallet: connected_wallet,
        };
        let (_, normalized_hash) =
            emulation::send_tonconnect_message(&message, &context, &network)?;
        normalized_hash
    } else {
        let wallet_name = select_wallet(wallet_name, config)?;
        let mut wallets = open_wallets(config, Some(&network), true)?;
        let wallet = wallets
            .remove(&wallet_name)
            .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(config, &wallet_name)))?;
        println!(
            "  {} Using testnet wallet: {} {}",
            "→".blue().bold(),
            wallet_name.cyan(),
            format_ton_address(&wallet.wallet.address, true).dimmed()
        );

        let payment_amount = format_nanograms(amount_nano);
        let confirmed = inquire::Confirm::new(&format!(
            "Send {payment_amount} on TON testnet to {payment_address_display}?"
        ))
        .with_default(false)
        .prompt()
        .context("Failed to read payment confirmation")?;
        if !confirmed {
            anyhow::bail!("Verification payment cancelled");
        }

        let message = build_verifier_payment_message(
            wallet.address(),
            payment_address,
            amount_nano,
            &quote.comment,
        )?;
        let (_, normalized_hash) =
            emulation::send_wallet_message(&message, wallet, &network, config.custom_networks())?;
        normalized_hash
    };

    println!("  {} Testnet payment sent", "✓".green().bold());
    wait_for_verifier_payment(config, quote, &normalized_external_hash)
}

fn build_verifier_payment_message(
    sender: StdAddr,
    payment_address: &TonAddress,
    amount_nano: &BigInt,
    comment: &str,
) -> anyhow::Result<Cell> {
    let amount_nano = u128::try_from(amount_nano.clone())
        .context("Verifier payment amount does not fit a TON currency collection")?;

    let mut body = CellBuilder::new();
    body.store_u32(0)?;
    let comment_bits = u16::try_from(comment.len().saturating_mul(8))
        .context("Verification payment comment is too long")?;
    body.store_raw(comment.as_bytes(), comment_bits)?;
    let body = body.build()?;

    let message = OwnedMessage {
        info: MsgInfo::Int(tycho_types::models::IntMsgInfo {
            ihr_disabled: true,
            bounce: true,
            bounced: false,
            src: IntAddr::Std(sender),
            dst: IntAddr::Std(ton_address_to_std_addr(payment_address)),
            value: CurrencyCollection::new(amount_nano),
            ihr_fee: Default::default(),
            fwd_fee: Default::default(),
            created_lt: 0,
            created_at: 0,
        }),
        init: None,
        body: CellSliceParts::from(body),
        layout: None,
    };
    CellBuilder::build_from(message).context("Failed to build verification payment message")
}

fn wait_for_verifier_payment(
    config: &ActonConfig,
    quote: &VerifierPaymentQuote,
    normalized_external_hash: &HashBytes,
) -> anyhow::Result<String> {
    const ATTEMPTS: usize = 60;

    println!(
        "  {} Waiting for finalized recipient transaction",
        "→".blue().bold()
    );
    let client = TonApiClient::new(Network::Testnet, config.custom_networks())?;
    let message_hash = hex::encode(normalized_external_hash.as_slice());

    for attempt in 1..=ATTEMPTS {
        match client.get_traces_by_msg_hash(&message_hash, 1) {
            Ok(traces) => {
                for trace in traces {
                    if trace.is_incomplete {
                        continue;
                    }
                    if let Some(transaction) = trace
                        .transactions
                        .values()
                        .find(|transaction| is_expected_payment_transaction(transaction, quote))
                    {
                        let transaction_hash_hex =
                            transaction_hash::toncenter_transaction_hash_hex(&transaction.hash)?;
                        let actonscan_url = crate::explorer::actonscan_transaction_link(
                            &Network::Testnet,
                            &transaction_hash_hex,
                        );
                        println!(
                            "  {} Payment finalized: {}",
                            "✓".green().bold(),
                            actonscan_url.underline()
                        );
                        return Ok(transaction.hash.clone());
                    }
                }
            }
            Err(error) => {
                log::debug!("Failed to poll testnet payment trace: {error:#}");
            }
        }

        if attempt < ATTEMPTS {
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    anyhow::bail!(
        "Payment was sent, but its finalized recipient transaction did not appear on TON testnet within {ATTEMPTS} seconds"
    )
}

fn is_expected_payment_transaction(
    transaction: &v3::Transaction,
    quote: &VerifierPaymentQuote,
) -> bool {
    if transaction.emulated
        || transaction.finality != "finalized"
        || transaction.description.aborted != Some(false)
        || !ton_addresses_equal(&transaction.account, &quote.payment_address)
    {
        return false;
    }
    let Some(incoming) = transaction.in_msg.as_ref() else {
        return false;
    };
    if incoming.bounced != Some(false)
        || !incoming
            .destination
            .as_deref()
            .is_some_and(|address| ton_addresses_equal(address, &quote.payment_address))
    {
        return false;
    }
    let expected_amount = quote.amount_nano.parse::<u128>().ok();
    let actual_amount = incoming
        .value
        .as_deref()
        .and_then(|value| value.parse::<u128>().ok());
    if expected_amount
        .zip(actual_amount)
        .is_none_or(|(expected, actual)| actual < expected)
    {
        return false;
    }

    incoming
        .message_content
        .as_ref()
        .and_then(toncenter_message_comment)
        .as_deref()
        == Some(quote.comment.as_str())
}

fn toncenter_message_comment(content: &v3::MessageContent) -> Option<String> {
    content
        .decoded
        .as_ref()
        .and_then(|decoded| {
            decoded
                .get("comment")
                .or_else(|| decoded.get("text"))
                .and_then(serde_json::Value::as_str)
        })
        .map(ToOwned::to_owned)
        .or_else(|| content.body.as_deref().and_then(parse_ton_comment_boc))
}

fn parse_ton_comment_boc(body: &str) -> Option<String> {
    let cell = Boc::decode_base64(body).ok()?;
    let mut slice = cell.as_slice().ok()?;
    (slice.load_u32().ok()? == 0).then_some(())?;
    Tuple::parse_snake_string_slice(&mut slice)
}

fn ton_addresses_equal(left: &str, right: &str) -> bool {
    TonAddress::from_str(left)
        .ok()
        .zip(TonAddress::from_str(right).ok())
        .is_some_and(|(left, right)| left == right)
}

fn build_verify_http_client() -> anyhow::Result<reqwest::blocking::Client> {
    http::blocking_client_builder()
        .pool_max_idle_per_host(0)
        .user_agent(crate::build_info::user_agent())
        .build()
        .context("Failed to build verifier HTTP client")
}

fn parse_backend_env(var_name: &str) -> Option<String> {
    std::env::var(var_name)
        .ok()
        .and_then(|s| normalize_backend_url(&s))
}

pub(crate) fn verifier_backend() -> String {
    parse_backend_env(VERIFY_BACKEND_ENV).unwrap_or_else(|| VERIFIER_BACKEND.to_string())
}

fn normalize_backend_url(raw: &str) -> Option<String> {
    let normalized = raw.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn verifier_sources(paths: &[(String, bool)]) -> anyhow::Result<Vec<VerifierSource>> {
    // Match the public verifier's upload policy before asking the user to pay.
    // Buildable local paths are not necessarily safe portable registry paths.
    anyhow::ensure!(
        paths.len() <= 256,
        "TON verifier accepts at most 256 source files"
    );
    let mut seen = BTreeSet::new();
    paths
        .iter()
        .map(|(path, is_entrypoint)| {
            validate_verifier_relative_path(path, "source path")?;
            let portable = path.len() <= 128
                && path.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-')
                })
                && path.split('/').all(|part| {
                    !part.is_empty() && !part.ends_with('.') && !part.eq_ignore_ascii_case(".git")
                })
                && !path
                    .split('/')
                    .next()
                    .is_some_and(|part| part.eq_ignore_ascii_case("output"));
            anyhow::ensure!(
                portable,
                "Source path is not supported by TON verifier: {path}. \
                 Use relative paths up to 128 ASCII characters with letters, numbers, '.', '_' and '-'; \
                 '.git' and the root 'output' directory are reserved"
            );

            let filename = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
            let source_extension_count = filename
                .split('.')
                .skip(1)
                .filter(|ext| matches!(*ext, "tolk" | "fc" | "func" | "tact" | "pkg"))
                .count();
            anyhow::ensure!(
                filename.ends_with(".tolk") && source_extension_count == 1,
                "Source path must have a single .tolk source extension: {path}"
            );
            anyhow::ensure!(
                seen.insert(path.to_ascii_lowercase()),
                "Duplicate source path (case-insensitive): {path}"
            );

            Ok(VerifierSource {
                path: path.clone(),
                is_entrypoint: *is_entrypoint,
                include_in_command: Some(true),
                is_stdlib: Some(false),
                has_include_directives: Some(true),
            })
        })
        .collect()
}

fn verifier_compile_params(
    config: &ActonConfig,
    version: &str,
) -> anyhow::Result<serde_json::Value> {
    let mut params = serde_json::Map::new();
    params.insert(
        "compiler_version".to_string(),
        serde_json::Value::String(version.to_string()),
    );

    let import_mappings = verifier_import_mappings(config)?;
    if !import_mappings.is_empty() {
        params.insert(
            "import_mappings".to_string(),
            serde_json::to_value(import_mappings)?,
        );
    }

    Ok(serde_json::Value::Object(params))
}

fn verifier_import_mappings(config: &ActonConfig) -> anyhow::Result<BTreeMap<String, String>> {
    let project_root = acton_config::config::project_root();
    config
        .mappings
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| {
            let normalized_key = if key.starts_with('@') {
                key
            } else {
                format!("@{key}")
            };
            let normalized_value =
                normalize_verifier_relative_path(&value, project_root, "import mapping")?;
            Ok((normalized_key, normalized_value))
        })
        .collect()
}

fn normalize_verifier_relative_path(
    raw: &str,
    project_root: &Path,
    label: &str,
) -> anyhow::Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("{label} path is empty");
    }

    let path = Path::new(raw);
    let relative = if path.is_absolute() {
        path.strip_prefix(project_root).with_context(|| {
            format!("{label} path must be relative or inside project root: {raw}")
        })?
    } else {
        path
    };

    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("{label} path contains an invalid component: {raw}");
            }
        }
    }

    if parts.is_empty() {
        anyhow::bail!("{label} path is empty after normalization: {raw}");
    }

    Ok(parts.join("/"))
}

fn validate_verifier_relative_path(raw: &str, label: &str) -> anyhow::Result<()> {
    let _ = normalize_verifier_relative_path(raw, acton_config::config::project_root(), label)?;
    Ok(())
}

fn build_verify_form(
    parts: &[UploadPart],
    code_hash: &str,
    tx_hash: &str,
    sources_json: &str,
    compile_params_json: &str,
) -> anyhow::Result<reqwest::blocking::multipart::Form> {
    let mut form = reqwest::blocking::multipart::Form::new()
        .percent_encode_noop()
        .text("code_hash", code_hash.to_string())
        .text("tx_hash", tx_hash.to_string())
        .text("language", "tolk")
        .text("compile_params", compile_params_json.to_string())
        .text("sources", sources_json.to_string());

    for part in parts {
        form = form.part(
            "files",
            reqwest::blocking::multipart::Part::bytes(part.bytes.clone())
                .file_name(part.field_name.clone())
                .mime_str("application/octet-stream")?,
        );
    }

    Ok(form)
}

fn source_retry_delay(attempt: usize) -> Duration {
    let secs = (attempt as u64).min(10);
    Duration::from_secs(secs)
}

fn format_ton_address(address: &TonAddress, is_testnet: bool) -> String {
    address.to_base64(!is_testnet, false, true)
}

fn format_std_address(address: &StdAddr, network: &Network, bounceable: bool) -> String {
    DisplayBase64StdAddr {
        addr: address,
        flags: Base64StdAddrFlags {
            testnet: network.uses_testnet_address_format(),
            base64_url: true,
            bounceable,
        },
    }
    .to_string()
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

fn source_files_from_source_map(source_map: &tolk_compiler::SourceMap) -> Vec<(PathBuf, bool)> {
    source_map
        .files()
        .iter()
        .filter_map(|file| {
            let path = file.file_name.as_str();
            if path.starts_with("@stdlib/") || path.starts_with("@fiftlib/") {
                None
            } else {
                // in symbol-types, file_id=0 is common.tolk, file_id=1 is main (entrypoint) file
                Some((PathBuf::from(path), file.file_id == 1))
            }
        })
        .collect()
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
    let _ = write!(out, "\n... (truncated, total {total} chars)");
    out
}

fn show_verifier_link(backend: &str, code_hash: &str) {
    println!(
        "View at: {}",
        format!("{}/{}", backend.trim_end_matches('/'), code_hash).blue()
    );
}

fn validate_verifier_address_code_hash(
    config: &ActonConfig,
    contract_address: &TonAddress,
    compiled_code_hash: &HashBytes,
) -> anyhow::Result<()> {
    let address = format_ton_address(contract_address, true);
    println!(
        "  {} Checking deployed code hash for address",
        "→".blue().bold()
    );

    let api_client = TonApiClient::new(Network::Testnet, config.custom_networks())?;
    let account = api_client
        .get_account_state(&address)
        .with_context(|| format!("Failed to fetch account state for {address}"))?;

    if account.status != "active" {
        anyhow::bail!(
            "Address {} is not active (status: {}); cannot compare deployed code hash",
            address.cyan(),
            account.status.yellow()
        );
    }

    let code_boc = account
        .code_boc
        .ok_or_else(|| anyhow!("Address {address} is active but has no code"))?;
    let deployed_code = Boc::decode_base64(&code_boc)
        .with_context(|| format!("Failed to decode deployed code BoC for {address}"))?;
    let deployed_code_hash = deployed_code.repr_hash();

    if deployed_code_hash != compiled_code_hash {
        anyhow::bail!(
            "Address code hash mismatch: compiled {}, deployed {} at {}",
            compiled_code_hash.to_string().yellow(),
            deployed_code_hash.to_string().yellow(),
            address.cyan()
        );
    }

    println!(
        "  {} Address code hash matches compiled code",
        "✓".green().bold()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tycho_types::cell::CellSlice;

    use super::*;

    const PAYMENT_ADDRESS: &str =
        "0:1111111111111111111111111111111111111111111111111111111111111111";
    const SENDER_ADDRESS: &str =
        "0:2222222222222222222222222222222222222222222222222222222222222222";
    const CODE_HASH: &str = "e67eec3bd481c7910c87a061e60ca509e82edd687a0e1c8bf1b437e6de3e6973";
    const COMMENT: &str =
        "acton-verify:v1:e67eec3bd481c7910c87a061e60ca509e82edd687a0e1c8bf1b437e6de3e6973";

    #[test]
    fn verifier_sources_validate_portable_paths_before_payment() {
        for path in [
            "contracts/main.tolk",
            "lib/my-contract.v2.tolk",
            "deps/Lib.TOLK",
        ] {
            assert!(
                verifier_sources(&[(path.to_owned(), true)]).is_ok(),
                "{path}"
            );
        }
        for path in [
            "contracts/my contract.tolk",
            "../main.tolk",
            "/main.tolk",
            "a//main.tolk",
            "a./main.tolk",
            ".Git/main.tolk",
            "output/main.tolk",
            "main.fc.tolk",
            "main.tolk.pkg",
            "main.txt",
            "合约.tolk",
        ] {
            assert!(
                verifier_sources(&[(path.to_owned(), true)]).is_err(),
                "{path}"
            );
        }
        assert!(verifier_sources(&[(format!("{}.tolk", "a".repeat(124)), true)]).is_err());
        assert!(
            verifier_sources(&[
                ("Main.tolk".to_owned(), true),
                ("main.tolk".to_owned(), false)
            ])
            .is_err()
        );
        let paths = (0..256)
            .map(|index| (format!("file{index}.tolk"), index == 0))
            .collect::<Vec<_>>();
        assert!(verifier_sources(&paths).is_ok());
        let mut paths = paths;
        paths.push(("extra.tolk".to_owned(), false));
        assert!(verifier_sources(&paths).is_err());
    }

    #[test]
    fn payment_address_uses_bounceable_testnet_format() {
        let address = TonAddress::from_str(
            "0:3029b3eaeda86a5381d86100f2a8b761c38de45642edb6e4bb1cca2e6dd7ffed",
        )
        .expect("raw payment address should parse");

        assert_eq!(
            format_std_address(&ton_address_to_std_addr(&address), &Network::Testnet, true),
            "kQAwKbPq7ahqU4HYYQDyqLdhw43kVkLttuS7HMoubdf_7eZe"
        );
    }

    fn payment_quote() -> VerifierPaymentQuote {
        VerifierPaymentQuote {
            payment_address: PAYMENT_ADDRESS.to_owned(),
            amount_nano: "10000000".to_owned(),
            comment: COMMENT.to_owned(),
        }
    }

    fn payment_transaction() -> v3::Transaction {
        serde_json::from_value(json!({
            "account": PAYMENT_ADDRESS,
            "hash": "payment-transaction-hash",
            "lt": "1",
            "block_ref": {"workchain": 0, "shard": "8000000000000000", "seqno": 1},
            "now": 1,
            "mc_block_seqno": 1,
            "emulated": false,
            "finality": "finalized",
            "prev_trans_hash": "previous-hash",
            "prev_trans_lt": "0",
            "orig_status": "active",
            "end_status": "active",
            "total_fees": "0",
            "description": {"type": "ord", "aborted": false},
            "in_msg": {
                "hash": "payment-message-hash",
                "destination": PAYMENT_ADDRESS,
                "value": "10000000",
                "bounced": false,
                "message_content": {"decoded": {"comment": COMMENT}}
            },
            "out_msgs": [],
            "account_state_before": {"hash": "before"},
            "account_state_after": {"hash": "after"}
        }))
        .expect("payment transaction fixture should deserialize")
    }

    #[test]
    fn verifier_payment_message_contains_the_exact_code_hash_comment() {
        let sender = ton_address_to_std_addr(
            &TonAddress::from_str(SENDER_ADDRESS).expect("sender address should parse"),
        );
        let destination =
            TonAddress::from_str(PAYMENT_ADDRESS).expect("payment address should parse");
        let cell = build_verifier_payment_message(
            sender.clone(),
            &destination,
            &BigInt::from(10_000_000u64),
            COMMENT,
        )
        .expect("payment message should build");
        let message = cell
            .parse::<OwnedMessage>()
            .expect("payment message should parse");
        let MsgInfo::Int(info) = message.info else {
            panic!("payment message should be internal");
        };

        assert!(info.bounce);
        assert_eq!(info.src, IntAddr::Std(sender));
        assert_eq!(
            info.dst,
            IntAddr::Std(ton_address_to_std_addr(&destination))
        );
        assert_eq!(u128::from(info.value.tokens), 10_000_000);

        let mut body = CellSlice::apply(&message.body).expect("payment body should parse");
        assert_eq!(body.load_u32().expect("comment opcode should load"), 0);
        let bit_len = body.size_bits();
        assert_eq!(usize::from(bit_len), COMMENT.len() * 8);
        let mut comment = vec![0; COMMENT.len()];
        body.load_raw(&mut comment, bit_len)
            .expect("comment bytes should load");
        assert_eq!(comment, COMMENT.as_bytes());
        assert!(COMMENT.ends_with(CODE_HASH));
    }

    #[test]
    fn payment_trace_requires_finality_amount_destination_and_comment() {
        let quote = payment_quote();
        let transaction = payment_transaction();
        assert!(is_expected_payment_transaction(&transaction, &quote));

        let mut overpayment = transaction.clone();
        overpayment
            .in_msg
            .as_mut()
            .expect("payment should have an incoming message")
            .value = Some("10000001".to_owned());
        assert!(is_expected_payment_transaction(&overpayment, &quote));

        let mut pending = transaction.clone();
        pending.finality = "unfinalized".to_owned();
        assert!(!is_expected_payment_transaction(&pending, &quote));

        let mut missing_aborted = transaction.clone();
        missing_aborted.description.aborted = None;
        assert!(!is_expected_payment_transaction(&missing_aborted, &quote));

        let mut aborted = transaction.clone();
        aborted.description.aborted = Some(true);
        assert!(!is_expected_payment_transaction(&aborted, &quote));

        let mut missing_bounced = transaction.clone();
        missing_bounced
            .in_msg
            .as_mut()
            .expect("payment should have an incoming message")
            .bounced = None;
        assert!(!is_expected_payment_transaction(&missing_bounced, &quote));

        let mut bounced = transaction.clone();
        bounced
            .in_msg
            .as_mut()
            .expect("payment should have an incoming message")
            .bounced = Some(true);
        assert!(!is_expected_payment_transaction(&bounced, &quote));

        let mut insufficient = transaction.clone();
        insufficient
            .in_msg
            .as_mut()
            .expect("payment should have an incoming message")
            .value = Some("9999999".to_owned());
        assert!(!is_expected_payment_transaction(&insufficient, &quote));

        let mut wrong_destination = transaction.clone();
        wrong_destination
            .in_msg
            .as_mut()
            .expect("payment should have an incoming message")
            .destination = Some(SENDER_ADDRESS.to_owned());
        assert!(!is_expected_payment_transaction(&wrong_destination, &quote));

        let mut wrong_comment = transaction;
        wrong_comment
            .in_msg
            .as_mut()
            .and_then(|message| message.message_content.as_mut())
            .and_then(|content| content.decoded.as_mut())
            .expect("payment should have a decoded comment")["comment"] =
            json!("acton-verify:v1:wrong-code-hash");
        assert!(!is_expected_payment_transaction(&wrong_comment, &quote));
    }

    #[test]
    fn payment_trace_decodes_the_comment_from_raw_boc() {
        let quote = payment_quote();
        let mut transaction = payment_transaction();
        let mut body = CellBuilder::new();
        body.store_u32(0).expect("comment opcode should store");
        body.store_raw(
            COMMENT.as_bytes(),
            u16::try_from(COMMENT.len() * 8).expect("comment should fit in a cell"),
        )
        .expect("comment should store");
        let body = body.build().expect("comment body should build");
        let content = transaction
            .in_msg
            .as_mut()
            .and_then(|message| message.message_content.as_mut())
            .expect("payment should have message content");
        content.decoded = None;
        content.body = Some(Boc::encode_base64(&body));

        assert!(is_expected_payment_transaction(&transaction, &quote));
    }
}
