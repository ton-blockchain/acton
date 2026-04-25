use crate::commands::common::{
    error_fmt, executor_verbosity_for_cli_level, max_executor_verbosity,
};
use crate::context::{
    AssertFailure, AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx,
    EmulationsState, Env, IoContext, KnownAddresses,
};
use crate::file_build_cache::FileBuildCache;
use crate::formatter::FormatterContext;
use crate::retrace;
use crate::wallets;
use crate::{ffi, stdlib};
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, Explorer, project_root};
use acton_config::test::BacktraceMode;
use acton_debug::exit_codes;
use acton_debug::replayer::TolkReplayer;
use acton_debug::{ReplayerDebugSession, reserve_dap_listener, start_dap_server_with_listener};
use anyhow::anyhow;
use log::error;
use rustc_hash::FxHashMap;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::io::{Write, stderr, stdout};
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tolkc::TolkSourceMap;
use tolkc::abi::ContractABI as CompilerContractABI;
use ton_abi::{ContractAbi, contract_abi};
use ton_api::Network;
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{
    AccountsState, LocalAccountsState, RemoteAccountState, RemoteSnapshotCache, WorldState,
};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, GetMethodResultSuccess, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, HashBytes};
use tycho_types::models::{Base64StdAddrFlags, DisplayBase64StdAddr, StateInit, StdAddr};
use vmlogs::parser::{CellLike, VmStackValue, vm_stack_value};

const ASSERTION_FAILED_EXIT_CODE: i32 = 567;
const CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT: i32 = 678;
const CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE: i32 = 679;

fn resolve_script_networks(
    net: Option<&str>,
    fork_net: Option<&str>,
) -> anyhow::Result<(Option<Network>, Option<Network>)> {
    let net = net.map(Network::from_str).transpose()?;
    let fork_net = fork_net.map(Network::from_str).transpose()?;

    if let (Some(net), Some(fork_net)) = (&net, &fork_net)
        && net != fork_net
    {
        anyhow::bail!(
            "`--net` ({net}) and `--fork-net` ({fork_net}) cannot differ when broadcasting; use one network or omit `--fork-net`"
        );
    }

    let fork_net = if fork_net.is_none() {
        net.clone()
    } else {
        fork_net
    };

    Ok((net, fork_net))
}

