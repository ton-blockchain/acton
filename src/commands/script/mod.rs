use crate::context::{AnyExecutor, BuildCache, Context, KnownAddresses};
use crate::debug_context::DebugContext;
use crate::{asserts_exts, exts, io_exts};
use abi::{ContractAbi, contract_abi};
use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender};
use dap::events::Event;
use dap::prelude::{Command, Request, Response};
use dap::types::Source;
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::get_executor::{GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use tolkc::source_map::{HighLevelSourceMap, SourceMap};
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;

pub fn script_cmd(path: &String) -> Result<(), anyhow::Error> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(anyhow!("Path '{}' is not a file", path));
    }

    if !path.ends_with(".tolk") {
        return Err(anyhow!("File must end with .tolk"));
    }

    let content = fs::read_to_string(path)?;
    run_script_file(path, &content)
}

fn run_script_file(file_path: &str, content: &str) -> Result<(), anyhow::Error> {
    let abi = contract_abi(content, file_path);

    let executable_code = content.to_string();
    let tmp_script_filename = format!("{}_script.tolk", file_path);

    fs::write(&tmp_script_filename, executable_code)?;

    let compilation_result = tolkc::compile_debug(Path::new(&tmp_script_filename));
    let result = match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let code_cell = ArcCell::from_boc_b64(&*result.code_boc64).unwrap();
            let data_cell = ArcCell::default();

            let script_result =
                execute_script(&code_cell, &data_cell, &abi, &result.source_map.unwrap());
            print_script_result(script_result?);
            Ok(())
        }
        tolkc::CompilerResult::Error(error) => {
            Err(anyhow!("Cannot compile script file {}", error.message))
        }
    };

    let _ = fs::remove_file(&tmp_script_filename);

    result
}

struct ScriptResult {
    get_result: GetMethodResult,
}

fn execute_script(
    code_cell: &ArcCell,
    data_cell: &ArcCell,
    abi: &ContractAbi,
    source_map: &SourceMap,
) -> anyhow::Result<ScriptResult> {
    let dest_address = contract_address(code_cell);

    let params = GetMethodParams {
        code: code_cell.to_boc_b64(false).unwrap().to_string(),
        data: data_cell.to_boc_b64(false).unwrap().to_string(),
        verbosity: 5,
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

    // let mut get_executor = GetExecutor::new(params.clone());
    let mut get_executor = StepGetExecutor::prepare_get_method(Default::default(), params.clone());

    let (req_receiver, response_sender, event_sender) = crate::dap::start_dap_server();

    let mut dbg_ctx = DebugContext::create_empty(
        AnyExecutor::Get(get_executor.clone()),
        source_map,
        &req_receiver,
        response_sender,
        event_sender,
    );

    let mut emulator = Emulator::new();
    let mut blockchain = Blockchain::new();
    let mut build_cache = BuildCache::new();
    let mut known_addresses = KnownAddresses::new();

    let mut ctx = Context {
        stdout_buffer: "".to_string(),
        stderr_buffer: "".to_string(),
        capture_test_output: false,
        assert_failure: &mut None,
        blockchain: &mut blockchain,
        emulator: &mut emulator,
        build_cache: &mut build_cache,
        known_addresses: &mut known_addresses,
        abi: (*abi).clone(),
        expected_exit_code: &mut None,
        dbg_ctx: &mut dbg_ctx,
    };

    exts::register_step_get_extensions(&mut get_executor, &mut ctx);
    io_exts::register_step_get_extensions(&mut get_executor, &mut ctx);
    asserts_exts::register_step_get_extensions(&mut get_executor, &mut ctx);

    get_executor.run_get_method(0, Default::default());

    for req in req_receiver.iter() {
        if let Command::Disconnect(req) = &req.command {
            println!("Disconnecting: {:?}", req);
            break;
        }
        let is_end = ctx.dbg_ctx.on_request(req)?;
        if is_end {
            ctx.dbg_ctx.event_sender.send(Event::Terminated(None))?;
            break;
        }
    }

    let result = get_executor.finish_get_method();
    // let result = get_executor.run_get_method(Default::default(), params);

    Ok(ScriptResult { get_result: result })
}

fn print_script_result(result: ScriptResult) {
    match &result.get_result {
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

fn contract_address(code: &ArcCell) -> TonAddress {
    let state_init = CellBuilder::new()
        .store_bit(false)
        .unwrap()
        .store_bit(false)
        .unwrap()
        .store_ref_cell_optional(Some(code))
        .unwrap()
        .store_ref_cell_optional(Some(&ArcCell::default()))
        .unwrap()
        .store_bit(false)
        .unwrap()
        .build()
        .unwrap();

    let dest_address = TonAddress::new(0, state_init.cell_hash());
    dest_address
}
