use crate::commands::common::error_fmt;
use crate::config::ActonConfig;
use crate::context::{
    AssertFailure, AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx,
    Emulations, Env, IoContext, KnownAddresses,
};
use crate::debugger::debug_context::DebugContext;
use crate::ffi;
use crate::file_build_cache::FileBuildCache;
use crate::formatter::FormatterContext;
use crate::wallets;
use abi::{ContractAbi, contract_abi};
use anyhow::anyhow;
use emulator::AnyExecutor;
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::executor::ExecutorVerbosity;
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use tolkc::source_map::SourceMap;
use ton_api::Network;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::Tuple;

#[allow(clippy::too_many_arguments)]
pub fn script_cmd(
    path: &String,
    debug: bool,
    debug_port: u16,
    clear_cache: bool,
    fork_net: Option<String>,
    api_key: Option<String>,
    broadcast: bool,
    net: String,
) -> anyhow::Result<()> {
    if clear_cache {
        let mut file_cache = FileBuildCache::new(None)?;
        file_cache.clear()?;
        println!("  {} Cache cleared", "✓".green().bold());
    }

    if !fs::exists(path).unwrap_or(false) {
        anyhow::bail!(error_fmt::file_not_found(path));
    }

    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        anyhow::bail!("{} is not a file", path.yellow());
    }

    if !path.ends_with(".tolk") {
        anyhow::bail!("Script file must end with {}", ".tolk".yellow());
    }

    Network::from_str(&net)?; // validate network

    let content = fs::read_to_string(path).map_err(|err| {
        anyhow!(color_print::cformat!(
            "Cannot access <yellow>{path}</>: {err}"
        ))
    })?;
    run_script_file(
        path, &content, debug, debug_port, fork_net, api_key, broadcast, net,
    )
}

/// A script is essentially a regular smart contract with a `main` function,
/// which serves as an alias for the `onInternalMessage` function with ID=0.
///
/// Executing the script means calling the get-method with ID=0 and an empty stack,
/// so the `main` function takes no arguments.
#[allow(clippy::too_many_arguments)]
fn run_script_file(
    file_path: &str,
    content: &str,
    debug: bool,
    debug_port: u16,
    fork_net: Option<String>,
    api_key: Option<String>,
    broadcast: bool,
    net: String,
) -> anyhow::Result<()> {
    let abi = contract_abi(content, file_path);

    match tolkc::compile(Path::new(file_path), debug) {
        tolkc::CompilerResult::Success(result) => {
            let code_cell = ArcCell::from_boc_b64(&result.code_boc64)?;
            let data_cell = ArcCell::default();

            execute_script(
                &code_cell,
                &data_cell,
                &abi,
                &result.source_map.unwrap_or(Default::default()),
                debug,
                debug_port,
                ExecutorVerbosity::FullLocationStackVerbose,
                fork_net,
                api_key,
                broadcast,
                net,
            )?;
            Ok(())
        }
        tolkc::CompilerResult::Error(error) => {
            anyhow::bail!("Cannot compile script file {}", error.message)
        }
    }
}

struct ScriptResult {
    result: GetMethodResult,
}

