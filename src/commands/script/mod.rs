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
use num_bigint::BigInt;
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
use tolk_compiler::abi::{ABIFunctionParameter, ContractABI as CompilerContractABI, Ty};
use tolk_syntax::ast::expressions::parse_tolk_int_literal;
use ton_abi::{ContractAbi, contract_abi};
use ton_api::Network;
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{
    AccountsState, LocalAccountsState, RemoteAccountState, RemoteSnapshotCache, WorldState,
};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, GetMethodResultSuccess, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use tvm_ffi::serde::serialize_tuple;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Store};
use tycho_types::models::{
    Base64StdAddrFlags, DisplayBase64StdAddr, StateInit, StdAddr, StdAddrFormat,
};

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
    tonconnect: bool,
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

    let (network, fork_net) = resolve_script_networks(net.as_deref(), fork_net.as_deref())?;
    if tonconnect && network.is_none() {
        anyhow::bail!("`--tonconnect` requires `--net mainnet` or `--net testnet`");
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
        &content,
        mappings.as_ref(),
        args,
        verbose,
        debug,
        backtrace,
        debug_listener,
        fork_net,
        fork_block_number,
        network,
        explorer,
        show_bodies,
        tonconnect,
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
    content: &str,
    mappings: Option<&BTreeMap<String, String>>,
    args: Vec<String>,
    verbose: u8,
    debug: bool,
    backtrace: Option<BacktraceMode>,
    debug_listener: Option<TcpListener>,
    fork_net: Option<Network>,
    fork_block_number: Option<u64>,
    net: Option<Network>,
    explorer: Option<Explorer>,
    show_bodies: bool,
    tonconnect: bool,
) -> anyhow::Result<()> {
    let mappings = mappings.cloned();
    let abi = contract_abi(content.into(), file_path, mappings.as_ref());

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
            let stack = parse_script_stack_args(result.abi.as_ref(), &args)?;
            let source_map = Arc::new(result.source_map.unwrap_or_default());
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
                tonconnect,
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
    compiler_abi: Option<Arc<CompilerContractABI>>,
}

