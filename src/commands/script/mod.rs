use crate::commands::abi_args::parse_main_stack_args;
use crate::commands::common::{
    error_fmt, executor_verbosity_for_cli_level, max_executor_verbosity,
};
use crate::context::{
    AssertFailure, AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx,
    EmulationsState, Env, ExecutionMode, IoContext, KnownAddresses,
};
use crate::file_build_cache::FileBuildCache;
use crate::formatter::FormatterContext;
use crate::retrace;
use crate::tonconnect::{TonConnectContext, TonConnectSession};
use crate::wallets;
use crate::{ffi, stdlib};
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, Explorer, project_root};
use acton_config::test::BacktraceMode;
use acton_debug::exit_codes;
use acton_debug::replayer::TolkReplayer;
use acton_debug::{ReplayerDebugSession, reserve_dap_listener, start_dap_server_with_listener};
use anyhow::anyhow;
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
use tolk_compiler::SourceMap;
use tolk_compiler::abi::ContractABI;
use ton_api::Network;
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{
    AccountsState, LocalAccountsState, RemoteAccountState, RemoteLibraryCache, RemoteSnapshotCache,
    WorldState,
};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, GetMethodResultSuccess, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use tvm_ffi::serde::serialize_tuple;
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, HashBytes};
use tycho_types::models::{Base64StdAddrFlags, DisplayBase64StdAddr, StateInit, StdAddr};

const ASSERTION_FAILED_EXIT_CODE: i32 = 567;

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
            "{} ({net}) and {} ({fork_net}) cannot differ when broadcasting; use one network or omit {}",
            "--net".yellow(),
            "--fork-net".yellow(),
            "--fork-net".yellow()
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
    fork_cache_enabled: bool,
    net: Option<String>,
    explorer: Option<Explorer>,
    show_bodies: bool,
    tonconnect: bool,
    tonconnect_port: u16,
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

    let (network, fork_net) = resolve_script_networks(net.as_deref(), fork_net.as_deref())?;
    if tonconnect && network.is_none() {
        anyhow::bail!(
            "{} requires {} or {}",
            "--tonconnect".yellow(),
            "--net mainnet".yellow(),
            "--net testnet".yellow()
        );
    }
    if tonconnect && let Some(network) = &network {
        crate::tonconnect::ensure_supported_network(network)?;
    }
    let debug_listener = if debug {
        Some(reserve_dap_listener(debug_port)?)
    } else {
        None
    };

    run_script_file(
        path,
        mappings.as_ref(),
        args,
        verbose,
        debug,
        backtrace,
        debug_listener,
        fork_net,
        fork_block_number,
        fork_cache_enabled,
        network,
        explorer,
        show_bodies,
        tonconnect,
        tonconnect_port,
    )
}

/// A script is essentially a regular smart contract with a `main` function,
/// which serves as an alias for the `onInternalMessage` function with ID=0.
///
/// Executing the script means calling the get-method with ID=0 and the stack
/// values provided on the command line.
#[allow(clippy::too_many_arguments)]
fn run_script_file(
    file_path: &str,
    mappings: Option<&BTreeMap<String, String>>,
    args: Vec<String>,
    verbose: u8,
    debug: bool,
    backtrace: Option<BacktraceMode>,
    debug_listener: Option<TcpListener>,
    fork_net: Option<Network>,
    fork_block_number: Option<u64>,
    fork_cache_enabled: bool,
    net: Option<Network>,
    explorer: Option<Explorer>,
    show_bodies: bool,
    tonconnect: bool,
    tonconnect_port: u16,
) -> anyhow::Result<()> {
    let mappings = mappings.cloned();

    let compiler = tolk_compiler::Compiler::new(2).with_mappings(&mappings);
    let need_debug_info = debug || backtrace == Some(BacktraceMode::Full);
    let mut verbosity = executor_verbosity_for_cli_level(verbose);

    if debug || backtrace == Some(BacktraceMode::Full) {
        verbosity = max_executor_verbosity(verbosity, ExecutorVerbosity::FullLocationStackVerbose);
    }

    match compiler.compile(Path::new(file_path), need_debug_info) {
        tolk_compiler::CompilerResult::Success(result) => {
            let code_cell = Boc::decode_base64(&result.code_boc64)?;
            let data_cell = CellBuilder::new().build()?;
            let stack = parse_main_stack_args(result.abi.as_ref(), &args)?;
            let source_map = Arc::new(result.source_map.unwrap_or_default());
            execute_script(
                Path::new(file_path),
                &code_cell,
                &data_cell,
                stack,
                result.abi.map(Arc::new),
                source_map,
                debug,
                backtrace,
                debug_listener,
                verbosity,
                fork_net,
                fork_block_number,
                fork_cache_enabled,
                net.as_ref(),
                explorer,
                show_bodies,
                tonconnect,
                tonconnect_port,
            )?;
            Ok(())
        }
        tolk_compiler::CompilerResult::Error(error) => {
            anyhow::bail!("Cannot compile script file {}", error.message)
        }
    }
}

struct ScriptResult {
    result: GetMethodResult,
    source_map: Arc<SourceMap>,
    abi: Option<Arc<ContractABI>>,
}

