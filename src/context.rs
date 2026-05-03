use crate::file_build_cache::FileBuildCache;
use crate::retrace::TolkTraceInfo;
use crate::tonconnect::TonConnectContext;
use acton_config::config;
use acton_config::config::{ActonConfig, ContractConfig, Explorer, WalletsConfig};
use acton_config::test::BacktraceMode;
use acton_debug::replayer::StepMode;
use acton_debug::{ChildDebugContextSpec, ReplayerDebugSession};
use num_bigint::BigInt;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tolk_compiler::SourceMap;
use tolk_compiler::abi::{ContractABI, Ty};
use ton::ton_wallet::TonWallet;
use ton_api::{Network, TonApiClient};
use ton_emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use ton_emulator::world_state::WorldState;
use ton_executor::ExecutorVerbosity;
use ton_executor::get::GetMethodResultSuccess;
use ton_source_map::SourceLocation;
use tvm_ffi::stack::{ContData, Tuple, TupleItem};
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{IntAddr, LibDescr, StdAddr, Transaction};

#[derive(Debug)]
pub struct DebugStopRequested;

impl std::fmt::Display for DebugStopRequested {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Debug session stopped")
    }
}

impl std::error::Error for DebugStopRequested {}

#[must_use]
pub fn is_debug_stop_requested(err: &anyhow::Error) -> bool {
    err.downcast_ref::<DebugStopRequested>().is_some()
}

#[derive(Debug, Clone)]
pub struct AssertBinFailure {
    pub operator: String,
    pub left: Tuple,
    pub left_ty: Ty,
    pub right: Tuple,
    pub right_ty: Ty,
    pub source_map: Arc<SourceMap>,
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
pub struct AssertDecimalFailure {
    pub left: String,
    pub right: String,
    pub message: Option<String>,
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone)]
pub struct GetMethodAssertFailure {
    pub get_method_presentation: String,
    pub vm_exit_code: i32,
    pub suggested_name: Option<String>,
    pub vm_log: Arc<str>,
    pub source_map: Arc<SourceMap>,
    pub abi: Option<Arc<ContractABI>>,
    pub caller_trace: Option<TolkTraceInfo>,
    pub location: Option<SourceLocation>,
}

/// A display-only search param: either a concrete value or a `<function>` marker.
#[derive(Debug, Clone)]
pub enum DisplayParam<T> {
    Value(T),
    Function,
}

#[derive(Debug, Clone)]
pub struct TransactionNotFoundParams {
    pub to: Option<DisplayParam<IntAddr>>,
    pub from: Option<DisplayParam<IntAddr>>,
    pub value: Option<DisplayParam<BigInt>>,
    pub exit_code: Option<DisplayParam<u32>>,
    pub success: Option<DisplayParam<bool>>,
    pub aborted: Option<DisplayParam<bool>>,
    pub deploy: Option<DisplayParam<bool>>,
    pub bounce: Option<DisplayParam<bool>>,
    pub bounced: Option<DisplayParam<bool>>,
    pub opcode: Option<DisplayParam<u32>>,
    pub action_exit_code: Option<DisplayParam<i32>>,
    pub compute_phase_skipped: Option<DisplayParam<bool>>,
    pub body: Option<DisplayParam<Cell>>,
    pub state_init: Option<DisplayParam<Cell>>,
}

/// A search field parsed from `SearchParams`.
/// Tag 0 = absent, tag 1 = user-provided predicate, tag 2 = plain value converted to predicate.
#[derive(Debug, Clone)]
pub struct SearchField {
    /// 1 = user predicate (display as `<predicate>`), 2 = value-based (display as `<value>`)
    pub tag: u8,
    pub predicate: ContData,
}

/// Parsed search params from `SearchParams` union fields.
/// Each field is either a predicate (with tag for display) or absent (None).
#[derive(Debug, Clone, Default)]
pub struct ParsedSearchParams {
    pub to: Option<SearchField>,
    pub from: Option<SearchField>,
    pub value: Option<SearchField>,
    pub exit_code: Option<SearchField>,
    pub success: Option<SearchField>,
    pub aborted: Option<SearchField>,
    pub deploy: Option<SearchField>,
    pub bounce: Option<SearchField>,
    pub bounced: Option<SearchField>,
    pub opcode: Option<SearchField>,
    pub action_exit_code: Option<SearchField>,
    pub compute_phase_skipped: Option<SearchField>,
    pub body: Option<SearchField>,
    pub state_init: Option<SearchField>,
}

