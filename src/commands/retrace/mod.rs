use crate::commands::common::error_fmt;
use crate::formatter::FormatterContext;
use crate::stdlib;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, project_root as configured_project_root};
use acton_debug::commands::retrace::dap::serve_retrace_dap;
use acton_debug::replayer::TolkReplayer;
use acton_debug::retrace as debug_retrace;
use anyhow::{Context, anyhow};
use retrace::{ComputeInfo, Network, retrace};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::{IntAddr, OutAction, RelaxedMsgInfo};

struct ContractTraceArtifacts {
    code_cell: Cell,
    tolk_source_map: tolkc::TolkSourceMap,
}

#[allow(unsafe_code)]
pub fn retrace_cmd(
    hash: String,
    net: Option<String>,
    api_key: Option<String>,
    verbose: bool,
    logs_dir: Option<String>,
    contract: Option<String>,
    dap_port: Option<u16>,
) -> anyhow::Result<()> {
    if let Some(key) = api_key {
        // SAFETY: this is a single thread program
        unsafe {
            std::env::set_var("TONCENTER_API_KEY", key);
        }
    }

    if dap_port.is_some() && contract.is_none() {
        anyhow::bail!(
            "{} requires {}",
            "--dap-port".yellow(),
            "--contract <NAME>".yellow()
        );
    }

    let contract_artifacts = if let Some(contract_name) = contract.as_deref() {
        Some(build_contract_trace_artifacts(contract_name)?)
    } else {
        None
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let networks = if let Some(net_str) = net {
        vec![Network::from_str(&net_str)?]
    } else {
        vec![Network::Mainnet, Network::Testnet]
    };

    let mut last_error = None;
    for network in networks {
        let retrace_future = retrace(network.clone(), &hash, HashMap::new());
        match rt.block_on(retrace_future) {
            Ok(result) => {
                if let Some(logs_dir) = &logs_dir {
                    fs::create_dir_all(logs_dir)?;
                    fs::write(
                        format!("{logs_dir}/vm.log"),
                        result.emulated_tx.vm_logs.as_ref(),
                    )?;
                    fs::write(
                        format!("{logs_dir}/executor.log"),
                        result.emulated_tx.executor_logs.as_ref(),
                    )?;
                    println!("{} Logs saved to {}", "Success:".green(), logs_dir);
                }
                print_retrace_result(network, &result, verbose, logs_dir.as_ref());

                if let (Some(contract_name), Some(artifacts)) =
                    (contract.as_deref(), contract_artifacts.as_ref())
                {
                    ensure_contract_matches_transaction(contract_name, &result, artifacts)?;

                    if let Some(port) = dap_port {
                        let replayer = debug_retrace::build_tolk_replayer(
                            &result.emulated_tx.vm_logs,
                            &artifacts.tolk_source_map,
                        )
                        .ok_or_else(|| {
                            anyhow!(
                                "Cannot build replayer for contract {}",
                                contract_name.yellow()
                            )
                        })?;

                        serve_retrace_dap(replayer, port)
                            .map_err(|err| anyhow!(err.to_string()))?;
                    }
                }
                return Ok(());
            }
            Err(e) => {
                last_error = Some(e);
            }
        }
    }

    if let Some(e) = last_error {
        anyhow::bail!("Failed to retrace transaction in any network: {e}");
    }
    anyhow::bail!("Failed to retrace transaction");
}

#[allow(dead_code)]
pub(crate) fn serve_prepared_retrace_dap(replayer: TolkReplayer, port: u16) -> anyhow::Result<()> {
    serve_retrace_dap(replayer, port).map_err(|err| anyhow!(err.to_string()))
}

fn print_retrace_result(
    network: Network,
    result: &retrace::TraceResult,
    verbose: bool,
    logs_dir: Option<&String>,
) {
    let tx = &result.emulated_tx;
    let money = &result.money;

    let compute_success = match &tx.compute_info {
        ComputeInfo::Success {
            success, exit_code, ..
        } => *success && (*exit_code == 0 || *exit_code == 1),
        ComputeInfo::Skipped => false,
    };

    let (action_success, action_exit_code) =
        if let Ok(tycho_types::models::TxInfo::Ordinary(desc)) = tx.raw.load_info() {
            if let Some(action) = &desc.action_phase {
                (action.success, action.result_code)
            } else {
                (true, 0)
            }
        } else {
            (true, 0)
        };

    let is_success = compute_success && action_success;

    println!("{:<20} {}", "Network:".dimmed(), network);
    println!(
        "{:<20} {}",
        "State Hash OK:".dimmed(),
        if result.state_update_hash_ok {
            "Yes".green().to_string()
        } else {
            "No".bright_red().to_string()
        }
    );

    println!("\n{}", "Transaction Details:".bold());
    let status_text = if is_success {
        format!(
            "{} {}",
            "Success".green(),
            "(exit code: 0)".to_string().dimmed()
        )
    } else if !compute_success {
        let exit_code = match &tx.compute_info {
            ComputeInfo::Success { exit_code, .. } => *exit_code,
            _ => 0,
        };
        format!(
            "{} {}",
            "Failed on compute phase".bright_red(),
            format!("(exit code: {exit_code})").dimmed()
        )
    } else {
        format!(
            "{} {}",
            "Failed on action phase".bright_red(),
            format!("(exit code: {action_exit_code})").dimmed()
        )
    };

    println!("  {:<15} {}", "Status:".dimmed(), status_text);
    println!(
        "  {:<15} {}",
        "Account:".dimmed(),
        format_address(result.in_msg.contract.clone()).cyan()
    );
    if let Some(sender) = &result.in_msg.sender {
        println!(
            "  {:<15} {}",
            "Sender:".dimmed(),
            format_address(sender.clone()).cyan()
        );
    } else {
        println!("  {:<15} External", "Sender:".dimmed());
    }
    println!("  {:<15} {}", "LT:".dimmed(), tx.lt);
    println!(
        "  {:<15} {}",
        "Time:".dimmed(),
        chrono::DateTime::from_timestamp(tx.utime as i64, 0).map_or_else(
            || tx.utime.to_string(),
            |d| d.format("%d.%m.%Y, %H:%M:%S").to_string()
        )
    );

    if let Some(amount) = result.in_msg.amount {
        println!(
            "  {:<15} {}",
            "Amount In:".dimmed(),
            format_tokens(amount).white()
        );
    }

    println!("\n{}", "Fees & Balance:".bold());
    println!(
        "  {:<15} {}",
        "Balance Before:".dimmed(),
        format_tokens(money.balance_before).white()
    );
    println!(
        "  {:<15} {}",
        "Amount Sent:".dimmed(),
        format_tokens(money.sent_total).white()
    );
    println!(
        "  {:<15} {}",
        "Total Fee:".dimmed(),
        format_tokens(money.total_fees).white()
    );
    if let ComputeInfo::Success { gas_fees, .. } = tx.compute_info {
        println!(
            "  {:<15} {}",
            "Gas Fee:".dimmed(),
            format_tokens(gas_fees).white()
        );
    }
    println!(
        "  {:<15} {}",
        "Balance After:".dimmed(),
        format_tokens(money.balance_after).white()
    );

    println!("\n{}", "Compute Phase:".bold());
    match &tx.compute_info {
        ComputeInfo::Skipped => println!("  {:<15} Skipped", "Status:".dimmed()),
        ComputeInfo::Success {
            success,
            exit_code,
            vm_steps,
            gas_used,
            gas_fees,
            ..
        } => {
            println!(
                "  {:<15} {}",
                "Success:".dimmed(),
                if *success {
                    "Yes".bright_green().to_string()
                } else {
                    "No".bright_red().to_string()
                }
            );
            println!("  {:<15} {}", "Exit Code:".dimmed(), exit_code);
            println!("  {:<15} {}", "VM Steps:".dimmed(), vm_steps);
            println!("  {:<15} {}", "Gas Used:".dimmed(), gas_used);
            println!(
                "  {:<15} {}",
                "Gas Fees:".dimmed(),
                format_tokens(*gas_fees).white()
            );
        }
    }

    println!("\n{}", "Action Phase:".bold());
    println!(
        "  {:<15} {}",
        "Success:".dimmed(),
        if action_success {
            "Yes".green().to_string()
        } else {
            "No".bright_red().to_string()
        }
    );
    if !action_success {
        println!("  {:<15} {}", "Exit Code:".dimmed(), action_exit_code);
    }
    println!("  {:<15} {}", "Total Actions:".dimmed(), tx.actions.len());

    let mut showed_hashes = false;

    if !tx.actions.is_empty() {
        println!("\n{}", "Out Actions:".bold());
        for (i, action) in tx.actions.iter().enumerate() {
            println!("  {}. {}", (i + 1).bold(), format_action_title(action));

            match action {
                OutAction::SendMsg { mode, out_msg } => {
                    println!(
                        "     {:<15} {}",
                        "Mode:".dimmed(),
                        FormatterContext::format_send_msg_flags(*mode).yellow()
                    );
                    if let Ok(msg) = out_msg.load() {
                        match &msg.info {
                            RelaxedMsgInfo::Int(int_info) => {
                                println!(
                                    "     {:<15} {}",
                                    "Destination:".dimmed(),
                                    format_address(int_info.dst.clone()).cyan()
                                );
                                println!(
                                    "     {:<15} {}",
                                    "Value:".dimmed(),
                                    format_tokens(u128::from(int_info.value.tokens) as u64).white()
                                );
                                if int_info.bounce {
                                    println!(
                                        "     {:<15} {}",
                                        "Bounce:".dimmed(),
                                        "Yes".bright_green()
                                    );
                                }
                            }
                            RelaxedMsgInfo::ExtOut(ext_info) => {
                                println!(
                                    "     {:<15} {}",
                                    "Destination:".dimmed(),
                                    ext_info
                                        .dst
                                        .as_ref()
                                        .map_or_else(|| "External".to_string(), ToString::to_string)
                                        .cyan()
                                );
                            }
                        }
                        if verbose {
                            println!(
                                "     {:<15} {}",
                                "Body:".dimmed(),
                                Boc::encode_hex(msg.body.1.clone()).yellow()
                            );
                        } else {
                            println!(
                                "     {:<15} {}",
                                "Body Hash:".dimmed(),
                                format!("0x{}", hex::encode(msg.body.1.hash(0))).dimmed()
                            );
                            showed_hashes = true;
                        }
                    } else {
                        println!("     {}", "Failed to load message details".bright_red());
                    }
                }
                OutAction::SetCode { new_code } => {
                    if verbose {
                        println!(
                            "     {:<15} {}",
                            "New Code:".dimmed(),
                            Boc::encode_hex(new_code.clone()).yellow()
                        );
                    } else {
                        println!(
                            "     {:<15} {}",
                            "New Code Hash:".dimmed(),
                            format!("0x{}", hex::encode(new_code.hash(0))).yellow()
                        );
                        showed_hashes = true;
                    }
                }
                OutAction::ReserveCurrency { mode, value } => {
                    println!(
                        "     {:<15} {}",
                        "Mode:".dimmed(),
                        FormatterContext::format_reserve_currency_flags(*mode).yellow()
                    );
                    println!(
                        "     {:<15} {}",
                        "Amount:".dimmed(),
                        format_tokens(u128::from(value.tokens) as u64).white()
                    );
                }
                OutAction::ChangeLibrary { mode, lib } => {
                    let mode_str = format!("{mode:?}");
                    let clean_mode = mode_str
                        .strip_prefix("ChangeLibraryMode(")
                        .and_then(|s| s.strip_suffix(")"))
                        .unwrap_or(&mode_str);

                    println!("     {:<15} {}", "Mode:".dimmed(), clean_mode.yellow());
                    match lib {
                        tycho_types::models::LibRef::Hash(h) => {
                            let value = &h.to_string();
                            println!(
                                "     {:<15} {}",
                                "Lib Hash:".dimmed(),
                                format!("0x{value}").yellow()
                            );
                        }
                        tycho_types::models::LibRef::Cell(c) => {
                            if verbose {
                                println!(
                                    "     {:<15} {}",
                                    "Lib Cell:".dimmed(),
                                    Boc::encode_hex(c.clone()).yellow()
                                );
                            } else {
                                println!(
                                    "     {:<15} {}",
                                    "Lib Hash:".dimmed(),
                                    format!("0x{}", hex::encode(c.hash(0))).yellow()
                                );
                                showed_hashes = true;
                            }
                        }
                    }
                }
            }
            println!(); // extra line between actions
        }
    }

    if showed_hashes && !verbose {
        println!("Help: Some fields are shown as hashes. Use --verbose to see full cell content.");
    }

    if let Some(opcode) = result.in_msg.opcode {
        println!("\n{}", "Message Data:".bold());
        println!("  {:<15} 0x{:08x}", "Opcode:".dimmed(), opcode);
        println!();
    }

    if logs_dir.is_none() {
        println!("Help: Use --logs-dir <DIR> to save full VM and executor logs to files.");
    }
}

fn build_contract_trace_artifacts(contract_name: &str) -> anyhow::Result<ContractTraceArtifacts> {
    stdlib::ensure_latest(configured_project_root())?;

    let acton_config = ActonConfig::load()?;
    let contract = acton_config
        .get_contract(contract_name)
        .cloned()
        .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&acton_config, contract_name)))?;
    let contract_path = resolve_project_config_path(configured_project_root(), &contract.src);

    if contract_path.extension().and_then(|ext| ext.to_str()) != Some("tolk") {
        anyhow::bail!(
            "Contract {} uses {} source. Source-level retrace requires a {} contract.",
            contract_name.yellow(),
            contract_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("unknown")
                .yellow(),
            ".tolk".yellow()
        );
    }

    let mappings = acton_config.mappings();
    let compiler = tolkc::Compiler::new(2).with_mappings(&mappings);
    let compilation_result = compiler.compile(&contract_path, true);

    match compilation_result {
        tolkc::CompilerResult::Success(res) => {
            let code_cell = Boc::decode_base64(res.code_boc64)
                .with_context(|| "Failed to decode compiled contract code BoC".to_string())?;
            let source_map = res.new_source_map.ok_or_else(|| {
                anyhow!(
                    "Compiler did not return source maps for {}",
                    contract_path.display()
                )
            })?;

            let debug_mark_base64 = res.debug_mark_base64.ok_or_else(|| {
                anyhow!(
                    "Compiler did not return debug marks for {}",
                    contract_path.display()
                )
            })?;

            let tolk_source_map = tolkc::TolkSourceMap::from_code_cell(
                source_map,
                &code_cell,
                Some(&debug_mark_base64),
            )?;

            Ok(ContractTraceArtifacts {
                code_cell,
                tolk_source_map,
            })
        }
        tolkc::CompilerResult::Error(error) => {
            anyhow::bail!(
                "Failed to compile contract {} for source-level retrace: {}",
                contract_name.yellow(),
                error.message.trim_end()
            );
        }
    }
}

