use acton::context::{
    AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx, EmulationsState,
    Env, IoContext, KnownAddresses,
};
use acton::debugger::any_executor::AnyExecutor;
use acton::debugger::debug_context::DebugContext;
use acton::file_build_cache::FileBuildCache;
use acton::formatter::FormatterContext;
use acton::{debugger, ffi};
use acton_config::config::ActonConfig;
use dap::events::Event;
use dap::responses::ContinueResponse;
use dap::types::StackFrame;
use dap_client::DapClient;
use owo_colors::OwoColorize;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use std::{fs, thread};
use tasm::printer::FormatOptions;
use tolkc::CompilerResult;
use ton_abi::{ContractAbi, contract_abi};
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{AccountsState, LocalAccountsState, WorldState};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetMethodResult, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use ton_source_map::SourceMap;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;

mod debug_test;
mod real_test;
mod support;
mod tests;

pub(crate) struct DebuggerClient {
    client: DapClient,
}

impl DebuggerClient {
    pub(crate) fn connect(address: &str) -> anyhow::Result<Self> {
        let mut client = DapClient::connect(address)?;
        client.start()?;
        client.initialize()?;
        wait_for_initialized(&client)?;
        client.configuration_done()?;
        client.launch()?;
        wait_for_stopped(&client)?;

        Ok(Self { client })
    }

