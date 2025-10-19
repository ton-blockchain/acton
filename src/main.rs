use abi::ABI;
use emulator::blockchain::Blockchain;
use emulator::config::DEFAULT_CONFIG;
use emulator::executor::{EmulationResult, Executor, ExecutorVerbosity, RunTransactionArgs};
use emulator_rs::context::Context;
use emulator_rs::{asserts_exts, exts, io_exts};
use num_bigint::{BigInt, BigUint};
use std::collections::HashMap;
use std::path::Path;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, CellBuilder};
use tonlib_core::tlb_types::block::coins::{CurrencyCollection, Grams};
use tonlib_core::tlb_types::block::message::{CommonMsgInfo, IntMsgInfo, Message};
use tonlib_core::tlb_types::block::msg_address::MsgAddress;
use tonlib_core::tlb_types::block::state_init::StateInit;
use tonlib_core::tlb_types::primitives::either::EitherRef;
use tonlib_core::tlb_types::primitives::reference::Ref;
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::boc::Boc;
use tycho_types::cell::Load;
use tycho_types::models::{ComputePhase, Transaction, TxInfo};

fn main() {
    let compilation_result = tolkc::compile(Path::new("main.tolk"));
    let code_cell = match compilation_result {
        tolkc::CompilerResult::Success(success) => {
            ArcCell::from_boc_b64(&*success.code_boc64).unwrap()
        }
        tolkc::CompilerResult::Error(error) => {
            println!("Compilation failed: {}", error.message);
            return;
        }
    };

    let mut blockchain = Blockchain::new(Executor::new());

    let mut test_blockchain = Blockchain::new(Executor::new());
    let mut ctx = Context {
        stdout_buffer: "".to_string(),
        stderr_buffer: "".to_string(),
        capture_test_output: true,
        assert_failure: &mut None,
        blockchain: &mut test_blockchain,
        abi: ABI {
            structs: HashMap::new(),
        },
    };

    exts::register_extensions(
        &mut blockchain.executor,
        (&mut ctx) as *mut _ as *mut std::ffi::c_void,
    );
    io_exts::register_extensions(
        &mut blockchain.executor,
        (&mut ctx) as *mut _ as *mut std::ffi::c_void,
    );
    asserts_exts::register_extensions(
        &mut blockchain.executor,
        (&mut ctx) as *mut _ as *mut std::ffi::c_void,
    );

    let state_init = CellBuilder::new()
        .store_bit(false)
        .unwrap()
        .store_bit(false)
        .unwrap()
        .store_ref_cell_optional(Some(&code_cell))
        .unwrap()
        .store_ref_cell_optional(Some(&ArcCell::default()))
        .unwrap()
        .store_bit(false)
        .unwrap()
        .build()
        .unwrap();

    let dest_address = TonAddress::new(0, state_init.cell_hash());
    let data_cell = ArcCell::from_boc_hex("b5ee9c724101010100020000004cacb9cd").unwrap();

    let msg = Message {
        info: CommonMsgInfo::Int(IntMsgInfo {
            ihr_disabled: true,
            bounce: true,
            bounced: false,
            src: MsgAddress::from_boc_hex("b5ee9c724101010100240000438015a63d6ec5cd11f837442aeba86b361f3890e715eca7c2cd44666017b8d6535d30a1578b99").unwrap(),
            dest: dest_address.to_msg_address(),
            value: CurrencyCollection {
                grams: Grams::new(BigUint::from(10000000000000000000u64)),
                other: None,
            },
            ihr_fee: Grams::new(BigUint::from(0u64)),
            fwd_fee: Grams::new(BigUint::from(0u64)),
            created_lt: 0,
            created_at: 0,
        }),
        init: Some(EitherRef::new(StateInit {
            split_depth: None,
            tick_tock: None,
            code: Some(Ref::new(code_cell.clone())),
            data: Some(Ref::new(data_cell.clone())),
            library: None,
        })),
        body: EitherRef::new(ArcCell::from(Cell::default())),
    };

    let msg_cell = Boc::decode_base64(msg.to_boc_b64(false).unwrap()).unwrap();
    let account = blockchain.get_account("".to_string());
    let params = RunTransactionArgs {
        config: DEFAULT_CONFIG.to_string(),
        libs: None,
        verbosity: ExecutorVerbosity::Short,
        shard_account: account,
        now: 0,
        lt: Default::default(),
        random_seed: None,
        ignore_chksig: false,
        debug_enabled: true,
        prev_blocks_info: None,
    };
    let output = blockchain
        .executor
        .run_transaction(msg_cell, BigInt::from(0), params);
    match output {
        EmulationResult::Success(result) => {
            #[allow(deprecated)]
            let tx_cell: tycho_types::cell::Cell =
                Boc::decode(base64::decode(&result.transaction).unwrap()).unwrap();
            let mut slice = tx_cell.as_slice().unwrap();
            let tx = Transaction::load_from(&mut slice).unwrap();

            let info: TxInfo = tx.info.parse().unwrap();
            // println!("{:?}", info);
            let exit_code = match info {
                TxInfo::Ordinary(info) => match info.compute_phase {
                    ComputePhase::Skipped(_) => 0,
                    ComputePhase::Executed(phase) => phase.exit_code,
                },
                TxInfo::TickTock(_) => 0,
            };

            println!("{}", exit_code);
            // println!("Transaction: {:?}", tx);
            // println!("Shard account: {}", result.shard_account);
            // println!("VM log: {}", result.vm_log);
            // if let Some(actions) = result.actions {
            //     println!("Actions: {}", actions);
            // }
        }
        EmulationResult::Error(result) => {
            println!("Emulation error: {}", result.error);
            if let Some(vm_log) = result.vm_log {
                println!("VM log: {}", vm_log);
            }
            if let Some(vm_exit_code) = result.vm_exit_code {
                println!("VM exit code: {}", vm_exit_code);
            }
        }
    }
}
