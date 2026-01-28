use crate::commands::common::{error_fmt, select_contract, select_wallet};
use crate::commands::disasm::disasm_cmd;
use crate::wallets::open_wallets;
use acton_config::config;
use acton_config::config::{ActonConfig, global_libraries_path};
use anyhow::{Context, anyhow};
use chrono::{DateTime, Local};
use inquire::{Select, Text};
use num_bigint::BigUint;
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tasm::printer::FormatOptions;
use tempfile::TempDir;
use tolkc::CompilerResult;
use toml_edit::{DocumentMut, Item, Table, value};
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
use tycho_types::cell::{CellImpl, HashBytes};

#[allow(clippy::too_many_arguments)]
pub fn publish_cmd(
    contract: Option<String>,
    code_arg: Option<String>,
    duration_arg: Option<String>,
    wallet_name: Option<String>,
    api_key: Option<String>,
    net: String,
    amount_arg: Option<String>,
    yes: bool,
    local: bool,
    global: bool,
) -> anyhow::Result<()> {
    let mut contract_id = contract;
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
        let contract_key = select_contract(contract_id.clone(), &config)?;
        let contract = config
            .get_contract(&contract_key)
            .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, &contract_key)))?;
        let contract_path = fs::canonicalize(contract.src.clone())
            .unwrap_or_else(|_| PathBuf::from(contract.src.clone()));

        if contract_path.extension() != Some("tolk".as_ref()) {
            anyhow::bail!(color_print::cformat!(
                "Contract source must be a <yellow>.tolk</> file"
            ));
        }

        contract_id = Some(contract_key.clone());

        println!("  {} Compiling contract", "→".blue().bold());
        let compiler = tolkc::Compiler::new(2).with_mappings(&config.mappings);
        let compilation_result = compiler.compile(Path::new(&contract_path), false);

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
        publisher_address
            .to_base64_url_flags(true, net == "testnet")
            .dimmed()
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
        wallet
            .wallet
            .address
            .to_base64_url_flags(true, net == "testnet")
            .dimmed()
    );

    let (bits, cells) = calculate_cell_size(library_code_cell.as_ref(), &mut HashSet::new());

    // Masterchain storage prices (config 18)
    // See https://tonviewer.com/config#18
    let bit_price = 1_000u128;
    let cell_price = 500_000u128;
    let bits_part = (u128::from(bits) * bit_price * u128::from(duration_seconds)) >> 16;
    let cells_part = (u128::from(cells) * cell_price * u128::from(duration_seconds)) >> 16;
    let storage_fee_nanoton = bits_part + cells_part;

    // Suggest 120% of storage fee + 0.06 TON for gas/fees
    let suggested_nanoton = (storage_fee_nanoton * 120 / 100) + 60_000_000;

    let amount_to_send_nanoton = if let Some(amount_str) = amount_arg {
        parse_ton_to_nanoton(&amount_str)?
    } else {
        let prompt = format!(
            "Enter amount in TON (at least {} TON for {}):",
            format_ton(suggested_nanoton),
            format_duration(duration_seconds)
        );
        let amount_str = Text::new(&prompt)
            .with_default(&format_ton(suggested_nanoton))
            .prompt()?;

        if amount_str.trim().is_empty() {
            return Ok(());
        }

        parse_ton_to_nanoton(amount_str.trim())?
    };

    if !yes {
        let confirm_custom = inquire::Confirm::new(&format!(
            "Send {} TON to publish library? Note that any extra TON will be refunded.",
            format_ton(amount_to_send_nanoton)
        ))
        .with_default(true)
        .prompt()?;

        if !confirm_custom {
            return Ok(());
        }
    }

    let api_client = TonApiClient::new(network.clone(), api_key)?;
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
        value: CurrencyCollection::new(BigUint::from(amount_to_send_nanoton)),
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

    save_library(
        contract_id.as_deref().unwrap_or("unknown"),
        &hex::encode(library_hash),
        &Boc::encode_base64(&library_code_cell),
        &publisher_address.to_base64_url_flags(true, net == "testnet"),
        duration_seconds,
        net,
        bits,
        cells,
        local,
        global,
    )?;

    println!("  {} Library info saved", "✓".green().bold());
    Ok(())
}