    pub(crate) fn connect_with_retry(
        address: &str,
        timeout: Duration,
    ) -> anyhow::Result<DebuggerClient> {
        use std::time::Instant;

        let deadline = Instant::now() + timeout;
        loop {
            return match DebuggerClient::connect(address) {
                Ok(client) => Ok(client),
                Err(e) => {
                    if let Some(io_err) = e.downcast_ref::<std::io::Error>()
                        && io_err.kind() == std::io::ErrorKind::ConnectionRefused
                    {
                        if Instant::now() >= deadline {
                            return Err(e);
                        }
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    Err(e)
                }
            };
        }
    }

    pub(crate) fn step_in(&mut self, thread_id: i64) -> anyhow::Result<()> {
        self.client.step_in(thread_id)
    }

    pub(crate) fn continue_execution(
        &mut self,
        thread_id: i64,
    ) -> anyhow::Result<ContinueResponse> {
        self.client.continue_execution(thread_id)
    }

    pub(crate) fn step_over(&mut self, thread_id: i64) -> anyhow::Result<()> {
        self.client.step_over(thread_id)
    }

    pub(crate) fn step_out(&mut self, thread_id: i64) -> anyhow::Result<()> {
        self.client.step_out(thread_id)
    }

    pub(crate) fn stack_trace(&mut self, thread_id: i64) -> anyhow::Result<Vec<StackFrame>> {
        let trace = self.client.stack_trace(thread_id)?;
        let positions = trace.stack_frames;
        Ok(positions)
    }

    pub(crate) fn variables(
        &mut self,
        thread_id: i64,
    ) -> anyhow::Result<Vec<dap::types::Variable>> {
        let variables = self.client.variables(thread_id)?;
        Ok(variables.variables)
    }

    #[allow(dead_code)]
    pub(crate) fn terminate(&mut self) -> anyhow::Result<()> {
        self.client.terminate()
    }
}

fn wait_for_initialized(client: &DapClient) -> anyhow::Result<()> {
    loop {
        if let Ok(Some(event)) = client.try_receive_event(Duration::from_secs(1))
            && matches!(event, Event::Initialized)
        {
            break;
        }
    }
    Ok(())
}

fn wait_for_stopped(client: &DapClient) -> anyhow::Result<()> {
    loop {
        if let Ok(Some(event)) = client.try_receive_event(Duration::from_millis(100))
            && matches!(event, Event::Stopped(_))
        {
            return Ok(());
        }
    }
}

pub(crate) fn run_script_file(
    file_path: &str,
    content: &str,
    debug_port: u16,
    stack: Tuple,
) -> anyhow::Result<String> {
    let abi = contract_abi(content.into(), file_path, &None);

    let config = ActonConfig::load();

    let mut compiler = tolkc::Compiler::new(2);
    if let Ok(config) = &config {
        compiler = compiler.with_mappings(&config.mappings)
    }

    match compiler.compile(Path::new(file_path), true) {
        CompilerResult::Success(result) => {
            let code_cell = ArcCell::from_boc_b64(&result.code_boc64)?;
            let data_cell = ArcCell::default();

            fs::write(
                "out.source_map.json",
                serde_json::to_string(&result.source_map)?,
            )?;

            let disasm = tasm::decompile::Disassembler::new();
            let code = disasm.decompile_cell(&Boc::decode_base64(&result.code_boc64)?)?;
            fs::write(
                "out.disasm.txt",
                code.print(&FormatOptions {
                    show_offsets: true,
                    show_hashes: true,
                    source_map: None,
                }),
            )?;
            fs::write("out.disasm.fif", result.fift_code)?;
            fs::write("out.boc", code_cell.to_boc(false)?)?;

            let source_map = result.source_map.unwrap_or_default();
            let (script_result, io, formatter) = execute_script(
                &code_cell,
                &data_cell,
                abi.into(),
                source_map.into(),
                debug_port,
                ExecutorVerbosity::FullLocationStackVerbose,
                stack,
            )?;
            get_script_result(script_result, io, formatter)
        }
        CompilerResult::Error(error) => {
            anyhow::bail!("Cannot compile script file {}", error.message)
        }
    }
}

fn execute_script<'a>(
    code_cell: &'a ArcCell,
    data_cell: &'a ArcCell,
    abi: Arc<ContractAbi>,
    source_map: Arc<SourceMap>,
    debug_port: u16,
    verbosity: ExecutorVerbosity,
    stack: Tuple,
) -> anyhow::Result<(GetMethodResult, IoContext, FormatterContext<'a>)> {
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

    let mut emulator = Emulator::new(verbosity, config_b64)?;
    let mut world_state =
        WorldState::new(AccountsState::Local(LocalAccountsState::new()), config_b64)?;
    let mut build_cache = BuildCache::new();
    let mut file_build_cache =
        FileBuildCache::dummy().expect("Failed to create file cache for script execution");
    let mut known_addresses = KnownAddresses::new();
    let mut known_code_cell = FxHashMap::default();
    let mut emulations = EmulationsState::new();

    let mut assert_failure = None;
    let mut expected_exit_code = None;

    let config = ActonConfig::load()?;

    let mut ctx = Context {
        env: Env {
            config: &config,
            abi: abi.clone(),
            default_log_level: verbosity,
            wallets: config.wallets.as_ref(),
            open_wallets: BTreeMap::new(),
            build_override: BTreeMap::new(),
            explorer: None,
            fork_net: None,
            api_key: None,
            running_id: "script".into(),
        },
        io: IoContext {
            stdout_buffer: String::new(),
            stderr_buffer: String::new(),
            capture_output: true,
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
        is_broadcasting: false,
        network: None,
    };

    let stack = serialize_tuple(&stack)?.to_boc_b64(false)?;

    let mut executor = StepGetExecutor::new(&stack, &params, Some(DEFAULT_CONFIG))?;
    ffi::register(&mut executor, &mut ctx);

    let transport = debugger::start_dap_server(debug_port);

    let mut dbg_ctx = DebugContext::new(
        transport,
        AnyExecutor::Get(executor.clone()),
        source_map,
        "main".into(),
    );

    ctx.debug = DebugCtx::new(&mut dbg_ctx);

    executor.prepare(0, &stack)?;

    ctx.debug.ctx().process_incoming_requests(true)?;

    let result = executor.finish(&params.code)?;
    let Context { io, .. } = ctx;

    let formatter = FormatterContext {
        contract_abi: abi,
        accounts: Cow::Owned(world_state.get_accounts().clone()),
        build_cache: Cow::Owned(build_cache.clone()),
        emulations: Cow::Owned(emulations.clone()),
        known_addresses: Cow::Owned(known_addresses.clone()),
        known_code_cells: Cow::Owned(known_code_cell.clone()),
        backtrace: None,
        fork_net: None,
        network: None,
        api_key: None,
    };

    Ok((result, io, formatter))
}

fn get_script_result(
    result: GetMethodResult,
    io: IoContext,
    formatter: FormatterContext,
) -> anyhow::Result<String> {
    match &result {
        GetMethodResult::Success(result) => {
            if result.vm_exit_code != 0 {
                anyhow::bail!("VM exit code {}", result.vm_exit_code)
            }

            let cell = ArcCell::from_boc_b64(&result.stack)?;

            let tuple = Tuple::deserialize(&cell)?;
            let tuple_str = formatter.format_tuple(&tuple, false, false);

            Ok(tuple_str + io.stdout_buffer.as_str() + io.stderr_buffer.as_str())
        }
        GetMethodResult::Error(error) => {
            anyhow::bail!("{} {}", "Execution error:".red(), error.error.red())
        }
    }
}

fn contract_address(code: &ArcCell) -> anyhow::Result<TonAddress> {
    let state_init = CellBuilder::new()
        .store_bit(false)?
        .store_bit(false)?
        .store_ref_cell_optional(Some(code))?
        .store_ref_cell_optional(Some(&ArcCell::default()))?
        .store_bit(false)?
        .build()?;

    let dest_address = TonAddress::new(0, state_init.cell_hash());
    Ok(dest_address)
}
