use crate::debug_context::DebugContext;
use abi::ContractAbi;
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::step_by_step_trait::StepSyStepExecutor;
use emulator::step_executor::StepExecutor;
use emulator::step_get_executor::StepGetExecutor;
use emulator::tuple::stack::{Tuple, TupleItem};
use num_bigint::BigInt;
use std::collections::HashMap;
use tolkc::source_map::SourceMap;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::models::{IntAddr, Transaction};

#[derive(Debug, Clone)]
pub struct AssertBinFailure {
    pub operator: String,
    pub left: Tuple,
    pub left_type: String,
    pub right: Tuple,
    pub right_type: String,
    pub message: Option<String>,
    pub location: Option<String>,
}

impl AssertBinFailure {
    pub fn is_ord(&self) -> bool {
        self.operator == "<"
            || self.operator == ">"
            || self.operator == "<="
            || self.operator == ">="
    }
}

#[derive(Debug, Clone)]
pub struct FailAssertFailure {
    pub message: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TransactionNotFoundParams {
    pub to: IntAddr,
    pub from: Option<IntAddr>,
    pub exit_code: Option<u32>,
    pub deploy: Option<bool>,
    pub bounced: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct TransactionGenericAssertFailure {
    pub message: Option<String>,
    pub location: Option<String>,
    pub txs: TupleItem,
    pub parsed_txs: Vec<Transaction>,
    pub params: TransactionNotFoundParams,
}

#[derive(Debug, Clone)]
pub enum AssertFailure {
    Bin(AssertBinFailure),
    Fail(FailAssertFailure),
    TransactionNotFound(TransactionGenericAssertFailure),
    TransactionIsFound(TransactionGenericAssertFailure),
}

impl AssertFailure {
    pub fn message(&self) -> Option<String> {
        match self {
            AssertFailure::Bin(arg) => arg.message.clone(),
            AssertFailure::Fail(arg) => arg.message.clone(),
            AssertFailure::TransactionNotFound(arg) => arg.message.clone(),
            AssertFailure::TransactionIsFound(arg) => arg.message.clone(),
        }
    }

    pub fn location(&self) -> Option<String> {
        match self {
            AssertFailure::Bin(arg) => arg.location.clone(),
            AssertFailure::Fail(arg) => arg.location.clone(),
            AssertFailure::TransactionNotFound(arg) => arg.location.clone(),
            AssertFailure::TransactionIsFound(arg) => arg.location.clone(),
        }
    }
}

pub struct BuildCache {
    pub built: HashMap<String, CompilationResult>,
}

impl BuildCache {
    pub fn new() -> Self {
        Self {
            built: HashMap::new(),
        }
    }

    pub fn memoize(
        &mut self,
        name: &String,
        path: &String,
        code: &String,
        code_hash: &String,
        source_map: SourceMap,
    ) {
        self.built.insert(
            path.clone(),
            CompilationResult {
                name: name.clone(),
                code_boc64: code.clone(),
                code_hash: code_hash.clone(),
                source_map,
            },
        );
    }

    pub fn result_for_code(&self, code: ArcCell) -> Option<(String, CompilationResult)> {
        self.built
            .iter()
            .find(|(_, result)| result.code_boc64 == code.to_boc_b64(false).unwrap())
            .map(|(name, result)| ((*name).clone(), (*result).clone()))
    }

    pub fn to_tuple_build_cache(&self) -> emulator::tuple::stack::BuildCache {
        emulator::tuple::stack::BuildCache {
            built: self
                .built
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        emulator::tuple::stack::CompilationResult {
                            name: v.name.clone(),
                            code_boc64: v.code_boc64.clone(),
                            code_hash: v.code_hash.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

#[derive(Clone)]
pub struct CompilationResult {
    pub name: String,
    pub code_boc64: String,
    pub code_hash: String,
    pub source_map: SourceMap,
}

pub struct KnownAddress {
    pub name: String,
}

pub struct KnownAddresses {
    pub addresses: HashMap<IntAddr, KnownAddress>,
}

impl KnownAddresses {
    pub fn new() -> Self {
        Self {
            addresses: HashMap::new(),
        }
    }

    pub fn to_tuple_known_addresses(&self) -> emulator::tuple::stack::KnownAddresses {
        emulator::tuple::stack::KnownAddresses {
            addresses: self
                .addresses
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        emulator::tuple::stack::KnownAddress {
                            name: v.name.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

#[derive(Clone)]
pub enum AnyExecutor {
    Get(StepGetExecutor),
    Message(StepExecutor),
}

impl AnyExecutor {
    pub fn step(&self) -> bool {
        match self {
            AnyExecutor::Get(get) => get.step(),
            AnyExecutor::Message(msg) => msg.step(),
        }
    }

    pub fn get_code_pos(&self) -> String {
        match self {
            AnyExecutor::Get(get) => get.get_code_pos(),
            AnyExecutor::Message(msg) => msg.get_code_pos(),
        }
    }

    pub fn get_stack(&self) -> String {
        match self {
            AnyExecutor::Get(get) => get.get_stack(),
            AnyExecutor::Message(msg) => msg.get_stack(),
        }
    }

    pub fn get_c7(&self) -> String {
        match self {
            AnyExecutor::Get(get) => get.get_c7(),
            AnyExecutor::Message(msg) => msg.get_c7(),
        }
    }
}

pub struct Context<'a> {
    pub stdout_buffer: String,
    pub stderr_buffer: String,
    pub capture_test_output: bool,
    pub assert_failure: &'a mut Option<AssertFailure>,
    pub expected_exit_code: &'a mut Option<BigInt>,
    pub blockchain: &'a mut Blockchain,
    pub emulator: &'a mut Emulator,
    pub build_cache: &'a mut BuildCache,
    pub known_addresses: &'a mut KnownAddresses,
    pub abi: ContractAbi,
    pub dbg_ctx: &'a mut DebugContext,
}

impl<'a> Context<'a> {
    pub fn fail(&mut self, message: String) {
        *self.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
            message: Some(message),
            location: None,
        }));
    }
}