fn calculate_cell_size(cell: &dyn CellImpl, seen: &mut HashSet<HashBytes>) -> (u64, u64) {
    let mut bits = u64::from(cell.bit_len());
    let mut cells = 0u64;
    for i in 0..4 {
        if let Some(r) = cell.reference(i) {
            if !seen.insert(*r.repr_hash()) {
                // already processed
                continue;
            }

            let (b, r_count) = calculate_cell_size(r, seen);
            bits += b;
            cells += 1 + r_count;
        }
    }
    (bits, cells)
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
    let client = TonApiClient::new(network, api_key)?;

    if !json {
        println!("  {} Fetching library: {}", "→".blue().bold(), hash);
    }

    let library_cell = client.get_library_by_hash(&hash)?;

    if !json {
        println!("  {} Fetched successfully", "✓".green().bold());
    }

    if disasm {
        let boc_hex = Boc::encode_hex(library_cell);

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
            println!("{boc_base64}");
        }
    }

    Ok(())
}

pub fn info_cmd(name: Option<String>, api_key: Option<String>) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;
    let libraries = config
        .libraries()
        .ok_or_else(|| anyhow!(error_fmt::no_libraries_found()))?;

    if libraries.is_empty() {
        anyhow::bail!(error_fmt::no_libraries_found());
    }

    let lib_name = if let Some(n) = name {
        n
    } else {
        let names = libraries.keys().cloned().collect::<Vec<_>>();
        Select::new("Select library:", names).prompt()?
    };

    let lib = libraries
        .get(&lib_name)
        .ok_or_else(|| anyhow!(error_fmt::library_not_found(&config, &lib_name)))?;

    let network = Network::from_str(&lib.network.to_string())?;
    let api_client = TonApiClient::new(network, api_key)?;

    let mut balance_u128: Option<u128> = None;
    let mut remaining_seconds: Option<u128> = None;

    if let Ok(balance) = api_client.get_address_balance(&lib.account) {
        balance_u128 = Some(balance.to_string().parse().unwrap_or(0));

        // Storage cost calculation (config 18)
        // See https://tonviewer.com/config#18
        let bit_price = 1_000u128;
        let cell_price = 500_000u128;

        let cost_per_second_x65536 =
            (u128::from(lib.bits) * bit_price) + (u128::from(lib.cells) * cell_price);

        if cost_per_second_x65536 > 0
            && let Some(balance_u128) = balance_u128
        {
            remaining_seconds = Some((balance_u128 * 65536) / cost_per_second_x65536);
        }
    }

    let w = 12;
    println!("{:<w$} {}", "Library:".dimmed(), lib_name.cyan().bold());

    println!(
        "{:<w$} {} ({})",
        "Deployed at:".dimmed(),
        lib.timestamp,
        format_relative_time(&lib.timestamp),
        w = w
    );
    println!("{:<w$} {}", "Contract:".dimmed(), lib.name);
    println!("{:<w$} {}", "Network:".dimmed(), lib.network);
    println!("{:<w$} {}", "Hash:".dimmed(), lib.hash.yellow());
    println!("{:<w$} {}", "Account:".dimmed(), lib.account.yellow());
    println!(
        "{:<w$} {} ({}s)",
        "Funded for:".dimmed(),
        format_duration(lib.duration),
        lib.duration
    );

    if let Some(balance_u128) = balance_u128 {
        println!(
            "{:<w$} {} TON",
            "Balance:".dimmed(),
            format_ton(balance_u128)
        );
    }

    if let Some(remaining_seconds) = remaining_seconds {
        println!(
            "{:<w$} ~{} ({}s)",
            "Remaining:".dimmed(),
            format_duration(remaining_seconds as u64),
            remaining_seconds
        );
    }

    println!("{:<w$} {}", "Code:".dimmed(), lib.code.magenta());

    println!(
        "{:<w$} {} bits, {} cells",
        "Size:".dimmed(),
        lib.bits,
        lib.cells
    );
    Ok(())
}