#[allow(clippy::too_many_arguments)]
pub fn script_cmd(
    path: &String,
    args: Vec<String>,
    verbose: u8,
    debug: bool,
    backtrace: Option<BacktraceMode>,
    debug_port: u16,
    clear_cache: bool,
    fork_net: Option<String>,
    fork_block_number: Option<u64>,
    net: Option<String>,
    explorer: Option<Explorer>,
    show_bodies: bool,
) -> anyhow::Result<()> {
    let project_root = project_root().to_path_buf();
    stdlib::ensure_latest(&project_root)?;
    let mappings = match ActonConfig::load() {
        Ok(config) => config.mappings(),
        Err(e) => {
            eprintln!("  {} Failed to load Acton.toml: {e:#}", "⚠".yellow().bold());
            None
        }
    };

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

    let content = fs::read_to_string(path)
        .map_err(|err| anyhow!("Cannot access {}: {err}", path.yellow()))?;

    let stack = parse_stack_args(args)?;

    let (network, fork_net) = resolve_script_networks(net.as_deref(), fork_net.as_deref())?;
    let debug_listener = if debug {
        Some(reserve_dap_listener(debug_port)?)
    } else {
        None
    };

    run_script_file(
        path,
        &content,
        &mappings,
        stack,
        verbose,
        debug,
        backtrace,
        debug_listener,
        fork_net,
        fork_block_number,
        network,
        explorer,
        show_bodies,
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
    mappings: &Option<BTreeMap<String, String>>,
    stack: Tuple,
    verbose: u8,
    debug: bool,
    backtrace: Option<BacktraceMode>,
    debug_listener: Option<TcpListener>,
    fork_net: Option<Network>,
    fork_block_number: Option<u64>,
    net: Option<Network>,
    explorer: Option<Explorer>,
    show_bodies: bool,
) -> anyhow::Result<()> {
    let abi = contract_abi(content.into(), file_path, mappings);

    let compiler = tolkc::Compiler::new(2).with_mappings(mappings);
    let need_debug_info = debug || backtrace == Some(BacktraceMode::Full);
    let mut verbosity = executor_verbosity_for_cli_level(verbose);

    if debug || backtrace == Some(BacktraceMode::Full) {
        verbosity = max_executor_verbosity(verbosity, ExecutorVerbosity::FullLocationStackVerbose);
    }

    match compiler.compile(Path::new(file_path), need_debug_info) {
        tolkc::CompilerResult::Success(result) => {
            let code_cell = Boc::decode_base64(&result.code_boc64)?;
            let data_cell = CellBuilder::new().build()?;
            let source_map = Arc::new(TolkSourceMap::from_code_cell(
                result.new_source_map.unwrap_or_default(),
                &code_cell,
                result.debug_mark_base64.as_deref(),
            )?);
            execute_script(
                &code_cell,
                &data_cell,
                stack,
                Arc::new(abi),
                result.abi.map(Arc::new),
                source_map,
                debug,
                backtrace,
                debug_listener,
                verbosity,
                fork_net,
                fork_block_number,
                net.as_ref(),
                explorer,
                show_bodies,
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
    source_map: Arc<TolkSourceMap>,
    compiler_abi: Option<Arc<CompilerContractABI>>,
}

#[allow(clippy::too_many_arguments)]
fn execute_script(
    code_cell: &Cell,
    data_cell: &Cell,
    stack: Tuple,
    abi: Arc<ContractAbi>,
    compiler_abi: Option<Arc<CompilerContractABI>>,
    source_map: Arc<TolkSourceMap>,
    debug: bool,
    backtrace: Option<BacktraceMode>,
    debug_listener: Option<TcpListener>,
    verbosity: ExecutorVerbosity,
    fork_net: Option<Network>,
    fork_block_number: Option<u64>,
    net: Option<&Network>,
    explorer: Option<Explorer>,
    show_bodies: bool,
) -> anyhow::Result<()> {
    let broadcast = net.is_some();
    let dest_address = contract_address(code_cell)?;
    let formatted_address = format_std_address(&dest_address, net);

    let now = std::time::SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let params = RunGetMethodArgs {
        code: Boc::encode_base64(code_cell),
        data: Boc::encode_base64(data_cell),
        verbosity,
        libs: String::new(),
        address: formatted_address,
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

    let mut emulator = Emulator::new(verbosity, config_b64)?;
    let resolver = match &fork_net {
        Some(net) => AccountsState::Remote(RemoteAccountState::new(
            net.clone(),
            fork_block_number,
            RemoteSnapshotCache::new(),
        )),
        None => AccountsState::Local(LocalAccountsState::new()),
    };
    let mut world_state = WorldState::new(resolver, config_b64)?;
    let mut build_cache = BuildCache::new();
    let mut file_build_cache = FileBuildCache::new(None)?;
    let mut known_addresses = KnownAddresses::new();
    let mut known_code_cell = FxHashMap::default();
    let mut emulations = EmulationsState::new();

    let mut assert_failure = None;
    let mut expected_exit_code = None;

    let config = ActonConfig::load()?;
    let open_wallets = wallets::open_wallets(&config, net, broadcast)?;

    let mut ctx = Context {
        env: Env {
            config: &config,
            project_root: project_root().to_path_buf(),
            abi,
            show_bodies,
            default_log_level: verbosity,
            wallets: config.wallets.as_ref(),
            open_wallets,
            build_override: BTreeMap::new(),
            explorer,
            fork_net,
            running_id: "script".into(),
            // The script's own compiled code contains any user-defined predicate
            // lambdas (e.g. those built by `expect(...).toHaveTx({ ... })`), so
            // we reuse it as the code cell for evaluating predicate continuations.
            test_code: Some(code_cell.clone()),
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
        message_iters: Default::default(),
        build: BuildContext {
            build_cache: &mut build_cache,
            file_build_cache: &mut file_build_cache,
            known_addresses: &mut known_addresses,
            known_code_cells: &mut known_code_cell,
            need_debug_info: debug || backtrace == Some(BacktraceMode::Full),
            backtrace,
        },
        debug: DebugCtx::Disabled,
        is_broadcasting: broadcast,
        network: net.cloned(),
    };

    let stack_b64 = Boc::encode_base64(serialize_tuple(&stack)?);

    if debug {
        let mut executor = StepGetExecutor::new(&stack_b64, &params, Some(DEFAULT_CONFIG))?;
        ffi::register(&mut executor, &mut ctx);

        let listener = debug_listener
            .ok_or_else(|| anyhow!("internal error: debug listener was not reserved"))?;
        let transport = start_dap_server_with_listener(listener)?;
        executor.prepare(0, &stack_b64)?;
        let mut replayer = TolkReplayer::new_live_vm(source_map.as_ref(), executor.clone().into())?;
        replayer.set_compiler_abi(compiler_abi.clone());

        let mut dbg_session = ReplayerDebugSession::new(transport, replayer, "main".into());
        ctx.debug = DebugCtx::new(&mut dbg_session);
        if ctx.debug.process_incoming_requests(true)? {
            return Ok(());
        }

        let result = executor.finish(&params.code)?;
        print_script_result(
            &ctx,
            ScriptResult {
                result,
                source_map,
                compiler_abi,
            },
        );
        return Ok(());
    }

    let mut executor = GetExecutor::new(&params)?;
    ffi::register(&mut executor, &mut ctx);
    let result = executor.run_get_method(&stack_b64, &params, Some(DEFAULT_CONFIG))?;

    print_script_result(
        &ctx,
        ScriptResult {
            result,
            source_map,
            compiler_abi,
        },
    );
    Ok(())
}

fn print_script_result<'a>(ctx: &'a Context<'a>, result: ScriptResult) {
    match &result.result {
        GetMethodResult::Success(success_result) => {
            let exit_code = success_result.vm_exit_code;

            if exit_code != 0 {
                print_nonzero_script_exit_code(ctx, success_result, &result, exit_code);

                if let Some(assert_failure) = ctx.asserts.assert_failure.as_ref() {
                    let formatter = FormatterContext::from_context(ctx);

                    if let AssertFailure::WalletNotFound(failure) = assert_failure {
                        let message = formatter.format_wallet_not_found_message(failure);
                        let highlighted_message =
                            FormatterContext::highlight_actual_expected(&message);
                        eprintln!("{} {}", "Error:".bright_red(), highlighted_message);

                        if let Some(location) = &failure.location {
                            println!("{} at {}", "└─".dimmed(), location.format().dimmed());
                        }
                    } else {
                        let detailed_message = formatter
                            .format_detailed_assert_failure(assert_failure, ctx.env.abi.clone());

                        if detailed_message.is_empty() {
                            println!("{}", "└─".dimmed());
                        } else {
                            println!("{detailed_message}");
                        }
                    }
                }

                let _ = stdout().flush();
                let _ = stderr().flush();
            }

            std::process::exit(i32::from(exit_code != 0));
        }
        GetMethodResult::Error(error) => {
            println!("{} {}", "Execution error:".red(), error.error.red());
            let _ = stdout().flush();
            let _ = stderr().flush();
            std::process::exit(1);
        }
    }
}

fn print_nonzero_script_exit_code<'a>(
    ctx: &'a Context<'a>,
    result: &GetMethodResultSuccess,
    script_result: &ScriptResult,
    exit_code: i32,
) {
    if exit_code == ASSERTION_FAILED_EXIT_CODE {
        return;
    }

    println!(
        "Script finished with exit code {}",
        exit_code.to_string().yellow(),
    );

    let details = format_nonzero_script_exit_code_details(ctx, result, script_result, exit_code);
    if !details.is_empty() {
        println!("{details}");
    }
}

fn format_nonzero_script_exit_code_details<'a>(
    ctx: &'a Context<'a>,
    result: &GetMethodResultSuccess,
    script_result: &ScriptResult,
    exit_code: i32,
) -> String {
    let formatter = FormatterContext::from_context(ctx);
    let mut details = String::new();
    let exit_code_info = retrace::find_exception_info(&result.vm_log, &script_result.source_map);
    let custom_exit_code_info = if matches!(
        exit_code,
        CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT
            | CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE
    ) {
        None
    } else {
        FormatterContext::find_custom_exit_code_info(
            exit_code,
            Some(ctx.env.abi.as_ref()),
            script_result.compiler_abi.as_deref(),
        )
    };

    if let Some(info) = &exit_code_info {
        writeln!(
            details,
            "at {}",
            FormatterContext::format_location(&info.loc)
        )
        .ok();

        let backtrace_lines = FormatterContext::format_backtrace(&info.backtrace);
        if !backtrace_lines.is_empty() {
            writeln!(details, "Backtrace:").ok();
            for line in backtrace_lines {
                writeln!(details, "  {line}").ok();
            }
        }

        if !info.description.is_empty() && custom_exit_code_info.is_none() {
            writeln!(details, "Description: {}", info.description.dimmed()).ok();
        }
    }

    if let Some(info) = exit_codes::find(exit_code) {
        let should_show_fallback_description = exit_code_info
            .as_ref()
            .is_none_or(|exception| exception.description.is_empty());
        if should_show_fallback_description {
            writeln!(details, "Description: {}", info.description.dimmed()).ok();
        }
        writeln!(details, "Phase: {}", info.phase.dimmed()).ok();
    } else if let Some(info) = custom_exit_code_info {
        writeln!(details, "Description: {}", info.description.dimmed()).ok();
        if info.symbolic_name != info.description {
            writeln!(details, "Error: {}", info.symbolic_name.dimmed()).ok();
        }
        writeln!(details, "Phase: {}", "Compute phase".dimmed()).ok();
    }

    if exit_code == CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT {
        writeln!(
            details,
            "Cannot run method of not deployed contract, make sure you're deployed contract first or passed {}",
            "--fork-net".yellow(),
        )
        .ok();
    } else if exit_code == CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE {
        writeln!(details, "Cannot run method of contract without code").ok();
    }

    if formatter.backtrace.is_none() {
        writeln!(
            details,
            "Re-run with {} to get more information",
            "--backtrace full".yellow()
        )
        .ok();
    }

    let details = details.trim();
    if details.is_empty() {
        String::new()
    } else {
        details
            .lines()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn contract_address(code: &Cell) -> anyhow::Result<StdAddr> {
    let state_init = StateInit {
        split_depth: None,
        special: None,
        code: Some(code.clone()),
        data: Some(CellBuilder::new().build()?),
        libraries: Default::default(),
    };

    let state_init_cell = CellBuilder::build_from(state_init)?;
    Ok(StdAddr::new(
        0,
        HashBytes(*state_init_cell.repr_hash().as_array()),
    ))
}

fn format_std_address(address: &StdAddr, network: Option<&Network>) -> String {
    let testnet = network.is_some_and(Network::uses_testnet_address_format);
    DisplayBase64StdAddr {
        addr: address,
        flags: Base64StdAddrFlags {
            testnet,
            base64_url: true,
            bounceable: true,
        },
    }
    .to_string()
}

fn parse_stack_args(args: Vec<String>) -> anyhow::Result<Tuple> {
    let mut items = Vec::new();
    for arg in args {
        let mut input = arg.as_str();
        let value = vm_stack_value(&mut input).map_err(|e| {
            error!("Failed to parse stack value '{arg}': {e}");
            anyhow!("Failed to parse argument {}", arg.yellow())
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

fn convert_vm_value_to_tuple_item(value: VmStackValue) -> anyhow::Result<TupleItem> {
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
            let cell = Boc::decode_hex(hex)?;
            Ok(TupleItem::Builder(cell))
        }
        VmStackValue::CellSlice(cs) => {
            let cell = Boc::decode_hex(cs.value)?;
            Ok(TupleItem::Slice(cell))
        }
        VmStackValue::Continuation(_) => {
            Err(anyhow!("Continuation not supported in script arguments"))
        }
        VmStackValue::String(s) => Ok(TupleItem::Cell(string_to_slice(&s)?)),
        VmStackValue::Unknown => Err(anyhow!("Unknown stack value type")),
    }
}

fn convert_cell_like(cell_like: CellLike) -> anyhow::Result<Cell> {
    match cell_like {
        CellLike::Cell(hex) | CellLike::Builder(hex) => Ok(Boc::decode_hex(hex)?),
    }
}

fn string_to_slice(s: &str) -> anyhow::Result<Cell> {
    let bytes = s.as_bytes();
    let total_bits = bytes.len() * 8;

    if total_bits <= 1023 {
        // Fast path, the string fits in one cell
        let mut b = CellBuilder::new();
        b.store_raw(bytes, total_bits as u16)?;
        return Ok(b.build()?);
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
    let mut next_cell: Option<Cell> = None;

    for (chunk, bits) in cell_data.into_iter().rev() {
        let mut b = CellBuilder::new();
        b.store_raw(chunk, bits as u16)?;

        if let Some(next) = next_cell {
            b.store_reference(next)?;
        }

        next_cell = Some(b.build()?);
    }

    if let Some(root_cell) = next_cell {
        return Ok(root_cell);
    }

    anyhow::bail!("No root cell for string");
}
