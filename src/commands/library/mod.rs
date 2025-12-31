use crate::commands::common::{error_fmt, select_contract, select_wallet};
use crate::commands::disasm::disasm_cmd;
use crate::config::ActonConfig;
use crate::wallets::open_wallets;
use anyhow::{Context, anyhow};
use inquire::Text;
use num_bigint::BigUint;
use owo_colors::OwoColorize;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tasm::printer::FormatOptions;
use tempfile::TempDir;
use tolkc::CompilerResult;
use ton_api::{Network, TonApiClient};
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::coins::{CurrencyCollection, Grams};
use tonlib_core::tlb_types::block::message::{CommonMsgInfo, IntMsgInfo, Message};
use tonlib_core::tlb_types::block::state_init::StateInit;
use tonlib_core::tlb_types::primitives::either::EitherRef;
use tonlib_core::tlb_types::primitives::reference::Ref;
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::boc::Boc;

pub fn publish_cmd(
    contract_id: Option<String>,
    code_arg: Option<String>,
    duration_arg: Option<String>,
    wallet_name: Option<String>,
    api_key: Option<String>,
    net: String,
) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;
    let network = Network::from_str(&net)?;

    let library_code_cell = if let Some(code_str) = code_arg {
        if let Ok(cell) = Boc::decode_base64(&code_str) {
            cell
        } else if let Ok(cell) = Boc::decode_hex(&code_str) {
            cell
        } else {
            anyhow::bail!("Failed to decode BoC data as hex or base64");
        }
    } else {
        let contract_key = select_contract(contract_id, &config)?;
        let contract = config
            .get_contract(&contract_key)
            .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, &contract_key)))?;
        let contract_path =
            fs::canonicalize(contract.src.clone()).unwrap_or(PathBuf::from(contract.src.clone()));

        if contract_path.extension() != Some("tolk".as_ref()) {
            anyhow::bail!(color_print::cformat!(
                "Contract source must be a <yellow>.tolk</> file"
            ));
        }

        println!("  {} Compiling contract", "→".blue().bold());
        let compilation_result = tolkc::compile(Path::new(&contract_path), false);

        match compilation_result {
            CompilerResult::Success(result) => {
                println!("  {} Compiled successfully", "✓".green().bold());
                Boc::decode_base64(&result.code_boc64)?
            }
            CompilerResult::Error(error) => {
                anyhow::bail!(
                    "{}\nFix compilation error first to publish library",
                    error.message
                );
            }
        }
    };

    let library_hash = library_code_cell.repr_hash();
    println!(
        "  {} Library hash: {}",
        "→".blue().bold(),
        hex::encode(library_hash).dimmed()
    );

    let duration_seconds = if let Some(d) = duration_arg {
        parse_duration(&d)?
    } else {
        let input = Text::new("Enter duration (e.g., 100d, 3600s, 1y):")
            .with_default("365d")
            .prompt()?;
        parse_duration(&input)?
    };

    println!(
        "  {} Duration: {} seconds",
        "→".blue().bold(),
        duration_seconds
    );

    let librarian_code = compile_librarian_with_duration(duration_seconds)?;

    let workchain = -1;
    let publisher_data = ArcCell::from_boc(&Boc::encode(&library_code_cell))?;
    let state_init = StateInit {
        split_depth: None,
        tick_tock: None,
        code: Some(Ref::new(librarian_code)),
        data: Some(Ref::new(publisher_data)),
        library: None,
    };
    let state_init_cell = state_init.to_cell()?;
    let state_init_hash = state_init_cell.cell_hash();

    let publisher_address = TonAddress::new(workchain, state_init_hash);

    println!(
        "  {} Publisher address: {}",
        "→".blue().bold(),
        publisher_address.to_base64_std().dimmed()
    );

    let wallet_name = select_wallet(wallet_name, &config)?;
    let mut wallets = open_wallets(&config, Some(network.as_str()), true)?;
    let wallet = wallets
        .remove(&wallet_name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &wallet_name)))?;

    println!(
        "  {} Using wallet: {} {}",
        "→".blue().bold(),
        wallet_name.cyan(),
        wallet.wallet.address.to_base64_std().dimmed()
    );

    let amount_to_send = Text::new("Enter amount in TON (leave empty to cancel):").prompt()?;

    if amount_to_send.trim().is_empty() {
        return Ok(());
    }

    let custom_ton: f64 = amount_to_send
        .trim()
        .parse()
        .context("Invalid TON amount")?;

    let confirm_custom = inquire::Confirm::new(&format!(
        "Send {:.4} TON to publish library? Note that any extra TON will be refunded.",
        custom_ton
    ))
    .with_default(true)
    .prompt()?;

    if !confirm_custom {
        return Ok(());
    }

    let amount_to_send = (custom_ton * 1_000_000_000.0) as u128;

    let api_client = TonApiClient::new(network.clone(), api_key.clone());
    let (seqno, need_state_init) = wallet.seqno(network.as_str())?;

    let expired_at_time = std::time::SystemTime::now() + std::time::Duration::from_secs(600);
    let expire_at = expired_at_time
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as u32;

    let message_info = IntMsgInfo {
        ihr_disabled: true,
        bounce: false,
        bounced: false,
        src: wallet.wallet.address.to_msg_address(),
        dest: publisher_address.to_msg_address(),
        value: CurrencyCollection::new(BigUint::from(amount_to_send)),
        ihr_fee: Grams::new(BigUint::from(0u64)),
        fwd_fee: Grams::new(BigUint::from(0u64)),
        created_at: 0,
        created_lt: 0,
    };

    let message = Message {
        info: CommonMsgInfo::Int(message_info),
        init: Some(EitherRef::new(state_init)),
        body: EitherRef::new(ArcCell::default()),
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
        .context("Failed to send publication transaction")?;

    println!("  {} Transaction sent successfully", "✓".green().bold());
    println!(
        "  {} Library should be available soon at hash: {}",
        "→".blue().bold(),
        hex::encode(library_hash).dimmed()
    );

    Ok(())
}

