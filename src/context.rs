use crate::debugger::debug_context::DebugContext;
use crate::file_build_cache::FileBuildCache;
use acton_config::config;
use acton_config::config::{ActonConfig, ContractConfig, Explorer, WalletsConfig};
use acton_config::test::BacktraceMode;
use num_bigint::BigInt;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tolkc::abi::ContractABI as CompilerContractABI;
use ton::ton_wallet::TonWallet;
use ton_abi::ContractAbi;
use ton_api::{Network, TonApiClient};
use ton_emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use ton_emulator::world_state::WorldState;
use ton_executor::ExecutorVerbosity;
use ton_executor::get::GetMethodResultSuccess;
use ton_source_map::{SourceLocation, SourceMap};
use tvmffi::stack::{ContData, Tuple, TupleItem};
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{IntAddr, LibDescr, StdAddr, Transaction};

#[derive(Debug, Clone)]
pub struct AssertBinFailure {
    pub operator: String,
    pub left: Tuple,
    pub left_type: String,
    pub right: Tuple,
    pub right_type: String,
    pub message: Option<String>,
    pub location: Option<SourceLocation>,
}

impl AssertBinFailure {
    #[must_use]
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
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone)]
pub struct GetMethodAssertFailure {
    pub get_method_presentation: String,
    pub vm_exit_code: i32,
    pub suggested_name: Option<String>,
    pub vm_log: Arc<str>,
    pub source_map: Option<Arc<SourceMap>>,
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone)]
pub struct TransactionNotFoundParams {
    pub to: Option<IntAddr>,
    pub from: Option<IntAddr>,
    pub value: Option<BigInt>,
    pub exit_code: Option<u32>,
    pub success: Option<bool>,
    pub aborted: Option<bool>,
    pub deploy: Option<bool>,
    pub bounce: Option<bool>,
    pub bounced: Option<bool>,
    pub opcode: Option<u32>,
    pub action_exit_code: Option<i32>,
    pub compute_phase_skipped: Option<bool>,
    pub body: Option<Cell>,
}

/// Search params with predicate continuations (from SearchParamsInternal).
/// Each field is an optional TVM continuation that takes a value and returns bool.
#[derive(Debug, Clone, Default)]
pub struct PredicateSearchParams {
    pub to: Option<ContData>,
    pub from: Option<ContData>,
    pub value: Option<ContData>,
    pub exit_code: Option<ContData>,
    pub success: Option<ContData>,
    pub aborted: Option<ContData>,
    pub deploy: Option<ContData>,
    pub bounce: Option<ContData>,
    pub bounced: Option<ContData>,
    pub opcode: Option<ContData>,
    pub action_exit_code: Option<ContData>,
    pub compute_phase_skipped: Option<ContData>,
    pub body: Option<ContData>,
}

#[derive(Debug, Clone)]
pub struct TransactionGenericAssertFailure {
    pub message: Option<String>,
    pub location: Option<SourceLocation>,
    pub txs: TupleItem,
    pub parsed_txs: Vec<Transaction>,
    pub params: TransactionNotFoundParams,
}

#[derive(Debug, Clone)]
pub struct WalletNotFoundFailure {
    pub wallet_name: String,
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone)]
pub enum AssertFailure {
    Bin(AssertBinFailure),
    Fail(FailAssertFailure),
    GetMethod(GetMethodAssertFailure),
    TransactionNotFound(TransactionGenericAssertFailure),
    TransactionIsFound(TransactionGenericAssertFailure),
    WalletNotFound(WalletNotFoundFailure),
}

impl AssertFailure {
    #[must_use]
    pub fn message(&self) -> Option<String> {
        match self {
            AssertFailure::Bin(arg) => arg.message.clone(),
            AssertFailure::Fail(arg) => arg.message.clone(),
            AssertFailure::GetMethod(_) => None, // Formatted in FormatterContext
            AssertFailure::TransactionNotFound(arg) => arg.message.clone(),
            AssertFailure::TransactionIsFound(arg) => arg.message.clone(),
            AssertFailure::WalletNotFound(_) => None, // Formatted in FormatterContext
        }
    }