#[allow(clippy::too_many_arguments)]
fn execute_script(
    file_path: &Path,
    code_cell: &Cell,
    data_cell: &Cell,
    stack: Tuple,
    abi: Option<Arc<ContractABI>>,
    source_map: Arc<SourceMap>,
    debug: bool,
    backtrace: Option<BacktraceMode>,
    debug_listener: Option<TcpListener>,
    verbosity: ExecutorVerbosity,
    fork_net: Option<Network>,
    fork_block_number: Option<u64>,
    fork_cache_enabled: bool,
    net: Option<&Network>,
    explorer: Option<Explorer>,
    show_bodies: bool,
    tonconnect: bool,
    tonconnect_port: u16,
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
        Some(net) => {
            let remote = RemoteAccountState::new(
                net.clone(),
                fork_block_number,
                RemoteSnapshotCache::new(),
                RemoteLibraryCache::new(),
                fork_cache_enabled,
            );
            AccountsState::Remote(remote)
        }
        None => AccountsState::Local(LocalAccountsState::new()),
    };
    let mut world_state = WorldState::new(resolver, config_b64)?;
    let mut build_cache = BuildCache::new();
    build_cache.memoize(
        "script",
        "script",
        file_path,
        &params.code,
        *code_cell.repr_hash(),
        source_map.clone(),
        abi.clone(),
    );
    let mut file_build_cache = FileBuildCache::new(None)?;
    let mut known_addresses = KnownAddresses::new();
    let mut known_code_cell = FxHashMap::default();
    let mut emulations = EmulationsState::new();

    let mut assert_failure = None;
    let mut expected_exit_code = None;

    let config = ActonConfig::load()?;
    let current_project_root = project_root().to_path_buf();
    let tonconnect = if tonconnect {
        let network = net.expect("`--tonconnect` must be validated before script execution");
        let storage_path = crate::tonconnect::session_storage_path(&current_project_root, network)?;
        let session = Arc::new(TonConnectSession::start(tonconnect_port, storage_path)?);
        let wallet = session.connect(network)?;
        Some(TonConnectContext { session, wallet })
    } else {
        None
    };
    let open_wallets = if tonconnect.is_some() {
        BTreeMap::new()
    } else {
        wallets::open_wallets(&config, net, broadcast)?
    };

    let mut ctx = Context {
        env: Env {
            config: &config,
            project_root: current_project_root,
            abi: abi.clone(),
            source_map: source_map.clone(),
            show_bodies,
            default_log_level: verbosity,
            wallets: config.wallets.as_ref(),
            open_wallets,
            tonconnect,
            build_override: BTreeMap::new(),
            explorer,
            fork_net,
            running_id: "script".into(),
            // The script's own compiled code contains any user-defined predicate
            // lambdas (e.g. those built by `expect(...).toHaveTx({ ... })`), so
            // we reuse it as the code cell for evaluating predicate continuations.
            execution_mode: ExecutionMode::Script,
            test_code: Some(code_cell.clone()),
        },
        io: IoContext {
            stdout_buffer: String::new(),
            stderr_buffer: String::new(),
            capture_output: false,
            live_output: false,
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
        replayer.set_abi(abi.clone());

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
                abi,
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
            abi,
        },
    );
    Ok(())
}

fn print_script_result<'a>(ctx: &'a Context<'a>, result: ScriptResult) {
    match &result.result {
        GetMethodResult::Success(success_result) => {
            let exit_code = success_result.vm_exit_code;
            let has_assert_failure = ctx.asserts.assert_failure.is_some();

            if exit_code != 0 {
                print_nonzero_script_exit_code(ctx, success_result, &result, exit_code);
            }

            if has_assert_failure {
                print_script_assert_failure(ctx);
            }

            if exit_code != 0 || has_assert_failure {
                let _ = stdout().flush();
                let _ = stderr().flush();
            }

            std::process::exit(i32::from(exit_code != 0 || has_assert_failure));
        }
        GetMethodResult::Error(error) => {
            println!("{} {}", "Execution error:".red(), error.error.red());
            let _ = stdout().flush();
            let _ = stderr().flush();
            std::process::exit(1);
        }
    }
}

fn print_script_assert_failure<'a>(ctx: &'a Context<'a>) {
    let Some(assert_failure) = ctx.asserts.assert_failure.as_ref() else {
        return;
    };

    let formatter = FormatterContext::from_context(ctx);

    if let AssertFailure::WalletNotFound(failure) = assert_failure {
        let message = formatter.format_wallet_not_found_message(failure);
        let highlighted_message = FormatterContext::highlight_actual_expected(&message);
        eprintln!("{} {}", "Error:".bright_red(), highlighted_message);

        if let Some(location) = &failure.location {
            println!("{} at {}", "└─".dimmed(), location.format().dimmed());
        }
    } else {
        let detailed_message = formatter.format_detailed_assert_failure(assert_failure);

        if detailed_message.is_empty() {
            println!("{}", "└─".dimmed());
        } else {
            println!("{detailed_message}");
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
    let custom_exit_code_info = if FormatterContext::is_special_get_method_exit_code(exit_code) {
        None
    } else {
        FormatterContext::find_custom_exit_code_info(exit_code, script_result.abi.as_deref())
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

    if let Some(info) = exit_codes::find_for_phase(exit_code, exit_codes::ExitCodePhase::Compute) {
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

    if let Some(message) = FormatterContext::special_get_method_exit_code_message(exit_code) {
        writeln!(details, "{message}").ok();
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
