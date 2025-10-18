mod config;
mod executor;
mod exit_codes;
mod exts;
mod exts_lib;
mod get_executor;
mod stack_serialization;

use crate::executor::{EmulationResult, Executor};
use crate::exts::register_extensions;
use num_bigint::BigUint;
use std::path::Path;
use tolk::{Compiler, CompilerResult};
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

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

fn main() {
    let compiler = Compiler::new();
    let compilation_result = compiler.compile(Path::new("main.tolk"));
    let code_cell = match compilation_result {
        Ok(CompilerResult::Success(success)) => {
            // println!("Compilation successful!");
            // println!("Fift code {}", success._fift_code);
            // println!("Code BOC64: {}", success.code_boc64);
            // println!("Code hash: {}", success.code_hash_hex);

            ArcCell::from_boc_b64(&*success.code_boc64).unwrap()
        }
        Ok(CompilerResult::Error(error)) => {
            println!("Compilation failed: {}", error.message);
            return;
        }
        Err(e) => {
            println!("Failed to parse compilation result: {}", e);
            return;
        }
    };

    let mut executor = Executor::new();
    register_extensions(&mut executor);

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

    let output = executor.run_transaction(msg);
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

            // println!("{}", exit_code);
            // println!("Transaction: {:?}", tx);
            // println!("Shard account: {}", result.shard_account);
            // println!("VM log: {}", result.vm_log);
            // if let Some(actions) = result.actions {
            //     println!("Actions: {}", actions);
            // }

            // TESTS.lock().unwrap().iter().for_each(|test| {
            //     println!("{}", test);
            // })
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