#[derive(Debug, Clone)]
pub struct TransactionGenericAssertFailure {
    pub message: Option<String>,
    pub location: Option<SourceLocation>,
    pub txs: Vec<TupleItem>,
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
    Decimal(AssertDecimalFailure),
    Fail(FailAssertFailure),
    Assume(FailAssertFailure),
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
            AssertFailure::Decimal(arg) => arg.message.clone(),
            AssertFailure::Fail(arg) | AssertFailure::Assume(arg) => arg.message.clone(),
            AssertFailure::GetMethod(_) | AssertFailure::WalletNotFound(_) => None, // Formatted in FormatterContext
            AssertFailure::TransactionNotFound(arg) | AssertFailure::TransactionIsFound(arg) => {
                arg.message.clone()
            }
        }
    }

    #[must_use]
    pub fn location(&self) -> Option<SourceLocation> {
        match self {
            AssertFailure::Bin(arg) => arg.location.clone(),
            AssertFailure::Decimal(arg) => arg.location.clone(),
            AssertFailure::Fail(arg) | AssertFailure::Assume(arg) => arg.location.clone(),
            AssertFailure::GetMethod(arg) => arg.location.clone(),
            AssertFailure::TransactionNotFound(arg) | AssertFailure::TransactionIsFound(arg) => {
                arg.location.clone()
            }
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

    pub fn memoize(
        &mut self,
        name: &str,
        path: &Path,
        code: &str,
        code_hash: HashBytes,
        source_map: Arc<SourceMap>,
        abi: Option<Arc<ContractABI>>,
    ) {
        self.built.insert(
            path.to_owned(),
            CompilationResult {
                name: name.to_owned(),
                code_boc64: code.to_owned(),
                code_hash,
                source_map,
                abi,
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
    pub abi: Option<Arc<ContractABI>>,
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
    pub trace_position_by_tx_lt: FxHashMap<u64, TracePosition>,
    pub trace_names: FxHashMap<u64, String>,
    pub get_methods: Vec<GetMethodResultSuccess>,
}

#[derive(Clone, Copy, Debug)]
pub struct TracePosition {
    pub trace_index: usize,
    pub tx_index: usize,
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

    pub fn save_message(&mut self, env_name: &str, message: Vec<SendMessageResult>) -> usize {
        let (successful_messages, failed_messages) = split_send_message_results(&message);
        let emulations = self.emulations_mut(env_name);
        emulations.messages.push(successful_messages);
        emulations.failed_messages.push(failed_messages);
        let trace_index = emulations.messages.len() - 1;
        let tx_lts = emulations.messages[trace_index]
            .iter()
            .map(|result| result.transaction.lt)
            .collect::<Vec<_>>();
        index_trace_transactions(emulations, trace_index, &tx_lts);
        trace_index
    }

    pub fn append_message_to_trace(
        &mut self,
        env_name: &str,
        trace_index: usize,
        message: Vec<SendMessageResult>,
    ) -> usize {
        let (successful_messages, failed_messages) = split_send_message_results(&message);
        let emulations = self.emulations_mut(env_name);

        if trace_index >= emulations.messages.len()
            || trace_index >= emulations.failed_messages.len()
        {
            emulations.messages.push(successful_messages);
            emulations.failed_messages.push(failed_messages);
            let trace_index = emulations.messages.len() - 1;
            let tx_lts = emulations.messages[trace_index]
                .iter()
                .map(|result| result.transaction.lt)
                .collect::<Vec<_>>();
            index_trace_transactions(emulations, trace_index, &tx_lts);
            return trace_index;
        }

        emulations.messages[trace_index].extend(successful_messages);
        emulations.failed_messages[trace_index].extend(failed_messages);
        recompute_trace_child_transactions(&mut emulations.messages[trace_index]);
        let tx_lts = emulations.messages[trace_index]
            .iter()
            .map(|result| result.transaction.lt)
            .collect::<Vec<_>>();
        index_trace_transactions(emulations, trace_index, &tx_lts);
        trace_index
    }

    pub fn save_get_method(&mut self, env_name: &str, get_method: GetMethodResultSuccess) {
        self.results
            .entry(env_name.to_owned())
            .or_insert_with(|| Emulations {
                name: env_name.to_owned(),
                messages: vec![],
                failed_messages: vec![],
                trace_position_by_tx_lt: FxHashMap::default(),
                trace_names: FxHashMap::default(),
                get_methods: vec![],
            })
            .get_methods
            .push(get_method);
    }

    pub fn save_trace_name(&mut self, env_name: &str, lt: u64, trace_name: String) {
        let Some(emulations) = self.results.get_mut(env_name) else {
            return;
        };

        let trace_root_lt = emulations
            .trace_position_by_tx_lt
            .get(&lt)
            .and_then(|position| emulations.messages.get(position.trace_index))
            .and_then(|trace| trace.first().map(|tx| tx.transaction.lt))
            .unwrap_or(lt);

        emulations.trace_names.insert(trace_root_lt, trace_name);
    }

    #[must_use]
    pub fn find_trace_segment_by_tx_lt_range(
        &self,
        env_name: &str,
        first_tx_lt: u64,
        last_tx_lt: u64,
    ) -> Option<&[SendMessageResultSuccess]> {
        let emulations = self.results.get(env_name)?;
        let first = emulations.trace_position_by_tx_lt.get(&first_tx_lt)?;
        let last = emulations.trace_position_by_tx_lt.get(&last_tx_lt)?;
        if first.trace_index != last.trace_index || first.tx_index > last.tx_index {
            return None;
        }

        let trace = emulations.messages.get(first.trace_index)?;
        trace.get(first.tx_index..=last.tx_index)
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

    fn emulations_mut(&mut self, env_name: &str) -> &mut Emulations {
        self.results
            .entry(env_name.to_owned())
            .or_insert_with(|| Emulations {
                name: env_name.to_owned(),
                messages: vec![],
                failed_messages: vec![],
                trace_position_by_tx_lt: FxHashMap::default(),
                trace_names: FxHashMap::default(),
                get_methods: vec![],
            })
    }
}

fn split_send_message_results(
    message: &[SendMessageResult],
) -> (Vec<SendMessageResultSuccess>, Vec<FailedSendMessageResult>) {
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

    (successful_messages, failed_messages)
}

fn recompute_trace_child_transactions(trace: &mut [SendMessageResultSuccess]) {
    let mut children_by_parent = FxHashMap::<u64, Vec<u64>>::default();

    for result in trace.iter() {
        if let Some(parent_lt) = result.parent_transaction {
            children_by_parent
                .entry(parent_lt)
                .or_default()
                .push(result.transaction.lt);
        }
    }

    for result in trace.iter_mut() {
        result.child_transactions = children_by_parent
            .remove(&result.transaction.lt)
            .unwrap_or_default();
    }
}

fn index_trace_transactions(emulations: &mut Emulations, trace_index: usize, tx_lts: &[u64]) {
    for (tx_index, tx_lt) in tx_lts.iter().copied().enumerate() {
        emulations.trace_position_by_tx_lt.insert(
            tx_lt,
            TracePosition {
                trace_index,
                tx_index,
            },
        );
    }
}

#[derive(Clone)]
pub struct PendingMessageStep {
    pub message: Cell,
    pub from: Option<IntAddr>,
    pub parent_lt: Option<u64>,
}

pub struct MessageCursor {
    pending: VecDeque<PendingMessageStep>,
    libs_owner: HashBytes,
    trace_index: Option<usize>,
}

pub struct MessageIterState {
    next_id: u64,
    cursors: FxHashMap<u64, MessageCursor>,
}

impl Default for MessageIterState {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageIterState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 1,
            cursors: FxHashMap::default(),
        }
    }

    #[must_use]
    pub fn insert_message_cursor(
        &mut self,
        message: Cell,
        from: Option<IntAddr>,
        libs_owner: HashBytes,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        self.cursors.insert(
            id,
            MessageCursor {
                pending: VecDeque::from([PendingMessageStep {
                    message,
                    from,
                    parent_lt: None,
                }]),
                libs_owner,
                trace_index: None,
            },
        );

        id
    }

    #[must_use]
    pub fn contains(&self, id: u64) -> bool {
        self.cursors.contains_key(&id)
    }

    #[must_use]
    pub fn is_done(&self, id: u64) -> bool {
        self.cursors
            .get(&id)
            .is_none_or(|cursor| cursor.pending.is_empty())
    }

    #[must_use]
    pub fn peek_next(&self, id: u64) -> Option<(PendingMessageStep, HashBytes)> {
        let cursor = self.cursors.get(&id)?;
        let pending = cursor.pending.front()?.clone();
        Some((pending, cursor.libs_owner))
    }

    pub fn advance(&mut self, id: u64) -> Option<()> {
        let cursor = self.cursors.get_mut(&id)?;
        cursor.pending.pop_front()?;
        Some(())
    }

    pub fn push_child_message(&mut self, id: u64, message: Cell, parent_lt: u64) -> Option<()> {
        let cursor = self.cursors.get_mut(&id)?;
        cursor.pending.push_back(PendingMessageStep {
            message,
            from: None,
            parent_lt: Some(parent_lt),
        });
        Some(())
    }

    #[must_use]
    pub fn trace_index(&self, id: u64) -> Option<Option<usize>> {
        self.cursors.get(&id).map(|cursor| cursor.trace_index)
    }

    pub fn set_trace_index(&mut self, id: u64, trace_index: usize) -> Option<()> {
        let cursor = self.cursors.get_mut(&id)?;
        cursor.trace_index = Some(trace_index);
        Some(())
    }

    #[must_use]
    pub fn close(&mut self, id: u64) -> bool {
        self.cursors.remove(&id).is_some()
    }

    #[must_use]
    pub fn close_if_done(&mut self, id: u64) -> bool {
        if self.is_done(id) {
            return self.close(id);
        }

        false
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
    pub abi: Option<Arc<ContractABI>>,
    pub source_map: Arc<SourceMap>,
    pub show_bodies: bool,
    pub default_log_level: ExecutorVerbosity,
    pub wallets: Option<&'a WalletsConfig>,
    pub open_wallets: BTreeMap<String, Wallet>,
    pub tonconnect: Option<TonConnectContext>,
    pub build_override: BTreeMap<String, Cell>, // contract name -> code
    pub explorer: Option<Explorer>,
    pub fork_net: Option<Network>,
    pub running_id: Arc<str>,
    pub execution_mode: ExecutionMode,
    /// The compiled code of the currently running test contract (for c3 in `run_continuation`).
    pub test_code: Option<Cell>,
}

pub struct Context<'a> {
    pub env: Env<'a>,

    pub io: IoContext,
    pub asserts: AssertsContext<'a>,
    pub chain: ChainContext<'a>,
    pub message_iters: MessageIterState,
    pub build: BuildContext<'a>,
    pub debug: DebugCtx<'a>,
    pub is_broadcasting: bool,
    pub network: Option<Network>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ExecutionMode {
    Test,
    Script,
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
    Enabled { inner: &'a mut ReplayerDebugSession },
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

    #[must_use]
    pub fn can_broadcast_to_network(&self) -> bool {
        self.env.execution_mode == ExecutionMode::Script && self.is_broadcasting
    }

    #[must_use]
    pub fn resolve_project_read_path(&self, path: &str) -> Option<PathBuf> {
        let path = self.resolve_project_relative_path(path)?;
        let project_root = dunce::canonicalize(&self.env.project_root).ok()?;
        let canonical_path = dunce::canonicalize(path).ok()?;
        canonical_path
            .starts_with(project_root)
            .then_some(canonical_path)
    }

    #[must_use]
    pub fn resolve_project_write_path(&self, path: &str) -> Option<PathBuf> {
        let path = self.resolve_project_relative_path(path)?;
        let project_root = dunce::canonicalize(&self.env.project_root).ok()?;

        if let Ok(canonical_path) = dunce::canonicalize(&path) {
            return canonical_path.starts_with(&project_root).then_some(path);
        }

        let parent = path.parent()?;
        let canonical_parent = dunce::canonicalize(parent).ok()?;
        canonical_parent.starts_with(&project_root).then_some(path)
    }

    fn resolve_project_relative_path(&self, path: &str) -> Option<PathBuf> {
        let mut relative_path = PathBuf::new();
        for component in Path::new(path).components() {
            match component {
                Component::CurDir => {}
                Component::Normal(part) => relative_path.push(part),
                Component::ParentDir => {
                    if !relative_path.pop() {
                        return None;
                    }
                }
                Component::Prefix(_) | Component::RootDir => return None,
            }
        }

        Some(self.env.project_root.join(relative_path))
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
    pub fn find_tonconnect_by_address(&self, addr: &StdAddr) -> Option<&TonConnectContext> {
        self.tonconnect
            .as_ref()
            .filter(|tonconnect| tonconnect.wallet.address == *addr)
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
    pub const fn new(inner: &'a mut ReplayerDebugSession) -> DebugCtx<'a> {
        DebugCtx::Enabled { inner }
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        matches!(self, DebugCtx::Enabled { .. })
    }

    fn session(&mut self) -> &mut ReplayerDebugSession {
        match self {
            DebugCtx::Enabled { inner: ctx, .. } => ctx,
            DebugCtx::Disabled => {
                panic!("Debug context accessed from non debug context");
            }
        }
    }

    pub fn process_incoming_requests(&mut self, terminate_at_end: bool) -> anyhow::Result<bool> {
        self.session().process_incoming_requests(terminate_at_end)
    }

    #[must_use]
    pub fn need_to_stop_child_thread_on_start(&mut self) -> bool {
        self.session().need_to_stop_child_thread_on_start()
    }

    pub fn begin_child_context(&mut self, spec: ChildDebugContextSpec) -> anyhow::Result<bool> {
        self.session().begin_child_context(spec)
    }

    pub fn finish_child_context(&mut self, thread_id: i64) -> anyhow::Result<()> {
        self.session().finish_child_context(thread_id)
    }

    pub fn step(&mut self, mode: StepMode) -> bool {
        self.session().step(mode)
    }

    #[must_use]
    pub fn active_context_is_terminated(&mut self) -> bool {
        self.session().active_context_is_terminated()
    }

    #[must_use]
    pub fn performing_step(&mut self) -> Option<StepMode> {
        self.session().performing_step()
    }
}

pub(crate) fn to_cell<T: Store + ?Sized>(obj: &T) -> Cell {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())
        .expect("Failed to store data into cell builder");
    builder.build().expect("Failed to build cell from builder")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_hash(byte: u8) -> HashBytes {
        HashBytes([byte; 32])
    }

    #[test]
    fn message_iter_peek_does_not_consume_until_advanced() {
        let mut state = MessageIterState::new();
        let root = Cell::default();
        let cursor_id = state.insert_message_cursor(root.clone(), None, dummy_hash(1));

        let (first_peek, first_owner) = state.peek_next(cursor_id).expect("cursor must be present");
        assert_eq!(first_owner, dummy_hash(1));
        assert_eq!(first_peek.parent_lt, None);
        assert_eq!(first_peek.message, root);
        assert!(!state.is_done(cursor_id));

        let (second_peek, second_owner) = state.peek_next(cursor_id).expect("peek must be stable");
        assert_eq!(second_owner, dummy_hash(1));
        assert_eq!(second_peek.parent_lt, None);
        assert_eq!(second_peek.message, root);

        state
            .advance(cursor_id)
            .expect("advance must consume the step");
        assert!(state.is_done(cursor_id));
    }

    #[test]
    fn message_iter_close_if_done_removes_exhausted_cursor() {
        let mut state = MessageIterState::new();
        let cursor_id = state.insert_message_cursor(Cell::default(), None, dummy_hash(2));

        assert!(state.contains(cursor_id));
        assert!(!state.close_if_done(cursor_id));
        assert!(state.contains(cursor_id));

        state
            .advance(cursor_id)
            .expect("advance must drain the only step");
        assert!(state.close_if_done(cursor_id));
        assert!(!state.contains(cursor_id));
        assert!(state.is_done(cursor_id));
    }

    #[test]
    fn message_iter_accepts_children_after_root_step_is_consumed() {
        let mut state = MessageIterState::new();
        let child = Cell::default();
        let cursor_id = state.insert_message_cursor(Cell::default(), None, dummy_hash(3));

        state.advance(cursor_id).expect("advance must consume root");
        assert!(state.is_done(cursor_id));

        state
            .push_child_message(cursor_id, child.clone(), 777)
            .expect("cursor should accept child messages while still open");

        let (pending, owner) = state
            .peek_next(cursor_id)
            .expect("child must become pending");
        assert_eq!(owner, dummy_hash(3));
        assert_eq!(pending.parent_lt, Some(777));
        assert_eq!(pending.message, child);
        assert!(!state.is_done(cursor_id));
    }
}