pub fn topup_cmd(
    name: Option<String>,
    duration_arg: Option<String>,
    wallet_name: Option<String>,
    api_key: Option<String>,
    amount_arg: Option<String>,
    yes: bool,
) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;
    let libraries = config
        .libraries()
        .ok_or_else(|| anyhow!(error_fmt::no_libraries_found()))?;

    if libraries.is_empty() {
        anyhow::bail!(error_fmt::no_libraries_found());
    }

    let lib_name = if let Some(n) = name {
        n
    } else {
        let names = libraries.keys().cloned().collect::<Vec<_>>();
        Select::new("Select library to top up:", names).prompt()?
    };

    let lib = libraries
        .get(&lib_name)
        .ok_or_else(|| anyhow!(error_fmt::library_not_found(&config, &lib_name)))?;

    let wallet_name = select_wallet(wallet_name, &config)?;
    let network = Network::from_str(&lib.network.to_string())?;
    let mut wallets = open_wallets(&config, Some(network.as_str()), true)?;
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
            .to_base64_url_flags(true, lib.network == config::Network::Testnet)
            .dimmed()
    );

    let amount_to_send_nanoton = if let Some(amount_str) = amount_arg {
        parse_ton_to_nanoton(&amount_str)?
    } else {
        let duration_seconds = if let Some(d) = duration_arg {
            parse_duration(&d)?
        } else {
            let input = Text::new("Enter duration to top up for (e.g., 100d, 1y):")
                .with_default("365d")
                .prompt()?;
            parse_duration(&input)?
        };

        // Storage cost calculation (config 18)
        let bit_price = 1_000u128;
        let cell_price = 500_000u128;
        let bits_part = (u128::from(lib.bits) * bit_price * u128::from(duration_seconds)) >> 16;
        let cells_part = (u128::from(lib.cells) * cell_price * u128::from(duration_seconds)) >> 16;
        let storage_fee_nanoton = bits_part + cells_part;

        let suggested_nanoton = storage_fee_nanoton * 120 / 100;

        let prompt = format!(
            "Enter amount in TON (at least {} TON for {}):",
            format_ton(suggested_nanoton),
            format_duration(duration_seconds)
        );
        let amount_str = Text::new(&prompt)
            .with_default(&format_ton(suggested_nanoton))
            .prompt()?;

        parse_ton_to_nanoton(amount_str.trim())?
    };

    if !yes {
        let confirm = inquire::Confirm::new(&format!(
            "Send {} TON to top-up library?",
            format_ton(amount_to_send_nanoton),
        ))
        .with_default(true)
        .prompt()?;

        if !confirm {
            return Ok(());
        }
    }

    let api_client = TonApiClient::new(network.clone(), api_key)?;
    let (seqno, need_state_init) = wallet.seqno(network.as_str())?;

    let expired_at_time = std::time::SystemTime::now() + std::time::Duration::from_secs(600);
    let expire_at = expired_at_time
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as u32;

    let dest_address = TonAddress::from_str(&lib.account)?;
    let message_info = IntMsgInfo {
        ihr_disabled: true,
        bounce: true,
        bounced: false,
        src: wallet.wallet.address.to_msg_address(),
        dest: dest_address.to_msg_address(),
        value: CurrencyCollection::new(BigUint::from(amount_to_send_nanoton)),
        ihr_fee: Grams::new(BigUint::from(0u64)),
        fwd_fee: Grams::new(BigUint::from(0u64)),
        created_at: 0,
        created_lt: 0,
    };

    let message = Message {
        info: CommonMsgInfo::Int(message_info),
        init: None,
        body: EitherRef::new(ArcCell::default()),
    };

    let message_cell = message.to_cell()?;
    let external = wallet.wallet.create_external_msg(
        expire_at,
        seqno,
        need_state_init,
        vec![message_cell.to_arc()],
    )?;

    println!("  {} Sending transaction...", "→".blue().bold());
    api_client
        .send_boc(&external.to_boc_b64(false)?)
        .context("Failed to send top-up transaction")?;

    println!(
        "  {} Top-up transaction sent successfully",
        "✓".green().bold()
    );
    Ok(())
}

fn format_duration(seconds: u64) -> String {
    let mut remaining = seconds;
    let years = remaining / 31_536_000;
    remaining %= 31_536_000;
    let days = remaining / 86_400;
    remaining %= 86_400;
    let hours = remaining / 3600;
    remaining %= 3600;
    let minutes = remaining / 60;
    let seconds = remaining % 60;

    let mut parts = Vec::new();
    if years > 0 {
        parts.push(format!(
            "{} year{}",
            years,
            if years > 1 { "s" } else { "" }
        ));
    }
    if days > 0 {
        parts.push(format!("{} day{}", days, if days > 1 { "s" } else { "" }));
    }
    if hours > 0 {
        parts.push(format!(
            "{} hour{}",
            hours,
            if hours > 1 { "s" } else { "" }
        ));
    }
    if minutes > 0 {
        parts.push(format!(
            "{} minute{}",
            minutes,
            if minutes > 1 { "s" } else { "" }
        ));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!(
            "{} second{}",
            seconds,
            if seconds > 1 { "s" } else { "" }
        ));
    }

    parts.join(" ")
}

fn format_relative_time(timestamp_str: &str) -> String {
    let Ok(dt) = DateTime::parse_from_rfc3339(timestamp_str) else {
        return timestamp_str.to_string();
    };
    let now = Local::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() < 60 {
        return "just now".to_string();
    }
    if duration.num_minutes() < 60 {
        return format!("{} min ago", duration.num_minutes());
    }
    if duration.num_hours() < 24 {
        return format!("{} hours ago", duration.num_hours());
    }
    if duration.num_days() < 30 {
        return format!("{} days ago", duration.num_days());
    }
    if duration.num_days() < 365 {
        let months = duration.num_days() / 30;
        return format!("{} month{} ago", months, if months > 1 { "s" } else { "" });
    }
    let years = duration.num_days() / 365;
    format!("{} year{} ago", years, if years > 1 { "s" } else { "" })
}