    #[must_use]
    pub fn location(&self) -> Option<SourceLocation> {
        match self {
            AssertFailure::Bin(arg) => arg.location.clone(),
            AssertFailure::Fail(arg) => arg.location.clone(),
            AssertFailure::GetMethod(arg) => arg.location.clone(),
            AssertFailure::TransactionNotFound(arg) => arg.location.clone(),
            AssertFailure::TransactionIsFound(arg) => arg.location.clone(),
            AssertFailure::WalletNotFound(arg) => arg.location.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildCache {
    pub built: FxHashMap<PathBuf, CompilationResult>,
}

impl Default for BuildCache {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            built: FxHashMap::default(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn memoize(
        &mut self,
        name: &str,
        path: &Path,
        code: &str,
        code_hash: HashBytes,
        source_map: Arc<SourceMap>,
        abi: Option<Arc<ContractAbi>>,
        compiler_abi: Option<Arc<CompilerContractABI>>,
    ) {
        self.built.insert(
            path.to_owned(),
            CompilationResult {
                name: name.to_owned(),
                code_boc64: code.to_owned(),
                code_hash,
                source_map,
                abi,
                compiler_abi,
            },
        );
    }

    #[must_use]
    pub fn result_for_code(&self, code: &Option<Cell>) -> Option<(PathBuf, CompilationResult)> {
        let Some(code) = code else { return None };
        let code_hash = code.repr_hash();
        self.built
            .iter()
            .find(|(_, result)| &result.code_hash == code_hash)
            .map(|(name, result)| ((*name).clone(), (*result).clone()))
    }
}

#[derive(Debug, Clone)]
pub struct CompilationResult {
    pub name: String,
    pub code_boc64: String,
    pub code_hash: HashBytes,
    pub source_map: Arc<SourceMap>,
    pub abi: Option<Arc<ContractAbi>>,
    pub compiler_abi: Option<Arc<CompilerContractABI>>,
}

#[derive(Debug, Clone)]
pub struct KnownAddress {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct KnownAddresses {
    pub addresses: FxHashMap<StdAddr, KnownAddress>,
}

impl Default for KnownAddresses {
    fn default() -> Self {
        Self::new()
    }
}

impl KnownAddresses {
    #[must_use]
    pub fn new() -> Self {
        Self {
            addresses: FxHashMap::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Emulations {
    pub name: String,
    pub messages: Vec<Vec<SendMessageResultSuccess>>,
    pub failed_messages: Vec<Vec<FailedSendMessageResult>>,
    pub trace_names: FxHashMap<u64, String>,
    pub get_methods: Vec<GetMethodResultSuccess>,
}

#[derive(Clone, Debug)]
pub struct FailedSendMessageResult {
    pub error: String,
    pub vm_log: Option<String>,
    pub vm_exit_code: Option<i64>,
    pub executor_logs: Option<Arc<str>>,
    pub missing_libraries: FxHashSet<String>,
}

impl Emulations {
    #[must_use]
    pub fn trace_name(&self, trace_transactions: &[SendMessageResultSuccess]) -> Option<&str> {
        let root_lt = trace_transactions.first()?.transaction.lt;
        self.trace_names.get(&root_lt).map(String::as_str)
    }
}

#[derive(Clone, Debug)]
pub struct EmulationsState {
    pub results: FxHashMap<String, Emulations>,
}

impl Default for EmulationsState {
    fn default() -> Self {
        Self::new()
    }
}

impl EmulationsState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            results: FxHashMap::default(),
        }
    }

    #[must_use]
    pub fn results_of(&self, id: &str) -> Option<&Emulations> {
        self.results.get(id)
    }

    pub fn messages(&self) -> impl Iterator<Item = &SendMessageResultSuccess> {
        self.results
            .values()
            .flat_map(|res| &res.messages)
            .flatten()
    }

    pub fn get_methods(&self) -> impl Iterator<Item = &GetMethodResultSuccess> {
        self.results.values().flat_map(|res| &res.get_methods)
    }

    pub fn save_message(&mut self, env_name: &str, message: Vec<SendMessageResult>) {
        let successful_messages = message
            .iter()
            .filter_map(|m| match m {
                SendMessageResult::Success(m) => Some(m),
                SendMessageResult::Error(_) => None,
            })
            .cloned()
            .collect::<Vec<_>>();
        let failed_messages = message
            .iter()
            .filter_map(|m| match m {
                SendMessageResult::Success(_) => None,
                SendMessageResult::Error(error) => Some(FailedSendMessageResult {
                    error: error.error.clone(),
                    vm_log: error.vm_log.clone(),
                    vm_exit_code: error.vm_exit_code,
                    executor_logs: error.executor_logs.clone(),
                    missing_libraries: error.missing_libraries.clone(),
                }),
            })
            .collect::<Vec<_>>();

        let emulations = self
            .results
            .entry(env_name.to_owned())
            .or_insert_with(|| Emulations {
                name: env_name.to_owned(),
                messages: vec![],
                failed_messages: vec![],
                trace_names: FxHashMap::default(),
                get_methods: vec![],
            });

        emulations.messages.push(successful_messages);
        emulations.failed_messages.push(failed_messages);
    }

    pub fn save_get_method(&mut self, env_name: &str, get_method: GetMethodResultSuccess) {
        self.results
            .entry(env_name.to_owned())
            .or_insert_with(|| Emulations {
                name: env_name.to_owned(),
                messages: vec![],
                failed_messages: vec![],
                trace_names: FxHashMap::default(),
                get_methods: vec![],
            })
            .get_methods
            .push(get_method);
    }

    pub fn save_trace_name(&mut self, env_name: &str, root_lt: u64, trace_name: String) {
        let Some(emulations) = self.results.get_mut(env_name) else {
            return;
        };

        emulations.trace_names.insert(root_lt, trace_name);
    }

    #[must_use]
    pub fn find_tx_by_lt(&self, lt: u64) -> Option<&SendMessageResultSuccess> {
        self.results
            .values()
            .flat_map(|result| result.messages.iter().flatten())
            .find(|res| res.transaction.lt == lt)
    }

    #[must_use]
    pub fn find_tx_logs(&self, lt: u64) -> Option<&str> {
        self.find_tx_by_lt(lt).map(|res| res.vm_log.as_ref())
    }

    #[must_use]
    pub fn find_tx_debug_logs(&self, lt: u64) -> Option<String> {
        self.find_tx_by_lt(lt).map(|res| {
            res.vm_log
                .lines()
                .filter(|line| line.starts_with("#DEBUG#:"))
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    #[must_use]
    pub fn find_tx_executor_logs(&self, lt: u64) -> Option<&str> {
        self.find_tx_by_lt(lt).map(|res| res.executor_logs.as_ref())
    }

    #[must_use]
    pub fn find_tx_missing_libraries(&self, lt: u64) -> Option<&FxHashSet<String>> {
        self.find_tx_by_lt(lt).map(|res| &res.missing_libraries)
    }
}

#[derive(Clone, Debug)]
pub struct Wallet {
    pub name: String,
    pub wallet: TonWallet,
    pub seqno: Option<u32>,
}

impl Wallet {
    pub fn seqno(&self, client: &TonApiClient) -> anyhow::Result<(u32, bool)> {
        client.get_wallet_seqno(&self.wallet.address.to_base64(true, true, true))
    }

    #[must_use]
    pub fn address(&self) -> StdAddr {
        StdAddr {
            anycast: None,
            address: HashBytes(
                <[u8; 32]>::try_from(self.wallet.address.hash.as_slice())
                    .expect("TonAddress hash must be exactly 32 bytes"),
            ),
            workchain: self.wallet.address.workchain as i8,
        }
    }
}

pub struct Env<'a> {
    pub config: &'a ActonConfig,
    pub project_root: PathBuf,
    pub abi: Arc<ContractAbi>,
    pub show_bodies: bool,
    pub default_log_level: ExecutorVerbosity,
    pub wallets: Option<&'a WalletsConfig>,
    pub open_wallets: BTreeMap<String, Wallet>,
    pub build_override: BTreeMap<String, Cell>, // contract ID -> code
    pub explorer: Option<Explorer>,
    pub fork_net: Option<Network>,
    pub api_key: Option<String>,
    pub running_id: Arc<str>,
    /// The compiled code of the currently running test contract (for c3 in run_continuation).
    pub test_code: Option<Cell>,
}

pub struct Context<'a> {
    pub env: Env<'a>,

    pub io: IoContext,
    pub asserts: AssertsContext<'a>,
    pub chain: ChainContext<'a>,
    pub build: BuildContext<'a>,
    pub debug: DebugCtx<'a>,
    pub is_broadcasting: bool,
    pub network: Option<Network>,
}

#[derive(Debug, Clone)]
pub struct IoContext {
    pub stdout_buffer: String,
    pub stderr_buffer: String,
    pub capture_output: bool,
}

pub struct AssertsContext<'a> {
    pub assert_failure: &'a mut Option<AssertFailure>,
    pub expected_exit_code: &'a mut Option<i32>,
}

pub struct ChainContext<'a> {
    pub world_state: &'a mut WorldState,
    pub emulator: &'a mut Emulator,
    pub emulations: &'a mut EmulationsState,
}

pub struct BuildContext<'a> {
    pub build_cache: &'a mut BuildCache,
    pub file_build_cache: &'a mut FileBuildCache,
    pub known_addresses: &'a mut KnownAddresses,
    pub known_code_cells: &'a mut FxHashMap<HashBytes, String>,
    pub need_debug_info: bool,
    pub backtrace: Option<BacktraceMode>,
}

pub enum DebugCtx<'a> {
    Disabled,
    Enabled { inner: &'a mut DebugContext },
}

impl Context<'_> {
    #[must_use]
    pub fn network(&self) -> Network {
        self.env
            .fork_net
            .as_ref()
            .or(self.network.as_ref())
            .unwrap_or(&Network::Testnet)
            .clone()
    }
}

impl Env<'_> {
    #[must_use]
    pub fn find_contract(&self, name: &str) -> Option<ContractConfig> {
        let contracts = self.config.contracts.clone()?.contracts;
        contracts.get(name).cloned()
    }

    #[must_use]
    pub fn find_wallet_by_address(&self, addr: &StdAddr) -> Option<Wallet> {
        let found = self
            .open_wallets
            .iter()
            .find(|(_, wallet)| &wallet.address() == addr)?;

        Some(found.1.clone())
    }

    #[must_use]
    pub fn find_wallet(&self, name: &str) -> Option<&config::WalletConfig> {
        self.wallets?.wallets.get(name)
    }
}

impl AssertsContext<'_> {
    pub fn fail(&mut self, message: String) {
        *self.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
            message: Some(message),
            location: None,
        }));
    }
}

impl ChainContext<'_> {
    #[must_use]
    pub fn build_libs(&self, owner: &IntAddr) -> Dict<HashBytes, LibDescr> {
        let std_address = owner.as_std().expect("VarAddr is unexpected");
        self.build_libs_with_hash_owner(&std_address.address)
    }

    #[must_use]
    pub fn build_libs_with_hash_owner(&self, owner: &HashBytes) -> Dict<HashBytes, LibDescr> {
        let mut libs = Dict::<HashBytes, LibDescr>::new();
        for lib in &self.world_state.libs() {
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

impl<'a> DebugCtx<'a> {
    pub const fn new(inner: &'a mut DebugContext) -> DebugCtx<'a> {
        DebugCtx::Enabled { inner }
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        matches!(self, DebugCtx::Enabled { .. })
    }

    pub fn ctx(&mut self) -> &mut DebugContext {
        match self {
            DebugCtx::Enabled { inner: ctx, .. } => ctx,
            DebugCtx::Disabled => {
                panic!("Debug context accessed from non debug context");
            }
        }
    }
}

pub(crate) fn to_cell<T: Store + ?Sized>(obj: &T) -> Cell {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())
        .expect("Failed to store data into cell builder");
    builder.build().expect("Failed to build cell from builder")
}