fn ensure_contract_matches_transaction(
    contract_name: &str,
    result: &retrace::TraceResult,
    artifacts: &ContractTraceArtifacts,
) -> anyhow::Result<()> {
    let Some(tx_code_hash) = result
        .code_cell
        .as_ref()
        .or(result.original_code_cell.as_ref())
        .map(|cell| cell.repr_hash())
    else {
        return Ok(());
    };

    let local_code_hash = artifacts.code_cell.repr_hash();
    if local_code_hash == tx_code_hash {
        return Ok(());
    }

    anyhow::bail!(
        "Contract {} does not match code of account {}: local hash {}, transaction hash {}",
        contract_name.yellow(),
        format_address(result.in_msg.contract.clone()).cyan(),
        local_code_hash.to_string().yellow(),
        tx_code_hash.to_string().yellow()
    );
}

fn resolve_project_config_path(project_root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn format_tokens(nanotons: u64) -> String {
    format!("{:.9} TON", nanotons as f64 / 1_000_000_000.0)
}

fn format_action_title(action: &OutAction) -> String {
    use tycho_types::models::OutAction;
    match action {
        OutAction::SendMsg { .. } => "Send Message".bright_blue().to_string(),
        OutAction::SetCode { .. } => "Set Code".bright_magenta().to_string(),
        OutAction::ReserveCurrency { .. } => "Reserve".bright_green().to_string(),
        OutAction::ChangeLibrary { .. } => "Change Library".bright_cyan().to_string(),
    }
}

fn format_address(addr: IntAddr) -> String {
    if let IntAddr::Std(addr) = addr {
        return addr.display_base64_url(false).to_string();
    }

    addr.to_string()
}