#[allow(clippy::too_many_arguments)]
fn execute_script(
    code_cell: &ArcCell,
    data_cell: &ArcCell,
    abi: &ContractAbi,
    source_map: &SourceMap,
    debug: bool,
    debug_port: u16,
    verbosity: ExecutorVerbosity,
    fork_net: Option<String>,
    api_key: Option<String>,
    broadcast: bool,
    net: String,
) -> anyhow::Result<()> {
    let dest_address = contract_address(code_cell)?;

    let params = GetMethodParams {
        code: code_cell.to_boc_b64(false)?.to_string(),
        data: data_cell.to_boc_b64(false)?.to_string(),
        verbosity,
        libs: "".to_string(),
        address: dest_address.to_string(),
        unixtime: 0,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id: 0,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let mut emulator = Emulator::new(verbosity);
    let mut blockchain = Blockchain::new(fork_net.clone(), api_key.clone());
    let mut build_cache = BuildCache::new();
    let mut file_build_cache =
        FileBuildCache::new(None).expect("Failed to create file cache for script execution");
    let mut known_addresses = KnownAddresses::new();
    let mut known_code_cell = HashMap::new();
    let mut emulations = Emulations::new();

    let mut assert_failure = None;
    let mut expected_exit_code = None;

    let config = ActonConfig::load()?;
    let open_wallets = wallets::open_wallets(&config, &net, broadcast)?;

    let mut ctx = Context {
        env: Env {
            config: &config,
            abi,
            default_log_level: verbosity,
            wallets: config.wallets.as_ref(),
            open_wallets,
            build_override: BTreeMap::new(),
        },
        io: IoContext {
            stdout_buffer: "".to_string(),
            stderr_buffer: "".to_string(),
            capture_output: false,
        },
        asserts: AssertsContext {
            assert_failure: &mut assert_failure,
            expected_exit_code: &mut expected_exit_code,
        },
        chain: ChainContext {
            blockchain: &mut blockchain,
            emulator: &mut emulator,
            emulations: &mut emulations,
        },
        build: BuildContext {
            build_cache: &mut build_cache,
            file_build_cache: &mut file_build_cache,
            known_addresses: &mut known_addresses,
            known_code_cells: &mut known_code_cell,
            need_debug_info: false,
            backtrace: None,
        },
        debug: DebugCtx::Disabled,
        is_broadcasting: broadcast,
        network: net,
    };

    if debug {
        let mut executor = StepGetExecutor::new(Tuple::empty(), params.clone());
        ffi::register(&mut executor, &mut ctx);

        let transport = crate::debugger::start_dap_server(debug_port);

        let mut dbg_ctx = DebugContext::new(
            transport,
            AnyExecutor::Get(executor.clone()),
            source_map,
            "main".to_string(),
        );

        ctx.debug = DebugCtx::new(&mut dbg_ctx);

        executor.prepare(0, Tuple::empty());

        ctx.debug.ctx().process_incoming_requests(true)?;

        let result = executor.finish(&params.code);
        print_script_result(&mut ctx, ScriptResult { result });
        return Ok(());
    }

    let mut executor = GetExecutor::new(params.clone());
    ffi::register(&mut executor, &mut ctx);

    let result = executor.run_get_method(Tuple::empty(), params, None);
    print_script_result(&mut ctx, ScriptResult { result });
    Ok(())
}

fn print_script_result(ctx: &mut Context, result: ScriptResult) {
    match &result.result {
        GetMethodResult::Success(success_result) => {
            let exit_code = success_result.vm_exit_code;

            if exit_code != 0
                && let Some(assert_failure) = ctx.asserts.assert_failure
            {
                match assert_failure {
                    AssertFailure::WalletNotFound(failure) => {
                        let message =
                            AssertFailure::format_wallet_not_found_message(failure, &ctx.env);
                        let highlighted_message =
                            FormatterContext::highlight_actual_expected(&message);
                        eprintln!("{} {}", "Error:".bright_red(), highlighted_message);

                        if let Some(location) = &failure.location
                            && !location.is_empty()
                        {
                            println!("{} at {}", "└─".dimmed(), location.dimmed());
                        }
                    }
                    _ => {
                        if let Some(message) = &assert_failure.message() {
                            if !message.is_empty() {
                                let highlighted_message =
                                    FormatterContext::highlight_actual_expected(message);
                                println!("{} {}", "Error:".bright_red(), highlighted_message);
                            } else {
                                println!("{}", "└─".dimmed());
                            }
                        } else {
                            println!("{}", "└─".dimmed());
                        }

                        if let Some(location) = &assert_failure.location()
                            && !location.is_empty()
                        {
                            println!("{} at {}", "└─".dimmed(), location.dimmed());
                        }
                    }
                }
            }

            std::process::exit(exit_code);
        }
        GetMethodResult::Error(error) => {
            println!("{} {}", "Execution error:".red(), error.error.red());
            std::process::exit(1);
        }
    }
}

fn contract_address(code: &ArcCell) -> anyhow::Result<TonAddress> {
    let state_init = CellBuilder::new()
        .store_bit(false)
        .map_err(|e| anyhow!("Failed to store bounce flag: {e}"))?
        .store_bit(false)
        .map_err(|e| anyhow!("Failed to store maybe libraries: {e}"))?
        .store_ref_cell_optional(Some(code))
        .map_err(|e| anyhow!("Failed to store code cell: {e}"))?
        .store_ref_cell_optional(Some(&ArcCell::default()))
        .map_err(|e| anyhow!("Failed to store data cell: {e}"))?
        .store_bit(false)
        .map_err(|e| anyhow!("Failed to store maybe tick/tock: {e}"))?
        .build()
        .map_err(|e| anyhow!("Failed to build state init cell: {e}"))?;

    let dest_address = TonAddress::new(0, state_init.cell_hash());
    Ok(dest_address)
}