pub fn fetch_cmd(
    hash: String,
    disasm: bool,
    api_key: Option<String>,
    output: Option<String>,
    net: String,
    json: bool,
) -> anyhow::Result<()> {
    let network = Network::from_str(&net)?;
    let client = TonApiClient::new(network, api_key);

    if !json {
        println!("  {} Fetching library: {}", "→".blue().bold(), hash);
    }

    let library_cell = client.get_library_by_hash(&hash)?;

    if !json {
        println!("  {} Fetched successfully", "✓".green().bold());
    }

    if disasm {
        let boc_hex = Boc::encode_hex(library_cell.clone());

        disasm_cmd(
            None,
            Some(boc_hex),
            output.clone(), // If output provided, disasm writes to it
            FormatOptions::default(),
            None,
            None,
            net,
            false,
        )?;
    } else {
        let boc_base64 = Boc::encode_base64(&library_cell);

        if json {
            println!(
                "{}",
                serde_json::json!({
                    "success": true,
                    "code_boc64": boc_base64
                })
            );
            return Ok(());
        }

        if let Some(path) = output {
            if path.ends_with(".boc") {
                let bytes = Boc::encode(&library_cell);
                fs::write(&path, bytes)?;
            } else {
                fs::write(&path, &boc_base64)?;
            }
            println!("  {} Written to {}", "✓".green().bold(), path);
        } else {
            println!("{}", boc_base64);
        }
    }

    Ok(())
}

fn parse_duration(s: &str) -> anyhow::Result<u64> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("Duration cannot be empty");
    }

    let (num_str, unit) = if let Some(stripped) = s.strip_suffix('d') {
        (stripped, "d")
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, "s")
    } else if let Some(stripped) = s.strip_suffix('y') {
        (stripped, "y")
    } else if s.chars().all(|c| c.is_ascii_digit()) {
        (s, "s")
    } else {
        anyhow::bail!("Invalid duration format. Use '100s', '100d', '1y' or just number (seconds)");
    };

    let num: u64 = num_str.parse().context("Invalid number in duration")?;

    match unit {
        "s" => Ok(num),
        "d" => Ok(num * 24 * 60 * 60),
        "y" => Ok(num * 365 * 24 * 60 * 60),
        _ => unreachable!(),
    }
}

fn compile_librarian_with_duration(duration: u64) -> anyhow::Result<ArcCell> {
    let content = include_str!("librarian/librarian.tolk");
    let content = content.replace(
        "3600 * 24 * 365 * 1 // 1 year, can top-up in any time",
        &duration.to_string(),
    );
    let tmp_dir = TempDir::new()?;
    let tmp_file_path = tmp_dir.path().join("librarian.tolk");
    let mut tmp_file = File::create(&tmp_file_path)?;
    tmp_file.write_all(content.as_bytes())?;
    let compilation_result = tolkc::compile(tmp_file_path.as_ref(), true);
    match compilation_result {
        CompilerResult::Success(result) => Ok(ArcCell::from_boc_b64(&result.code_boc64)?),
        CompilerResult::Error(err) => {
            anyhow::bail!("Unable to compile librarian: {}", err.message);
        }
    }
}