#[must_use]
pub fn format_ton(nanoton: u128) -> String {
    let ton = nanoton / 1_000_000_000;
    let fraction = nanoton % 1_000_000_000;

    if fraction == 0 {
        return ton.to_string();
    }

    let fraction_str = format!("{fraction:09}");
    let trimmed_fraction = fraction_str.trim_end_matches('0');
    format!("{ton}.{trimmed_fraction}")
}

pub fn parse_ton_to_nanoton(s: &str) -> anyhow::Result<u128> {
    let s = s.trim();
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() > 2 {
        anyhow::bail!("Invalid TON format: multiple dots");
    }

    let int_part: u128 = parts[0]
        .parse()
        .context("Invalid integer part of TON amount")?;
    let mut nanoton = int_part
        .checked_mul(1_000_000_000)
        .ok_or_else(|| anyhow::anyhow!("TON amount too large"))?;

    if parts.len() == 2 {
        let mut frac_str = parts[1].to_string();
        if frac_str.len() > 9 {
            frac_str.truncate(9);
        }
        let frac_val: u128 = frac_str
            .parse()
            .context("Invalid fractional part of TON amount")?;
        let multiplier = 10u128.pow(9 - frac_str.len() as u32);
        nanoton += frac_val * multiplier;
    }

    Ok(nanoton)
}

#[allow(clippy::too_many_arguments)]
fn save_library(
    contract_name: &str,
    hash: &str,
    code: &str,
    account: &str,
    duration: u64,
    network: String,
    bits: u64,
    cells: u64,
    local: bool,
    global: bool,
) -> anyhow::Result<()> {
    let is_global = if global {
        true
    } else if local {
        false
    } else {
        let options = vec![
            "Local (libraries.toml)",
            "Global (~/.acton/libraries/global.libraries.toml)",
        ];
        let selection = Select::new("Save library info to:", options).prompt()?;
        selection.starts_with("Global")
    };

    let config_path = if is_global {
        let global_dir = global_libraries_path()
            .ok_or_else(|| anyhow!("Could not determine global libraries path"))?
            .parent()
            .ok_or_else(|| anyhow!("Invalid global libraries path"))?
            .to_path_buf();

        fs::create_dir_all(&global_dir)?;
        global_dir.join("global.libraries.toml")
    } else {
        PathBuf::from("libraries.toml")
    };

    let mut doc = if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        content.parse::<DocumentMut>()?
    } else {
        DocumentMut::new()
    };

    if !doc.contains_key("libraries") {
        doc.insert("libraries", Item::Table(Table::new()));
    }

    let libraries = doc["libraries"]
        .as_table_mut()
        .ok_or_else(|| anyhow!("Invalid libraries.toml format"))?;

    let mut final_name = contract_name.to_string();
    if libraries.contains_key(&final_name) {
        let mut i = 1;
        while libraries.contains_key(&format!("{contract_name}-{i}")) {
            i += 1;
        }
        final_name = format!("{contract_name}-{i}");
    }

    let mut lib_table = Table::new();
    lib_table.insert("name", value(contract_name));
    lib_table.insert("hash", value(hash));
    lib_table.insert("code", value(code));
    lib_table.insert("account", value(account));
    lib_table.insert("duration", value(duration as i64));
    lib_table.insert("network", value(network));
    lib_table.insert("timestamp", value(Local::now().to_rfc3339()));
    lib_table.insert("bits", value(bits as i64));
    lib_table.insert("cells", value(cells as i64));

    libraries.insert(&final_name, Item::Table(lib_table));

    fs::write(config_path, doc.to_string())?;

    if final_name != contract_name {
        println!(
            "  {} Library with ID '{}' already exists, saved as '{}'",
            "ℹ".blue().bold(),
            contract_name,
            final_name
        );
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

    let mut compiler = tolkc::Compiler::new(2);
    let acton_config = ActonConfig::load().ok();
    if let Some(config) = &acton_config
        && let Some(mappings) = &config.mappings
    {
        compiler.mappings = mappings.clone().into_iter().collect();
    }

    let compilation_result = compiler.compile(tmp_file_path.as_ref(), true);
    match compilation_result {
        CompilerResult::Success(result) => Ok(ArcCell::from_boc_b64(&result.code_boc64)?),
        CompilerResult::Error(err) => {
            anyhow::bail!("Unable to compile librarian: {}", err.message);
        }
    }
}
