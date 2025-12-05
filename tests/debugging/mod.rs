use abi::{ContractAbi, contract_abi};
use acton::config::ActonConfig;
use acton::context::{
    AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx, Emulations, Env,
    IoContext, KnownAddresses,
};
use acton::debugger::debug_context::DebugContext;
use acton::file_build_cache::FileBuildCache;
use acton::formatter::FormatterContext;
use acton::{debugger, ffi};
use dap::events::Event;
use dap::responses::ContinueResponse;
use dap::types::StackFrame;
use dap_client::DapClient;
use emulator::AnyExecutor;
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::executor::ExecutorVerbosity;
use emulator::get_executor::{GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, fs, thread};
use tasm::printer::FormatOptions;
use tolkc::CompilerResult;
use tolkc::source_map::SourceMap;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;

mod debug_test;
mod real_test;
mod support;
mod tests;

pub struct DebuggerClient {
    client: DapClient,
}

impl DebuggerClient {
    pub fn connect(address: &str) -> anyhow::Result<Self> {
        let mut client = DapClient::connect(address)?;
        client.start()?;
        client.initialize()?;
        wait_for_initialized(&mut client)?;
        client.configuration_done()?;
        client.launch()?;
        wait_for_stopped(&mut client)?;

        Ok(Self { client })
    }

    pub fn connect_with_retry(address: &str, timeout: Duration) -> anyhow::Result<DebuggerClient> {
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

    pub fn step_in(&mut self, thread_id: i64) -> anyhow::Result<()> {
        self.client.step_in(thread_id)
    }

    pub fn continue_execution(&mut self, thread_id: i64) -> anyhow::Result<ContinueResponse> {
        self.client.continue_execution(thread_id)
    }

    pub fn step_over(&mut self, thread_id: i64) -> anyhow::Result<()> {
        self.client.step_over(thread_id)
    }

    pub fn step_out(&mut self, thread_id: i64) -> anyhow::Result<()> {
        self.client.step_out(thread_id)
    }

    pub fn stack_trace(&mut self, thread_id: i64) -> anyhow::Result<Vec<StackFrame>> {
        let trace = self.client.stack_trace(thread_id)?;
        let positions = trace.stack_frames;
        Ok(positions)
    }

    pub fn variables(&mut self, thread_id: i64) -> anyhow::Result<Vec<dap::types::Variable>> {
        let variables = self.client.variables(thread_id)?;
        Ok(variables.variables)
    }

    pub fn terminate(&mut self) -> anyhow::Result<()> {
        self.client.terminate()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourcePosition {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl SourcePosition {
    pub fn new(file: String, line: u32, column: u32) -> Self {
        Self {
            file: normalize_path(&file),
            line,
            column,
        }
    }
}

impl std::fmt::Display for SourcePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

fn normalize_path(path: &str) -> String {
    let path_buf = PathBuf::from(path);
    if let Ok(current_dir) = env::current_dir()
        && let Ok(relative) = path_buf.strip_prefix(&current_dir)
    {
        return relative.to_string_lossy().to_string();
    }
    path.to_string()
}

fn wait_for_initialized(client: &mut DapClient) -> anyhow::Result<()> {
    loop {
        if let Ok(Some(event)) = client.try_receive_event(Duration::from_secs(1))
            && matches!(event, Event::Initialized)
        {
            break;
        }
    }
    Ok(())
}

fn wait_for_stopped(client: &mut DapClient) -> anyhow::Result<()> {
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
    let abi = contract_abi(content, file_path);

    match tolkc::compile(Path::new(file_path), true) {
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

            let (script_result, ctx, formatter) = execute_script(
                &code_cell,
                &data_cell,
                &abi,
                &result.source_map.unwrap_or(Default::default()),
                debug_port,
                ExecutorVerbosity::FullLocationStackVerbose,
                stack,
            )?;
            get_script_result(script_result, ctx, formatter)
        }
        CompilerResult::Error(error) => {
            anyhow::bail!("Cannot compile script file {}", error.message)
        }
    }
}

fn execute_script(
    code_cell: &ArcCell,
    data_cell: &ArcCell,
    abi: &ContractAbi,
    source_map: &SourceMap,
    debug_port: u16,
    verbosity: ExecutorVerbosity,
    stack: Tuple,
) -> anyhow::Result<(GetMethodResult, IoContext, FormatterContext)> {
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
    let mut blockchain = Blockchain::new(None, None);
    let mut build_cache = BuildCache::new();
    let mut file_build_cache =
        FileBuildCache::dummy().expect("Failed to create file cache for script execution");
    let mut known_addresses = KnownAddresses::new();
    let mut known_code_cell = HashMap::new();
    let mut emulations = Emulations::new();

    let mut assert_failure = None;
    let mut expected_exit_code = None;

    let config = ActonConfig::load()?;

    let mut ctx = Context {
        env: Env {
            config: &config,
            abi,
            default_log_level: verbosity,
            wallets: config.wallets.as_ref(),
            open_wallets: BTreeMap::new(),
        },
        io: IoContext {
            stdout_buffer: "".to_string(),
            stderr_buffer: "".to_string(),
            capture_output: true,
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
        is_broadcasting: false,
        network: "testnet".to_string(),
    };

    let mut executor = StepGetExecutor::new(stack.clone(), params.clone());
    ffi::register(&mut executor, &mut ctx);

    let transport = debugger::start_dap_server(debug_port);

    let mut dbg_ctx = DebugContext::new(
        transport,
        AnyExecutor::Get(executor.clone()),
        source_map,
        "main".to_string(),
    );

    ctx.debug = DebugCtx::new(&mut dbg_ctx);

    executor.prepare(0, stack);

    ctx.debug.ctx().process_incoming_requests(true)?;

    let result = executor.finish(&params.code);
    let formatter = FormatterContext::from_context(&ctx);
    let io = ctx.io;
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
            let tuple_str = formatter.format_tuple(&tuple);

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
