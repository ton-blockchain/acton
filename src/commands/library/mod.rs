use crate::commands::common::{error_fmt, select_contract, select_wallet};
use crate::commands::disasm::disasm_cmd;
use crate::external_send::{SendBocContext, format_send_boc_error};
use crate::wallets::open_wallets;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, global_libraries_path, project_root};
use anyhow::{Context, anyhow};
use chrono::{DateTime, Local};
use inquire::{Select, Text};
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use tasm::printer::FormatOptions;
use tempfile::TempDir;
use tolkc::CompilerResult;
use toml_edit::{DocumentMut, Item, Table, value};
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton_api::{Network, TonApiClient};
use tycho_types::boc::Boc;
use tycho_types::boc::BocRepr;
use tycho_types::cell::{Cell, CellBuilder, CellImpl, CellSliceParts, HashBytes};
use tycho_types::models::{
    Base64StdAddrFlags, CurrencyCollection, DisplayBase64StdAddr, IntAddr, IntMsgInfo, MsgInfo,
    OwnedMessage, StateInit, StdAddr, StdAddrFormat,
};

#[allow(clippy::too_many_arguments)]
pub fn publish_cmd(
    contract: Option<String>,
    code_arg: Option<String>,
    duration_arg: Option<String>,
    wallet_name: Option<String>,
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
        let contract_path = contract.absolute_source_path(project_root());

        if contract_path.extension() != Some("tolk".as_ref()) {
            anyhow::bail!("Contract source must be a {} file", ".tolk".yellow());
        }

        contract_id = Some(contract_key.clone());

        println!("  {} Compiling contract", "→".blue().bold());
        let mappings = config.mappings();
        let compiler = tolkc::Compiler::new(2).with_mappings(&mappings);
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
        format!("0x{}", hex::encode(library_hash)).dimmed()
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
    let publisher_data = library_code_cell.clone();
    let state_init = StateInit {
        split_depth: None,
        special: None,
        code: Some(librarian_code),
        data: Some(publisher_data),
        libraries: Default::default(),
    };
    let state_init_cell = CellBuilder::build_from(&state_init)?;
    let state_init_hash = state_init_cell.repr_hash();

    let publisher_address = StdAddr::new(workchain, HashBytes(*state_init_hash.as_array()));

    println!(
        "  {} Publisher address: {}",
        "→".blue().bold(),
        format_std_address(&publisher_address, &network).dimmed()
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
        format_std_address(&wallet.address(), &network).dimmed()
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

    let config = ActonConfig::load().unwrap_or_default();
    let custom_networks = config.custom_networks();
    let api_client = TonApiClient::new(network.clone(), custom_networks)?;
    let (seqno, need_state_init) = wallet.seqno(&api_client)?;

    let expired_at_time = std::time::SystemTime::now() + std::time::Duration::from_secs(600);
    let expire_at = expired_at_time
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as u32;

    let message_info = IntMsgInfo {
        ihr_disabled: true,
        bounce: false,
        bounced: false,
        src: IntAddr::Std(wallet.address()),
        dst: IntAddr::Std(publisher_address.clone()),
        value: CurrencyCollection::new(amount_to_send_nanoton),
        ihr_fee: Default::default(),
        fwd_fee: Default::default(),
        created_at: 0,
        created_lt: 0,
    };

    let message = OwnedMessage {
        info: MsgInfo::Int(message_info),
        init: Some(state_init),
        body: CellSliceParts::from(CellBuilder::new().build()?),
        layout: None,
    };

    let message_cell_boc = BocRepr::encode(message)?;
    let message_cell = TonCell::from_boc(message_cell_boc)?;
    let external =
        wallet
            .wallet
            .create_ext_in_msg(vec![message_cell], seqno, expire_at, need_state_init)?;

    let boc = &external.to_boc_base64()?;
    let network_name = network.to_string();
    let context = SendBocContext::wallet(&wallet, &network_name, seqno, need_state_init);
    api_client
        .send_boc(boc)
        .map_err(|error| format_send_boc_error(error, context))
        .context("Failed to send publication transaction")?;

    println!("  {} Transaction sent successfully", "✓".green().bold());
    println!(
        "  {} Library should be available soon at hash: {}",
        "→".blue().bold(),
        format!("0x{}", hex::encode(library_hash)).dimmed()
    );

    save_library(
        contract_id.as_deref().unwrap_or("unknown"),
        &hex::encode(library_hash),
        &Boc::encode_base64(&library_code_cell),
        &format_std_address(&publisher_address, &network),
        duration_seconds,
        if network == Network::Localnet {
            "localnet".to_string()
        } else {
            net
        },
        bits,
        cells,
        local,
        global,
        project_root(),
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
    output: Option<String>,
    net: String,
    json: bool,
) -> anyhow::Result<()> {
    let config = ActonConfig::load().unwrap_or_default();
    let custom_networks = config.custom_networks();
    let network = Network::from_str(&net)?;
    let client = TonApiClient::new(network, custom_networks)?;

    if !json {
        println!("  {} Fetching library: 0x{hash}", "→".blue().bold());
    }

    let hash = HashBytes::from_str(&hash).context("Invalid library hash format")?;
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
            Some(net),
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

pub fn info_cmd(name: Option<String>) -> anyhow::Result<()> {
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

    let custom_networks = config.custom_networks();
    let network = Network::from_str(&lib.network.to_string())?;
    let api_client = TonApiClient::new(network, custom_networks)?;

    let last_topup_timestamp = &lib.last_topup_timestamp;
    let mut balance_u128: Option<u128> = None;
    let mut remaining_seconds: Option<u128> = None;
    let mut storage_runway_exhausted = false;

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
            let funded_seconds = (balance_u128 * 65536) / cost_per_second_x65536;
            let elapsed_seconds = elapsed_seconds_since(last_topup_timestamp).unwrap_or(0);
            storage_runway_exhausted = elapsed_seconds >= funded_seconds;
            remaining_seconds = Some(funded_seconds.saturating_sub(elapsed_seconds));
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
    println!(
        "{:<w$} {} ({})",
        "Last top-up:".dimmed(),
        last_topup_timestamp,
        format_relative_time(last_topup_timestamp),
        w = w
    );
    println!("{:<w$} {}", "Contract:".dimmed(), lib.name);
    println!("{:<w$} {}", "Network:".dimmed(), lib.network);
    println!(
        "{:<w$} {}",
        "Hash:".dimmed(),
        format!("0x{}", lib.hash).yellow()
    );
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

        if storage_runway_exhausted {
            println!(
                "  {} Storage runway is exhausted. Library may still be active, but top up urgently to avoid freeze.",
                "⚠".yellow().bold()
            );
        }
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
    let mut wallets = open_wallets(&config, Some(&network), true)?;
    let wallet = wallets
        .remove(&wallet_name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &wallet_name)))?;

    println!(
        "  {} Using wallet: {} {}",
        "→".blue().bold(),
        wallet_name.cyan(),
        format_std_address(&wallet.address(), &network).dimmed()
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

    let config = ActonConfig::load().unwrap_or_default();
    let custom_networks = config.custom_networks();
    let network_name = network.to_string();
    let api_client = TonApiClient::new(network, custom_networks)?;
    let (seqno, need_state_init) = wallet.seqno(&api_client)?;

    let expired_at_time = std::time::SystemTime::now() + std::time::Duration::from_secs(600);
    let expire_at = expired_at_time
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as u32;

    let dest_address = StdAddr::from_str_ext(&lib.account, StdAddrFormat::any())
        .with_context(|| format!("Invalid account address {}", lib.account))?
        .0;
    let message_info = IntMsgInfo {
        ihr_disabled: true,
        bounce: true,
        bounced: false,
        src: IntAddr::Std(wallet.address()),
        dst: IntAddr::Std(dest_address),
        value: CurrencyCollection::new(amount_to_send_nanoton),
        ihr_fee: Default::default(),
        fwd_fee: Default::default(),
        created_at: 0,
        created_lt: 0,
    };

    let message = OwnedMessage {
        info: MsgInfo::Int(message_info),
        init: None,
        body: Default::default(),
        layout: None,
    };

    let message_cell_boc = BocRepr::encode(message)?;
    let message_cell = TonCell::from_boc(message_cell_boc)?;
    let external =
        wallet
            .wallet
            .create_ext_in_msg(vec![message_cell], seqno, expire_at, need_state_init)?;

    println!("  {} Sending transaction...", "→".blue().bold());
    let boc = &external.to_boc_base64()?;
    let context = SendBocContext::wallet(&wallet, &network_name, seqno, need_state_init);
    api_client
        .send_boc(boc)
        .map_err(|error| format_send_boc_error(error, context))
        .context("Failed to send top-up transaction")?;

    println!(
        "  {} Top-up transaction sent successfully",
        "✓".green().bold()
    );

    let last_topup_timestamp = Local::now().to_rfc3339();
    update_library_last_topup_timestamp(project_root(), &lib_name, &last_topup_timestamp)
        .context("Top-up transaction was sent, but failed to update library metadata")?;

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

fn elapsed_seconds_since(timestamp_str: &str) -> Option<u128> {
    let dt = DateTime::parse_from_rfc3339(timestamp_str).ok()?;
    let now = Local::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() <= 0 {
        return Some(0);
    }

    Some(duration.num_seconds() as u128)
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
    project_root: &Path,
) -> anyhow::Result<()> {
    let is_global = if global {
        true
    } else if local {
        false
    } else {
        let options = vec![
            "Local (libraries.toml)",
            "Global (~/.config/acton/libraries/global.libraries.toml)",
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
        project_root.join("libraries.toml")
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
    let now = Local::now().to_rfc3339();
    lib_table.insert("name", value(contract_name));
    lib_table.insert("hash", value(hash));
    lib_table.insert("code", value(code));
    lib_table.insert("account", value(account));
    lib_table.insert("duration", value(duration as i64));
    lib_table.insert("network", value(network));
    lib_table.insert("timestamp", value(now.clone()));
    lib_table.insert("last_topup_timestamp", value(now));
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

fn update_library_last_topup_timestamp(
    project_root: &Path,
    lib_name: &str,
    timestamp: &str,
) -> anyhow::Result<()> {
    let local_path = project_root.join("libraries.toml");
    if update_library_last_topup_timestamp_in_file(&local_path, lib_name, timestamp)? {
        return Ok(());
    }

    if let Some(global_path) = global_libraries_path()
        && update_library_last_topup_timestamp_in_file(&global_path, lib_name, timestamp)?
    {
        return Ok(());
    }

    anyhow::bail!("Library '{lib_name}' metadata was not found in local/global libraries files")
}

fn update_library_last_topup_timestamp_in_file(
    path: &Path,
    lib_name: &str,
    timestamp: &str,
) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let mut doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    let Some(libraries_item) = doc.get_mut("libraries") else {
        return Ok(false);
    };
    let Some(libraries) = libraries_item.as_table_mut() else {
        return Ok(false);
    };
    let Some(lib_item) = libraries.get_mut(lib_name) else {
        return Ok(false);
    };
    let Some(lib_table) = lib_item.as_table_mut() else {
        return Ok(false);
    };

    lib_table.insert("last_topup_timestamp", value(timestamp));
    fs::write(path, doc.to_string())
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(true)
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

fn compile_librarian_with_duration(duration: u64) -> anyhow::Result<Cell> {
    let content = include_str!("librarian/librarian.tolk");
    let content = content.replace(
        "3600 * 24 * 365 * 1 // 1 year, can top-up in any time",
        &duration.to_string(),
    );
    let tmp_dir = TempDir::new()?;
    let tmp_file_path = tmp_dir.path().join("librarian.tolk");
    let mut tmp_file = File::create(&tmp_file_path)?;
    tmp_file.write_all(content.as_bytes())?;

    let acton_config = ActonConfig::load();
    let mut compiler = tolkc::Compiler::new(2);
    if let Ok(config) = &acton_config {
        let mappings = config.mappings();
        compiler = compiler.with_mappings(&mappings);
    }

    let compilation_result = compiler.compile(tmp_file_path.as_ref(), true);
    match compilation_result {
        CompilerResult::Success(result) => Ok(Boc::decode_base64(&result.code_boc64)?),
        CompilerResult::Error(err) => {
            anyhow::bail!("Unable to compile librarian: {}", err.message);
        }
    }
}

fn format_std_address(address: &StdAddr, network: &Network) -> String {
    DisplayBase64StdAddr {
        addr: address,
        flags: Base64StdAddrFlags {
            testnet: network.uses_testnet_address_format(),
            base64_url: true,
            bounceable: false,
        },
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use std::path::PathBuf;
    use std::sync::{LazyLock, Mutex};
    use tempfile::tempdir;

    static CWD_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct CurrentDirGuard {
        previous_dir: PathBuf,
    }

    impl CurrentDirGuard {
        fn new(previous_dir: PathBuf) -> Self {
            Self { previous_dir }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.previous_dir)
                .expect("must restore working directory after test");
        }
    }

    #[test]
    fn parse_ton_to_nanoton_accepts_integer_and_fractional_values() {
        assert_eq!(
            parse_ton_to_nanoton("1").expect("must parse integer TON"),
            1_000_000_000
        );
        assert_eq!(
            parse_ton_to_nanoton("1.5").expect("must parse fractional TON"),
            1_500_000_000
        );
        assert_eq!(
            parse_ton_to_nanoton("0.000000001").expect("must parse nanoton precision"),
            1
        );
    }

    #[test]
    fn parse_ton_to_nanoton_truncates_extra_fraction_digits() {
        assert_eq!(
            parse_ton_to_nanoton("1.0000000009").expect("must parse and truncate"),
            1_000_000_000
        );
    }

    #[test]
    fn parse_ton_to_nanoton_rejects_invalid_inputs() {
        let dots_err = parse_ton_to_nanoton("1.2.3").expect_err("must fail on multiple dots");
        assert!(
            dots_err
                .to_string()
                .contains("Invalid TON format: multiple dots"),
            "unexpected error: {dots_err}"
        );

        let frac_err = parse_ton_to_nanoton("1.a").expect_err("must fail on invalid fraction");
        assert!(
            frac_err
                .to_string()
                .contains("Invalid fractional part of TON amount"),
            "unexpected error: {frac_err}"
        );

        let overflow_err = parse_ton_to_nanoton("340282366920938463463374607432")
            .expect_err("must fail on multiplication overflow");
        assert!(
            overflow_err.to_string().contains("TON amount too large"),
            "unexpected error: {overflow_err}"
        );
    }

    #[test]
    fn parse_duration_accepts_supported_units() {
        assert_eq!(parse_duration("100s").expect("must parse seconds"), 100);
        assert_eq!(parse_duration("2d").expect("must parse days"), 172_800);
        assert_eq!(parse_duration("1y").expect("must parse years"), 31_536_000);
        assert_eq!(
            parse_duration("3600").expect("must parse raw seconds"),
            3600
        );
    }

    #[test]
    fn parse_duration_rejects_invalid_inputs() {
        let empty_err = parse_duration("").expect_err("must fail on empty duration");
        assert!(
            empty_err.to_string().contains("Duration cannot be empty"),
            "unexpected error: {empty_err}"
        );

        let format_err = parse_duration("1h").expect_err("must fail on unsupported unit");
        assert!(
            format_err.to_string().contains("Invalid duration format"),
            "unexpected error: {format_err}"
        );

        let number_err = parse_duration("abc").expect_err("must fail on non-numeric input");
        assert!(
            number_err.to_string().contains("Invalid duration format"),
            "unexpected error: {number_err}"
        );
    }

    #[test]
    fn format_duration_formats_common_values() {
        assert_eq!(format_duration(0), "0 second");
        assert_eq!(format_duration(62), "1 minute 2 seconds");
        assert_eq!(format_duration(3661), "1 hour 1 minute 1 second");
        assert_eq!(format_duration(31_536_000 + 86_400), "1 year 1 day");
    }

    #[test]
    fn elapsed_seconds_since_handles_invalid_future_and_past_values() {
        assert_eq!(elapsed_seconds_since("not-a-timestamp"), None);

        let future = (Local::now() + Duration::minutes(5)).to_rfc3339();
        assert_eq!(elapsed_seconds_since(&future), Some(0));

        let past = (Local::now() - Duration::minutes(2)).to_rfc3339();
        let elapsed = elapsed_seconds_since(&past).expect("must parse and compute elapsed");
        assert!(
            elapsed >= 60,
            "elapsed must be at least one minute for stable assertion: {elapsed}"
        );
    }

    #[test]
    fn format_relative_time_returns_original_for_invalid_timestamp() {
        assert_eq!(format_relative_time("bad-ts"), "bad-ts");
    }

    #[test]
    fn save_library_writes_required_fields_to_local_file() {
        let _lock = CWD_LOCK.lock().expect("must lock cwd for this test");
        let dir = tempdir().expect("must create temp dir");
        let previous_dir = std::env::current_dir().expect("must get current directory");
        std::env::set_current_dir(dir.path()).expect("must switch to temp directory");
        let _dir_guard = CurrentDirGuard::new(previous_dir);

        save_library(
            "counter",
            "deadbeef",
            "te6ccgEBAQEA",
            "EQD123",
            3600,
            "testnet".to_string(),
            834,
            5,
            true,
            false,
            dir.path(),
        )
        .expect("must save library metadata");

        let content = fs::read_to_string(dir.path().join("libraries.toml"))
            .expect("must read generated libraries.toml");
        let doc: DocumentMut = content.parse().expect("must parse generated toml");
        let libraries = doc["libraries"]
            .as_table()
            .expect("must contain [libraries] table");
        let entry = libraries
            .get("counter")
            .and_then(Item::as_table)
            .expect("must contain [libraries.counter] entry");

        assert_eq!(entry.get("name").and_then(Item::as_str), Some("counter"));
        assert_eq!(entry.get("hash").and_then(Item::as_str), Some("deadbeef"));
        assert_eq!(
            entry.get("code").and_then(Item::as_str),
            Some("te6ccgEBAQEA")
        );
        assert_eq!(entry.get("account").and_then(Item::as_str), Some("EQD123"));
        assert_eq!(entry.get("duration").and_then(Item::as_integer), Some(3600));
        assert_eq!(entry.get("network").and_then(Item::as_str), Some("testnet"));
        assert_eq!(entry.get("bits").and_then(Item::as_integer), Some(834));
        assert_eq!(entry.get("cells").and_then(Item::as_integer), Some(5));

        let timestamp = entry
            .get("timestamp")
            .and_then(Item::as_str)
            .expect("must contain timestamp");
        let last_topup = entry
            .get("last_topup_timestamp")
            .and_then(Item::as_str)
            .expect("must contain last_topup_timestamp");

        assert_eq!(
            timestamp, last_topup,
            "initial top-up timestamp must match creation timestamp"
        );
    }

    #[test]
    fn save_library_appends_suffix_for_duplicate_names() {
        let _lock = CWD_LOCK.lock().expect("must lock cwd for this test");
        let dir = tempdir().expect("must create temp dir");
        let previous_dir = std::env::current_dir().expect("must get current directory");
        std::env::set_current_dir(dir.path()).expect("must switch to temp directory");
        let _dir_guard = CurrentDirGuard::new(previous_dir);

        save_library(
            "counter",
            "hash-1",
            "te6ccgEBAQEA",
            "EQD1",
            3600,
            "testnet".to_string(),
            100,
            1,
            true,
            false,
            dir.path(),
        )
        .expect("must save first library entry");

        save_library(
            "counter",
            "hash-2",
            "te6ccgEBBgEA",
            "EQD2",
            7200,
            "testnet".to_string(),
            200,
            2,
            true,
            false,
            dir.path(),
        )
        .expect("must save second library entry with suffix");

        let content = fs::read_to_string(dir.path().join("libraries.toml"))
            .expect("must read generated libraries.toml");
        let doc: DocumentMut = content.parse().expect("must parse generated toml");
        let libraries = doc["libraries"]
            .as_table()
            .expect("must contain [libraries] table");

        assert!(libraries.contains_key("counter"), "must keep original key");
        assert!(
            libraries.contains_key("counter-1"),
            "must create suffixed key for duplicate name"
        );
    }

    #[test]
    fn update_library_last_topup_timestamp_in_file_updates_existing_entry() {
        let dir = tempdir().expect("must create temp dir");
        let path = dir.path().join("libraries.toml");
        fs::write(
            &path,
            r#"[libraries.my-lib]
name = "MyLib"
last_topup_timestamp = "2026-01-01T00:00:00Z"
"#,
        )
        .expect("must write initial toml");

        let updated =
            update_library_last_topup_timestamp_in_file(&path, "my-lib", "2026-02-01T00:00:00Z")
                .expect("must update timestamp");

        assert!(updated, "must report successful update");
        let content = fs::read_to_string(&path).expect("must read updated toml");
        assert!(
            content.contains(r#"last_topup_timestamp = "2026-02-01T00:00:00Z""#),
            "updated content: {content}"
        );
    }

    #[test]
    fn update_library_last_topup_timestamp_in_file_returns_false_for_missing_paths_or_shape() {
        let dir = tempdir().expect("must create temp dir");

        let missing = update_library_last_topup_timestamp_in_file(
            &dir.path().join("missing.toml"),
            "my-lib",
            "2026-02-01T00:00:00Z",
        )
        .expect("must handle missing file");
        assert!(!missing, "missing file should return false");

        let no_libraries = dir.path().join("no-libraries.toml");
        fs::write(
            &no_libraries,
            r#"[package]
name = "demo"
"#,
        )
        .expect("must write toml without libraries");
        let no_libs_result = update_library_last_topup_timestamp_in_file(
            &no_libraries,
            "my-lib",
            "2026-02-01T00:00:00Z",
        )
        .expect("must handle missing libraries table");
        assert!(
            !no_libs_result,
            "missing libraries table should return false"
        );

        let wrong_shape = dir.path().join("wrong-shape.toml");
        fs::write(
            &wrong_shape,
            r#"[libraries]
my-lib = "not-a-table"
"#,
        )
        .expect("must write wrong-shape toml");
        let wrong_shape_result = update_library_last_topup_timestamp_in_file(
            &wrong_shape,
            "my-lib",
            "2026-02-01T00:00:00Z",
        )
        .expect("must handle wrong item shape");
        assert!(!wrong_shape_result, "non-table entry should return false");

        let missing_entry = dir.path().join("missing-entry.toml");
        fs::write(
            &missing_entry,
            r#"[libraries.other]
name = "Other"
"#,
        )
        .expect("must write toml with different entry");
        let missing_entry_result = update_library_last_topup_timestamp_in_file(
            &missing_entry,
            "my-lib",
            "2026-02-01T00:00:00Z",
        )
        .expect("must handle missing library entry");
        assert!(
            !missing_entry_result,
            "missing target entry should return false"
        );
    }

    #[test]
    fn update_library_last_topup_timestamp_in_file_fails_on_invalid_toml() {
        let dir = tempdir().expect("must create temp dir");
        let path = dir.path().join("invalid.toml");
        fs::write(&path, "not = [valid").expect("must write invalid toml");

        let err =
            update_library_last_topup_timestamp_in_file(&path, "my-lib", "2026-02-01T00:00:00Z")
                .expect_err("must fail on invalid toml");
        assert!(
            err.to_string().contains("Failed to parse"),
            "unexpected error: {err}"
        );
    }
}