#[allow(clippy::too_many_arguments)]
fn execute_script(
    code_cell: &Cell,
    data_cell: &Cell,
    stack: Tuple,
    abi: Arc<ContractAbi>,
    compiler_abi: Option<Arc<CompilerContractABI>>,
    source_map: Arc<SourceMap>,
    debug: bool,
    backtrace: Option<BacktraceMode>,
    debug_listener: Option<TcpListener>,
    verbosity: ExecutorVerbosity,
    fork_net: Option<Network>,
    fork_block_number: Option<u64>,
    net: Option<&Network>,
    explorer: Option<Explorer>,
    show_bodies: bool,
    tonconnect: bool,
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
    let tonconnect = if tonconnect {
        let network = net.expect("`--tonconnect` must be validated before script execution");
        let session = Arc::new(TonConnectSession::start()?);
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
            project_root: project_root().to_path_buf(),
            abi,
            source_map: Some(source_map.clone()),
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
    let custom_exit_code_info = if FormatterContext::is_special_get_method_exit_code(exit_code) {
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

fn parse_script_stack_args(
    compiler_abi: Option<&CompilerContractABI>,
    args: &[String],
) -> anyhow::Result<Tuple> {
    let Some(compiler_abi) = compiler_abi else {
        if args.is_empty() {
            return Ok(Tuple::empty());
        }
        anyhow::bail!("Cannot parse script arguments: missing ABI");
    };
    let Some(main) = compiler_abi
        .get_methods
        .iter()
        .find(|method| method.tvm_method_id == 0)
    else {
        if args.is_empty() {
            return Ok(Tuple::empty());
        }
        anyhow::bail!("Cannot parse script arguments: main function ABI was not found");
    };

    let expected_count = main.parameters.len();
    if args.len() != expected_count {
        anyhow::bail!(
            "Wrong number of arguments: expected {}, got {}",
            expected_count,
            args.len()
        );
    }

    let mut items = Vec::with_capacity(args.len());
    for (param, arg) in main.parameters.iter().zip(args) {
        items.push(parse_script_parameter(param, arg)?);
    }

    Ok(Tuple(items))
}

fn parse_script_parameter(param: &ABIFunctionParameter, raw: &str) -> anyhow::Result<TupleItem> {
    validate_script_arg_ty(&param.ty)
        .and_then(|()| parse_script_value(&param.ty, raw))
        .map_err(|err| match err {
            ScriptArgParseError::Invalid => {
                anyhow!(
                    "Cannot parse argument {} as {}: {}",
                    param.name.yellow(),
                    param.ty.to_string().yellow(),
                    format_arg_value(raw).yellow()
                )
            }
            ScriptArgParseError::Unsupported { kind, ty }
                if unsupported_type_message_needs_name(&ty) =>
            {
                anyhow!(
                    "Argument {} has unsupported {} type {}",
                    param.name.yellow(),
                    kind.yellow(),
                    ty.yellow()
                )
            }
            ScriptArgParseError::Unsupported { kind, .. } => {
                anyhow!(
                    "Argument {} has unsupported {} type",
                    param.name.yellow(),
                    kind.yellow()
                )
            }
        })
}

#[derive(Debug)]
enum ScriptArgParseError {
    Invalid,
    Unsupported { kind: &'static str, ty: String },
}

fn validate_script_arg_ty(ty: &Ty) -> Result<(), ScriptArgParseError> {
    match ty {
        Ty::Nullable { inner, .. } | Ty::ArrayOf { inner } => validate_script_arg_ty(inner),
        Ty::Int
        | Ty::IntN { .. }
        | Ty::UintN { .. }
        | Ty::VarintN { .. }
        | Ty::VaruintN { .. }
        | Ty::Coins
        | Ty::BitsN { .. }
        | Ty::Bool
        | Ty::Cell
        | Ty::CellOf { .. }
        | Ty::Slice
        | Ty::String
        | Ty::Address
        | Ty::AddressExt
        | Ty::AddressOpt
        | Ty::NullLiteral => Ok(()),
        _ => Err(unsupported_ty(ty)),
    }
}

fn parse_script_value(ty: &Ty, raw: &str) -> Result<TupleItem, ScriptArgParseError> {
    let trimmed = raw.trim();
    match ty {
        Ty::String if !trimmed.starts_with('"') => string_tuple_item(raw),
        Ty::Nullable { .. } if trimmed == "null" => Ok(TupleItem::Null),
        Ty::Nullable { inner, .. } => parse_script_value(inner, raw),
        _ => {
            let mut parser = ScriptArgParser::new(raw);
            let item = parser.parse_value(ty)?;
            parser.skip_ws();
            if parser.is_eof() {
                Ok(item)
            } else {
                Err(ScriptArgParseError::Invalid)
            }
        }
    }
}

struct ScriptArgParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> ScriptArgParser<'a> {
    const fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    const fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.rest().chars().next()
            && ch.is_whitespace()
        {
            self.pos += ch.len_utf8();
        }
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.input.as_bytes().get(self.pos).copied() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_value(&mut self, ty: &Ty) -> Result<TupleItem, ScriptArgParseError> {
        self.skip_ws();
        match ty {
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins => self.parse_int(),
            Ty::Bool => self.parse_bool(),
            Ty::Cell | Ty::CellOf { .. } => self.parse_cell().map(TupleItem::Cell),
            Ty::Slice | Ty::BitsN { .. } => self.parse_cell().map(TupleItem::Slice),
            Ty::String => self.parse_string(),
            Ty::Address | Ty::AddressExt => self.parse_address(),
            Ty::AddressOpt => {
                if self.consume_exact_token("null") {
                    Ok(TupleItem::Null)
                } else {
                    self.parse_address()
                }
            }
            Ty::NullLiteral => {
                if self.consume_exact_token("null") {
                    Ok(TupleItem::Null)
                } else {
                    Err(ScriptArgParseError::Invalid)
                }
            }
            Ty::ArrayOf { inner } => self.parse_array(inner),
            _ => Err(unsupported_ty(ty)),
        }
    }

    fn parse_int(&mut self) -> Result<TupleItem, ScriptArgParseError> {
        let token = self.parse_token()?;
        if token == "NaN" {
            return Ok(TupleItem::Nan);
        }
        let value = parse_number(token).ok_or(ScriptArgParseError::Invalid)?;
        Ok(TupleItem::Int(value))
    }

    fn parse_bool(&mut self) -> Result<TupleItem, ScriptArgParseError> {
        let token = self.parse_token()?;
        match token {
            "true" => Ok(TupleItem::Int(BigInt::from(-1))),
            "false" => Ok(TupleItem::Int(BigInt::from(0))),
            _ => Err(ScriptArgParseError::Invalid),
        }
    }

    fn parse_cell(&mut self) -> Result<Cell, ScriptArgParseError> {
        let token = self.parse_token()?;
        Boc::decode_hex(token).map_err(|_| ScriptArgParseError::Invalid)
    }

    fn parse_string(&mut self) -> Result<TupleItem, ScriptArgParseError> {
        if self.rest().starts_with('"') {
            let value = self.parse_quoted_string()?;
            string_tuple_item(&value)
        } else {
            let value = self.parse_token()?;
            string_tuple_item(value)
        }
    }

    fn parse_address(&mut self) -> Result<TupleItem, ScriptArgParseError> {
        let token = self.parse_token()?;
        address_tuple_item(token)
    }

    fn parse_array(&mut self, inner: &Ty) -> Result<TupleItem, ScriptArgParseError> {
        if !self.consume_byte(b'[') {
            return Err(ScriptArgParseError::Invalid);
        }

        let mut items = Vec::new();
        loop {
            self.skip_ws();
            if self.consume_byte(b']') {
                return Ok(TupleItem::Tuple(Tuple(items)));
            }
            if self.is_eof() {
                return Err(ScriptArgParseError::Invalid);
            }

            items.push(self.parse_value(inner)?);

            self.skip_ws();
            self.consume_byte(b',');
        }
    }

    fn parse_quoted_string(&mut self) -> Result<String, ScriptArgParseError> {
        if !self.consume_byte(b'"') {
            return Err(ScriptArgParseError::Invalid);
        }

        let mut value = String::new();
        while !self.is_eof() {
            let ch = self.next_char().ok_or(ScriptArgParseError::Invalid)?;
            match ch {
                '"' => return Ok(value),
                '\n' | '\r' => return Err(ScriptArgParseError::Invalid),
                '\\' => {
                    let escaped = self.next_char().ok_or(ScriptArgParseError::Invalid)?;
                    match escaped {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '0' => value.push('\0'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        other => value.push(other),
                    }
                }
                other => value.push(other),
            }
        }

        Err(ScriptArgParseError::Invalid)
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.rest().chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn parse_token(&mut self) -> Result<&'a str, ScriptArgParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.rest().chars().next() {
            if ch.is_whitespace() || matches!(ch, ',' | ']') {
                break;
            }
            self.pos += ch.len_utf8();
        }
        if self.pos == start {
            Err(ScriptArgParseError::Invalid)
        } else {
            Ok(&self.input[start..self.pos])
        }
    }

    fn consume_exact_token(&mut self, expected: &str) -> bool {
        let start = self.pos;
        match self.parse_token() {
            Ok(token) if token == expected => true,
            _ => {
                self.pos = start;
                false
            }
        }
    }
}

fn parse_number(raw: &str) -> Option<BigInt> {
    let (negative, raw) = raw
        .strip_prefix('-')
        .map_or((false, raw), |raw| (true, raw));
    let literal = parse_tolk_int_literal(raw)?;
    let mut value = BigInt::parse_bytes(literal.digits().as_bytes(), literal.radix())?;
    if negative {
        value = -value;
    }
    Some(value)
}

fn string_tuple_item(value: &str) -> Result<TupleItem, ScriptArgParseError> {
    let mut tuple = Tuple::empty();
    tuple.push_string_slice(value);
    tuple.0.pop().ok_or(ScriptArgParseError::Invalid)
}

fn address_tuple_item(value: &str) -> Result<TupleItem, ScriptArgParseError> {
    let (addr, _) = StdAddr::from_str_ext(
        ffi::emulation::normalize_address_input(value),
        StdAddrFormat::any(),
    )
    .map_err(|_| ScriptArgParseError::Invalid)?;
    let mut builder = CellBuilder::new();
    addr.store_into(&mut builder, Cell::empty_context())
        .map_err(|_| ScriptArgParseError::Invalid)?;
    builder
        .build()
        .map(TupleItem::Slice)
        .map_err(|_| ScriptArgParseError::Invalid)
}

fn unsupported_ty(ty: &Ty) -> ScriptArgParseError {
    ScriptArgParseError::Unsupported {
        kind: unsupported_ty_kind(ty),
        ty: ty.to_string(),
    }
}

fn unsupported_ty_kind(ty: &Ty) -> &'static str {
    match ty {
        Ty::AliasRef { alias_name, .. } if alias_name == "tuple" => "tuple",
        Ty::AliasRef { alias_name, .. } if alias_name == "dict" => "dict",
        Ty::AliasRef { .. } => "alias",
        Ty::StructRef { .. } => "struct",
        Ty::Union { .. } => "union",
        Ty::MapKV { .. } => "map",
        Ty::LispListOf { .. } => "lisp list",
        Ty::Tensor { .. } | Ty::ShapedTuple { .. } => "tuple",
        Ty::GenericT { .. } => "generic",
        Ty::Callable => "continuation",
        Ty::Builder => "builder",
        Ty::AddressAny => "any_address",
        Ty::Remaining => "remaining",
        Ty::Void => "void",
        Ty::EnumRef { .. } => "enum",
        Ty::Unknown => "unknown",
        _ => "argument",
    }
}

fn unsupported_type_message_needs_name(ty: &str) -> bool {
    !matches!(
        ty,
        "builder" | "dict" | "tuple" | "continuation" | "unknown" | "void"
    )
}

fn format_arg_value(raw: &str) -> String {
    const MAX_LEN: usize = 120;

    let value = format!("{raw:?}");
    if value.chars().count() <= MAX_LEN {
        value
    } else {
        let shortened = value.chars().take(MAX_LEN).collect::<String>();
        format!("{shortened}...")
    }
}
