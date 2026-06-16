pub(crate) mod debug;

use acton::context::{
    AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx, DebugStopRequested,
    EmulationsState, Env, ExecutionMode, IoContext, KnownAddresses, is_debug_stop_requested,
};
use acton::ffi;
use acton::file_build_cache::FileBuildCache;
use acton_config::config::{ActonConfig, LibrariesConfig, WalletsConfig, normalize_mappings};
use acton_debug::ReplayerDebugSession;
use acton_debug::replayer::TolkReplayer;
use acton_debug::{start_dap_server, start_dap_server_with_listener};
use anyhow::Context as AnyhowContext;
use dap::events::Event;
use dap::responses::ContinueResponse;
use dap::types::StackFrame;
use dap_client::DapClient;
use owo_colors::OwoColorize;
use rustc_hash::FxHashMap;
use std::collections::{BTreeMap, HashMap};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tolk_compiler::abi::ContractABI;
use tolk_compiler::{CompilerResult, SourceMap};
use ton::block_tlb::StateInit;
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_core::types::TonAddress;
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{AccountsState, LocalAccountsState, WorldState};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetMethodResult, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use tvm_ffi::serde::serialize_tuple;
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

// The shared Fift/Tolk compile path crashes under higher test concurrency,
// so serialize setup while keeping the debug session itself parallel.
static DEBUG_COMPILER_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
pub(crate) const DEBUG_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const DEBUG_EVENT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub(crate) struct DebugMethod {
    id: i32,
    name: Option<String>,
    execution_mode: ExecutionMode,
}

impl DebugMethod {
    pub(crate) fn main() -> Self {
        Self {
            id: 0,
            name: Some("main".to_string()),
            execution_mode: ExecutionMode::Script,
        }
    }

    pub(crate) fn from_id(id: i32) -> Self {
        Self {
            id,
            name: None,
            execution_mode: ExecutionMode::Script,
        }
    }

    pub(crate) fn from_name(name: &str) -> Self {
        let id = i32::from(CRC16.checksum(name.as_bytes())) | 0x1_00_00;
        Self {
            id,
            name: Some(name.to_string()),
            execution_mode: ExecutionMode::Test,
        }
    }

    fn display_name(&self, abi: Option<&ContractABI>) -> String {
        self.name
            .clone()
            .or_else(|| {
                abi.and_then(|abi| {
                    abi.find_get_method_by_id(self.id)
                        .map(|method| method.name.clone())
                })
            })
            .unwrap_or_else(|| format!("method#{}", self.id))
    }
}

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

    pub(crate) fn scopes(&mut self, frame_id: i64) -> anyhow::Result<Vec<dap::types::Scope>> {
        let scopes = self.client.scopes(frame_id)?;
        Ok(scopes.scopes)
    }

    pub(crate) fn variables(
        &mut self,
        variables_reference: i64,
    ) -> anyhow::Result<Vec<dap::types::Variable>> {
        let variables = self.client.variables(variables_reference)?;
        Ok(variables.variables)
    }

    #[allow(dead_code)]
    pub(crate) fn terminate(&mut self) -> anyhow::Result<()> {
        self.client.terminate()
    }
}

fn wait_for_initialized(client: &DapClient) -> anyhow::Result<()> {
    let deadline = Instant::now() + DEBUG_EVENT_TIMEOUT;
    loop {
        if Instant::now() >= deadline {
            anyhow::bail!(
                "Timed out waiting for DAP initialized event after {DEBUG_EVENT_TIMEOUT:?}"
            );
        }
        if let Ok(Some(event)) = client.try_receive_event(Duration::from_secs(1))
            && matches!(event, Event::Initialized)
        {
            break;
        }
    }
    Ok(())
}

fn wait_for_stopped(client: &DapClient) -> anyhow::Result<()> {
    let deadline = Instant::now() + DEBUG_EVENT_TIMEOUT;
    loop {
        if Instant::now() >= deadline {
            anyhow::bail!("Timed out waiting for DAP stopped event after {DEBUG_EVENT_TIMEOUT:?}");
        }
        if let Ok(Some(event)) = client.try_receive_event(Duration::from_millis(100))
            && matches!(event, Event::Stopped(_))
        {
            return Ok(());
        }
    }
}

pub(crate) fn run_script_file(
    file_path: &str,
    project_root: &Path,
    debug_port: u16,
    debug_listener: Option<TcpListener>,
    method: DebugMethod,
    stack: Tuple,
    capture_outer_frame_locals: bool,
) -> anyhow::Result<String> {
    let script_path = Path::new(file_path);

    let (abi, code_cell, source_map) = {
        let _compile_guard = DEBUG_COMPILER_LOCK
            .lock()
            .expect("debug compiler lock poisoned");

        let config = load_project_config(project_root);

        let mut compiler = tolk_compiler::Compiler::new(2).with_allow_no_entrypoint(true);
        if let Ok(config) = &config {
            let mappings = config.mappings();
            compiler = compiler.with_mappings(&mappings);
        }

        match compiler.compile(script_path, true) {
            CompilerResult::Success(result) => {
                let code_cell = TonCell::from_boc_base64(&result.code_boc64)?;
                let source_map = Arc::new(result.source_map.unwrap_or_default());
                let abi: Option<Arc<ContractABI>> = result.abi.map(Arc::new);

                (abi, code_cell, source_map)
            }
            CompilerResult::Error(error) => {
                anyhow::bail!("Cannot compile script file {}", error.message)
            }
        }
    };

    let data_cell = TonCell::empty().clone();
    let execution = execute_script(
        &code_cell,
        &data_cell,
        abi,
        source_map,
        debug_port,
        debug_listener,
        method,
        ExecutorVerbosity::FullLocationStackVerbose,
        stack,
        project_root,
        capture_outer_frame_locals,
    );
    let script_result = match execution {
        Ok(result) => result,
        Err(err) if is_debug_stop_requested(&err) => return Ok(String::new()),
        Err(err) => return Err(err),
    };
    get_script_result(script_result)
}

