use crate::config::ActonConfig;
use crate::context::{AnyExecutor, BuildCache, Context, Emulations, KnownAddresses};
use crate::debug_context::DebugContext;
use crate::ffi;
use crate::file_build_cache::FileBuildCache;
use abi::{ContractAbi, contract_abi};
use anyhow::anyhow;
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::executor::ExecutorVerbosity;
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tolkc::source_map::SourceMap;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::Tuple;

pub fn script_cmd(
    path: &String,
    debug: bool,
    debug_port: u16,
    clear_cache: bool,
) -> anyhow::Result<()> {
    if clear_cache {
        let mut file_cache = FileBuildCache::new(None)?;
        file_cache.clear()?;
        println!("  {} Cache cleared", "✓".green().bold());
    }

    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(anyhow!("Path '{}' is not a file", path));
    }

    if !path.ends_with(".tolk") {
        return Err(anyhow!("File must end with .tolk"));
    }

    let content = fs::read_to_string(path)?;
    run_script_file(path, &content, debug, debug_port)
}

/// A script is essentially a regular smart contract with a `main` function,
/// which serves as an alias for the `onInternalMessage` function with ID=0.
///
/// Executing the script means calling the get-method with ID=0 and an empty stack,
/// so the `main` function takes no arguments.
fn run_script_file(
    file_path: &str,
    content: &str,
    debug: bool,
    debug_port: u16,
) -> anyhow::Result<()> {
    let abi = contract_abi(content, file_path);

    match tolkc::compile(Path::new(file_path), debug) {
        tolkc::CompilerResult::Success(result) => {
            let code_cell = ArcCell::from_boc_b64(&*result.code_boc64)?;
            let data_cell = ArcCell::default();

            let script_result = execute_script(
                &code_cell,
                &data_cell,
                &abi,
                &result.source_map.unwrap_or(Default::default()),
                debug,
                debug_port,
                ExecutorVerbosity::FullLocationStackVerbose,
            );
            print_script_result(script_result?);
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

fn execute_script(
    code_cell: &ArcCell,
    data_cell: &ArcCell,
    abi: &ContractAbi,
    source_map: &SourceMap,
    debug: bool,
    debug_port: u16,
    verbosity: ExecutorVerbosity,
) -> anyhow::Result<ScriptResult> {
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
    let mut blockchain = Blockchain::new();
    let mut build_cache = BuildCache::new();
    let mut file_build_cache =
        FileBuildCache::new(None).expect("Failed to create file cache for script execution");
    let mut known_addresses = KnownAddresses::new();
    let mut known_code_cell = HashMap::new();
    let mut emulations = Emulations::new();
    let mut libraries = vec![];

    let mut ctx = Context {
        config: &ActonConfig::load()?,
        stdout_buffer: "".to_string(),
        stderr_buffer: "".to_string(),
        capture_output: false,
        assert_failure: &mut None,
        blockchain: &mut blockchain,
        emulator: &mut emulator,
        build_cache: &mut build_cache,
        file_build_cache: &mut file_build_cache,
        known_addresses: &mut known_addresses,
        known_code_cells: &mut known_code_cell,
        emulations: &mut emulations,
        abi,
        expected_exit_code: &mut None,
        dbg_ctx: None,
        debug,
        backtrace: None,
        need_debug_info: false,
        libraries: &mut libraries,
        default_log_level: verbosity,
    };

    if debug {
        let mut executor = StepGetExecutor::new(Tuple::empty(), params.clone());
        ffi::register(&mut executor, &mut ctx);

        let transport = crate::dap::start_dap_server(debug_port);

        let mut dbg_ctx = DebugContext::new(
            transport,
            AnyExecutor::Get(executor.clone()),
            source_map,
            "main".to_string(),
        );

        ctx.with_dbg(&mut dbg_ctx);

        executor.prepare_get_method(0, Tuple::empty());

        ctx.dbg().process_incoming_requests(true)?;

        let result = executor.finish_get_method(&params.code);
        return Ok(ScriptResult { result });
    }

    let mut executor = GetExecutor::new(params.clone());
    ffi::register(&mut executor, &mut ctx);

    let result = executor.run_get_method(Tuple::empty(), params);
    Ok(ScriptResult { result })
}

fn print_script_result(result: ScriptResult) {
    match &result.result {
        GetMethodResult::Success(success_result) => {
            let exit_code = success_result.vm_exit_code;
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
        .map_err(|e| anyhow!("Failed to store bounce flag: {}", e))?
        .store_bit(false)
        .map_err(|e| anyhow!("Failed to store maybe libraries: {}", e))?
        .store_ref_cell_optional(Some(code))
        .map_err(|e| anyhow!("Failed to store code cell: {}", e))?
        .store_ref_cell_optional(Some(&ArcCell::default()))
        .map_err(|e| anyhow!("Failed to store data cell: {}", e))?
        .store_bit(false)
        .map_err(|e| anyhow!("Failed to store maybe tick/tock: {}", e))?
        .build()
        .map_err(|e| anyhow!("Failed to build state init cell: {}", e))?;

    let dest_address = TonAddress::new(0, state_init.cell_hash());
    Ok(dest_address)
}
