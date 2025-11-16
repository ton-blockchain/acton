use crate::config::ActonConfig;
use crate::debug_context::DebugContext;
use crate::file_build_cache::FileBuildCache;
use abi::ContractAbi;
use emulator::blockchain::Blockchain;
use emulator::emulator::{Emulator, SendMessageResult};
use emulator::executor::ExecutorVerbosity;
use emulator::get_executor::GetMethodResultSuccess;
use emulator::step_executor::StepExecutor;
use emulator::step_get_executor::StepGetExecutor;
use emulator::traits::BaseExecutor;
use num_bigint::BigInt;
use std::collections::HashMap;
use tolkc::source_map::SourceMap;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::cell::{Cell, HashBytes};
use tycho_types::dict::Dict;
use tycho_types::models::{IntAddr, LibDescr, Transaction};

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
    pub to: Option<IntAddr>,
    pub from: Option<IntAddr>,
    pub exit_code: Option<u32>,
    pub deploy: Option<bool>,
    pub bounced: Option<bool>,
    pub opcode: Option<u32>,
    pub action_exit_code: Option<i32>,
    pub compute_phase_skipped: Option<bool>,
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

#[derive(Debug, Clone)]
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

    pub fn result_for_code(&self, code: &Option<Cell>) -> Option<(String, CompilationResult)> {
        let Some(code) = code else { return None };
        let code_hash = code.repr_hash().to_string().to_uppercase();
        self.built
            .iter()
            .find(|(_, result)| result.code_hash == code_hash)
            .map(|(name, result)| ((*name).clone(), (*result).clone()))
    }
}

#[derive(Debug, Clone)]
pub struct CompilationResult {
    pub name: String,
    pub code_boc64: String,
    pub code_hash: String,
    pub source_map: SourceMap,
}

#[derive(Debug, Clone)]
pub struct KnownAddress {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct KnownAddresses {
    pub addresses: HashMap<IntAddr, KnownAddress>,
}

impl KnownAddresses {
    pub fn new() -> Self {
        Self {
            addresses: HashMap::new(),
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

    pub fn get_control_register(&self, idx: usize) -> String {
        match self {
            AnyExecutor::Get(get) => get.get_control_register(idx),
            AnyExecutor::Message(msg) => msg.get_control_register(idx),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Emulations {
    pub results: Vec<Vec<SendMessageResult>>,
    pub get_results: Vec<GetMethodResultSuccess>,
}

impl Emulations {
    pub fn new() -> Self {
        Self {
            results: vec![],
            get_results: vec![],
        }
    }

    pub fn find_tx_by_lt(&self, lt: u64) -> Option<&SendMessageResult> {
        self.results.iter().flatten().find(|res| match res {
            SendMessageResult::Success(res) if res.transaction.lt == lt => true,
            _ => false,
        })
    }

    pub fn find_tx_logs(&self, lt: u64) -> Option<String> {
        self.find_tx_by_lt(lt).and_then(|res| Some(res.vm_logs()))
    }

    pub fn find_tx_debug_logs(&self, lt: u64) -> Option<String> {
        self.find_tx_by_lt(lt)
            .and_then(|res| Some(res.debug_logs()))
    }

    pub fn find_tx_executor_logs(&self, lt: u64) -> Option<String> {
        self.find_tx_by_lt(lt)
            .and_then(|res| Some(res.executor_logs()))
    }
}

pub struct Context<'a> {
    pub config: &'a ActonConfig,
    pub stdout_buffer: String,
    pub stderr_buffer: String,
    pub capture_test_output: bool,
    pub assert_failure: &'a mut Option<AssertFailure>,
    pub expected_exit_code: &'a mut Option<BigInt>,
    pub blockchain: &'a mut Blockchain,
    pub emulator: &'a mut Emulator,
    pub build_cache: &'a mut BuildCache,
    pub file_build_cache: &'a mut FileBuildCache,
    pub known_addresses: &'a mut KnownAddresses,
    pub known_code_cells: &'a mut HashMap<String, String>,
    pub abi: ContractAbi,
    pub debug: bool,
    pub need_debug_info: bool,
    pub backtrace: Option<String>,
    pub emulations: &'a mut Emulations,
    pub dbg_ctx: &'a mut DebugContext,
    pub libraries: &'a mut Vec<Cell>,
    pub default_log_level: ExecutorVerbosity,
}

impl<'a> Context<'a> {
    pub fn fail(&mut self, message: String) {
        *self.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
            message: Some(message),
            location: None,
        }));
    }

    pub fn build_libs(&self, owner: &IntAddr) -> Dict<HashBytes, LibDescr> {
        self.build_libs_with_hash_owner(&owner.as_std().unwrap().address)
    }

    pub fn build_libs_with_hash_owner(&self, owner: &HashBytes) -> Dict<HashBytes, LibDescr> {
        let mut libs = Dict::<HashBytes, LibDescr>::new();
        for lib in self.libraries.clone() {
            let mut publishers = Dict::new();
            publishers.add(owner, ()).ok();

            libs.add(
                lib.repr_hash(),
                LibDescr {
                    lib: lib.clone(),
                    publishers,
                },
            )
            .ok();
        }
        libs
    }
}