#[allow(clippy::too_many_arguments)]
fn execute_script(
    code_cell: &TonCell,
    data_cell: &TonCell,
    abi: Option<Arc<ContractABI>>,
    source_map: Arc<SourceMap>,
    debug_port: u16,
    debug_listener: Option<TcpListener>,
    method: DebugMethod,
    verbosity: ExecutorVerbosity,
    stack: Tuple,
    project_root: &Path,
    capture_outer_frame_locals: bool,
) -> anyhow::Result<GetMethodResult> {
    let dest_address = contract_address(code_cell)?;
    let method_name = method.display_name(abi.as_deref());
    let execution_mode = method.execution_mode;

    let now = std::time::SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let params = RunGetMethodArgs {
        code: code_cell.to_boc_base64()?,
        data: data_cell.to_boc_base64()?,
        verbosity,
        libs: String::new(),
        address: dest_address.to_string(),
        unixtime: duration_since_epoch.as_secs().try_into()?,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id: method.id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let config_b64: Option<&str> = None;

    let mut emulator = Emulator::new(verbosity, config_b64)?;
    let mut world_state =
        WorldState::new(AccountsState::Local(LocalAccountsState::new()), config_b64)?;
    let mut build_cache = BuildCache::new();
    let config = load_project_config(project_root)?;
    let mut file_build_cache =
        FileBuildCache::temporary_for_project(project_root.to_path_buf(), config.clone())
            .expect("Failed to create file cache for script execution");
    let mut known_addresses = KnownAddresses::new();
    let mut known_code_cell = FxHashMap::default();
    let mut emulations = EmulationsState::new();

    let mut assert_failure = None;
    let mut expected_exit_code = None;

    // `code_cell` is a `TonCell` (from `ton::ton_core::cell`) but `Env.test_code` expects a
    // `tycho_types::cell::Cell`. The two cell libraries don't interop directly, so we round-trip
    let test_code_cell = Boc::decode_base64(code_cell.to_boc_base64()?)?;

    let mut ctx = Context {
        env: Env {
            config: &config,
            project_root: project_root.to_path_buf(),
            abi: abi.clone(),
            source_map: source_map.clone(),
            show_bodies: false,
            default_log_level: verbosity,
            wallets: config.wallets.as_ref(),
            open_wallets: BTreeMap::new(),
            tonconnect: None,
            build_override: BTreeMap::new(),
            explorer: None,
            fork_net: None,
            running_id: method_name.clone().into(),
            execution_mode,
            test_code: Some(test_code_cell),
        },
        io: IoContext {
            stdout_buffer: String::new(),
            stderr_buffer: String::new(),
            capture_output: true,
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
            need_debug_info: true,
            backtrace: None,
        },
        debug: DebugCtx::Disabled,
        is_broadcasting: false,
        network: None,
    };

    let stack = Boc::encode_base64(serialize_tuple(&stack)?);

    let mut executor = StepGetExecutor::new(&stack, &params, Some(DEFAULT_CONFIG))?;
    ffi::register(&mut executor, &mut ctx);

    let transport = if let Some(listener) = debug_listener {
        start_dap_server_with_listener(listener)?
    } else {
        start_dap_server(debug_port)?
    };
    executor.prepare(method.id, &stack)?;
    let mut replayer = TolkReplayer::new_live_vm(source_map.as_ref(), executor.clone().into())?;
    replayer.set_abi(abi);
    let mut dbg_session = ReplayerDebugSession::new(transport, replayer, method_name.into())
        .with_outer_frame_local_snapshots(capture_outer_frame_locals);
    ctx.debug = DebugCtx::new(&mut dbg_session);

    if ctx.debug.process_incoming_requests(true)? {
        return Err(DebugStopRequested.into());
    }

    let result = executor.finish(&params.code)?;
    Ok(result)
}

fn load_project_config(project_root: &Path) -> anyhow::Result<ActonConfig> {
    let manifest_path = project_root.join("Acton.toml");
    if !manifest_path.exists() {
        return Ok(ActonConfig::load().unwrap_or_default());
    }

    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let mut config: ActonConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;
    config.mappings = normalize_mappings(&config.mappings, project_root);

    config.wallets = Some(WalletsConfig {
        wallets: BTreeMap::new(),
    });
    config.libraries = Some(LibrariesConfig {
        libraries: BTreeMap::new(),
    });

    Ok(config)
}

fn get_script_result(result: GetMethodResult) -> anyhow::Result<String> {
    match &result {
        GetMethodResult::Success(result) => {
            if result.vm_exit_code != 0 {
                anyhow::bail!("VM exit code {}\n{}", result.vm_exit_code, result.vm_log)
            }

            // Debug tests assert DAP state snapshots; this process output is discarded by
            // `DebugClient::finish_execution`, so formatting the final stack here only keeps
            // an otherwise unused raw tuple formatter alive.
            Ok(String::new())
        }
        GetMethodResult::Error(error) => {
            anyhow::bail!("{} {}", "Execution error:".red(), error.error.red())
        }
    }
}

fn contract_address(code: &TonCell) -> anyhow::Result<TonAddress> {
    StateInit::new(code.clone(), TonCell::empty().clone())
        .derive_address(0)
        .map_err(Into::into)
}
