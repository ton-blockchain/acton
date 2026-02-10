use crate::commands::common::error_fmt;
use crate::context::{
    AssertFailure, AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx,
    EmulationsState, Env, IoContext, KnownAddresses,
};
use crate::debugger::any_executor::AnyExecutor;
use crate::debugger::debug_context::DebugContext;
use crate::file_build_cache::FileBuildCache;
use crate::formatter::FormatterContext;
use crate::wallets;
use crate::{ffi, stdlib};
use acton_config::config::{ActonConfig, Explorer};
use anyhow::anyhow;
use log::error;
use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::UNIX_EPOCH;
use ton_abi::{ContractAbi, contract_abi};
use ton_api::Network;
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{
    AccountsState, LocalAccountsState, RemoteAccountState, RemoteSnapshotCache, WorldState,
};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use ton_source_map::SourceMap;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::{Tuple, TupleItem};
use vmlogs::parser::{CellLike, VmStackValue, vm_stack_value};

#[allow(clippy::too_many_arguments)]
pub fn script_cmd(
    path: &String,
    args: Vec<String>,
    debug: bool,
    debug_port: u16,
    clear_cache: bool,
    fork_net: Option<String>,
    api_key: Option<String>,
    fork_block_number: Option<u64>,
    broadcast: bool,
    net: Option<String>,
    explorer: Option<Explorer>,
) -> anyhow::Result<()> {
    stdlib::ensure_latest(Path::new("."))?;

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

    if let Some(net) = &net {
        Network::from_str(net)?; // validate network
    }

    let content = fs::read_to_string(path).map_err(|err| {
        anyhow!(color_print::cformat!(
            "Cannot access <yellow>{path}</>: {err}"
        ))
    })?;

    let stack = parse_stack_args(args)?;

    run_script_file(
        path,
        &content,
        stack,
        debug,
        debug_port,
        fork_net,
        api_key,
        fork_block_number,
        broadcast,
        net,
        explorer,
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
    stack: Tuple,
    debug: bool,
    debug_port: u16,
    fork_net: Option<String>,
    api_key: Option<String>,
    fork_block_number: Option<u64>,
    broadcast: bool,
    net: Option<String>,
    explorer: Option<Explorer>,
) -> anyhow::Result<()> {
    let acton_config = ActonConfig::load();
    let mappings = if let Ok(config) = &acton_config {
        &config.mappings
    } else {
        &None
    };
    let abi = contract_abi(content, file_path, mappings);

    let mut compiler = tolkc::Compiler::new(2);
    if let Ok(config) = &acton_config {
        compiler = compiler.with_mappings(&config.mappings);
    }

    match compiler.compile(Path::new(file_path), debug) {
        tolkc::CompilerResult::Success(result) => {
            let code_cell = ArcCell::from_boc_b64(&result.code_boc64)?;
            let data_cell = ArcCell::default();

            execute_script(
                &code_cell,
                &data_cell,
                stack,
                &abi,
                &result.source_map.unwrap_or_default(),
                debug,
                debug_port,
                ExecutorVerbosity::FullLocationStackVerbose,
                fork_net,
                api_key,
                fork_block_number,
                broadcast,
                net,
                explorer,
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
    stack: Tuple,
    abi: &ContractAbi,
    source_map: &SourceMap,
    debug: bool,
    debug_port: u16,
    verbosity: ExecutorVerbosity,
    fork_net: Option<String>,
    api_key: Option<String>,
    fork_block_number: Option<u64>,
    broadcast: bool,
    net: Option<String>,
    explorer: Option<Explorer>,
) -> anyhow::Result<()> {
    let dest_address = contract_address(code_cell)?;

    let now = std::time::SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let params = RunGetMethodArgs {
        code: code_cell.to_boc_b64(false)?,
        data: data_cell.to_boc_b64(false)?,
        verbosity,
        libs: String::new(),
        address: dest_address.to_string(),
        unixtime: duration_since_epoch.as_secs().try_into()?,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id: 0,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let config_b64: Option<&str> = None;
    let fork_net = fork_net.as_deref().map(Network::from_str).transpose()?;

    let mut emulator = Emulator::new(verbosity, config_b64)?;
    let resolver = match &fork_net {
        Some(net) => AccountsState::Remote(RemoteAccountState::new(
            net.clone(),
            fork_block_number,
            api_key.clone(),
            RemoteSnapshotCache::new(),
        )),
        None => AccountsState::Local(LocalAccountsState::new()),
    };
    let mut world_state = WorldState::new(resolver, config_b64)?;
    let mut build_cache = BuildCache::new();
    let mut file_build_cache = FileBuildCache::new(None)?;
    let mut known_addresses = KnownAddresses::new();
    let mut known_code_cell = HashMap::new();
    let mut emulations = EmulationsState::new();

    let mut assert_failure = None;
    let mut expected_exit_code = None;

    let config = ActonConfig::load()?;
    let network = net.as_deref().map(Network::from_str).transpose()?;
    let open_wallets = wallets::open_wallets(&config, network.as_ref(), broadcast)?;

    let mut ctx = Context {
        env: Env {
            config: &config,
            abi,
            default_log_level: verbosity,
            wallets: config.wallets.as_ref(),
            open_wallets,
            build_override: BTreeMap::new(),
            explorer,
            fork_net,
            api_key,
            running_id: "script".to_owned(),
        },
        io: IoContext {
            stdout_buffer: String::new(),
            stderr_buffer: String::new(),
            capture_output: false,
        },
        asserts: AssertsContext {
            assert_failure: &mut assert_failure,
            expected_exit_code: &mut expected_exit_code,
        },
        chain: ChainContext {
            world_state: &mut world_state,
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
        network,
    };

    if debug {
        let stack = serialize_tuple(&stack)?.to_boc_b64(false)?;
        let mut executor = StepGetExecutor::new(&stack, &params, Some(DEFAULT_CONFIG))?;
        ffi::register(&mut executor, &mut ctx);

        let transport = crate::debugger::start_dap_server(debug_port);

        let mut dbg_ctx = DebugContext::new(
            transport,
            AnyExecutor::Get(executor.clone()),
            source_map,
            "main".to_string(),
        );

        ctx.debug = DebugCtx::new(&mut dbg_ctx);

        executor.prepare(0, &stack)?;

        ctx.debug.ctx().process_incoming_requests(true)?;

        let result = executor.finish(&params.code)?;
        print_script_result(&mut ctx, ScriptResult { result });
        return Ok(());
    }

    let mut executor = GetExecutor::new(&params)?;
    ffi::register(&mut executor, &mut ctx);

    let stack = serialize_tuple(&stack)?.to_boc_b64(false)?;
    let result = executor.run_get_method(&stack, &params, Some(DEFAULT_CONFIG))?;
    print_script_result(&mut ctx, ScriptResult { result });
    Ok(())
}

fn print_script_result(ctx: &mut Context<'_>, result: ScriptResult) {
    match &result.result {
        GetMethodResult::Success(success_result) => {
            let exit_code = success_result.vm_exit_code;

            if exit_code != 0
                && let Some(assert_failure) = ctx.asserts.assert_failure
            {
                if let AssertFailure::WalletNotFound(failure) = assert_failure {
                    let message = AssertFailure::format_wallet_not_found_message(failure, &ctx.env);
                    let highlighted_message = FormatterContext::highlight_actual_expected(&message);
                    eprintln!("{} {}", "Error:".bright_red(), highlighted_message);

                    if let Some(location) = &failure.location
                        && !location.is_empty()
                    {
                        println!("{} at {}", "└─".dimmed(), location.dimmed());
                    }
                } else {
                    if let Some(message) = &assert_failure.message() {
                        if message.is_empty() {
                            println!("{}", "└─".dimmed());
                        } else {
                            let highlighted_message =
                                FormatterContext::highlight_actual_expected(message);
                            println!("{} {}", "Error:".bright_red(), highlighted_message);
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

fn parse_stack_args(args: Vec<String>) -> anyhow::Result<Tuple> {
    let mut items = Vec::new();
    for arg in args {
        let mut input = arg.as_str();
        let value = vm_stack_value(&mut input).map_err(|e| {
            error!("Failed to parse stack value '{arg}': {e}");
            anyhow!(color_print::cformat!(
                "Failed to parse argument <yellow>{arg}</>"
            ))
        })?;

        if !input.trim().is_empty() {
            return Err(anyhow!(
                "Failed to parse argument '{arg}': trailing characters"
            ));
        }

        let item = convert_vm_value_to_tuple_item(value)?;
        items.push(item);
    }
    Ok(Tuple(items).unwrap_tuple())
}

fn convert_vm_value_to_tuple_item(value: VmStackValue<'_>) -> anyhow::Result<TupleItem> {
    match value {
        VmStackValue::Null => Ok(TupleItem::Null),
        VmStackValue::NaN => Ok(TupleItem::Nan),
        VmStackValue::Integer(s) => {
            let bi = s.parse().map_err(|_| anyhow!("Invalid integer: {s}"))?;
            Ok(TupleItem::Int(bi))
        }
        VmStackValue::Tuple(values) => {
            let mut inner_items = Vec::new();
            for v in values {
                inner_items.push(convert_vm_value_to_tuple_item(v)?);
            }
            Ok(TupleItem::Tuple(Tuple(inner_items)))
        }
        VmStackValue::Cell(cell_like) => convert_cell_like(cell_like).map(TupleItem::Cell),
        VmStackValue::Builder(hex) => {
            let cell = ArcCell::from_boc_hex(hex)?;
            Ok(TupleItem::Builder(cell))
        }
        VmStackValue::CellSlice(cs) => {
            let cell = ArcCell::from_boc_hex(cs.value)?;
            Ok(TupleItem::Slice(cell))
        }
        VmStackValue::Continuation(_) => {
            Err(anyhow!("Continuation not supported in script arguments"))
        }
        VmStackValue::String(s) => Ok(TupleItem::Cell(string_to_slice(s)?)),
        VmStackValue::Unknown => Err(anyhow!("Unknown stack value type")),
    }
}

fn convert_cell_like(cell_like: CellLike<'_>) -> anyhow::Result<ArcCell> {
    match cell_like {
        CellLike::Cell(hex) => Ok(ArcCell::from_boc_hex(hex)?),
        CellLike::Builder(hex) => Ok(ArcCell::from_boc_hex(hex)?),
    }
}

fn string_to_slice(s: &str) -> anyhow::Result<ArcCell> {
    let bytes = s.as_bytes();
    let total_bits = bytes.len() * 8;

    if total_bits <= 1023 {
        // Fast path, the string fits in one cell
        let mut b = CellBuilder::new();
        b.store_bits(total_bits, bytes)?;
        return Ok(b.build()?.to_arc());
    }

    let mut remaining_bytes = bytes;
    let mut cell_data = Vec::new();

    while !remaining_bytes.is_empty() {
        let chunk_size = std::cmp::min(remaining_bytes.len(), 127); // 127 bytes = 1016 bits < 1023
        let chunk = &remaining_bytes[..chunk_size];
        cell_data.push((chunk, chunk.len() * 8));
        remaining_bytes = &remaining_bytes[chunk_size..];
    }

    // build cells from last to first
    let mut next_cell: Option<ArcCell> = None;

    for (chunk, bits) in cell_data.into_iter().rev() {
        let mut b = CellBuilder::new();
        b.store_bits(bits, chunk)?;

        if let Some(next) = next_cell {
            b.store_reference(&next)?;
        }

        next_cell = Some(ArcCell::from(b.build()?));
    }

    if let Some(root_cell) = next_cell {
        return Ok(root_cell);
    }

    anyhow::bail!("No root cell for string");
}
