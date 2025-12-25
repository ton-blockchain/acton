use crate::config::{ActonConfig, ContractConfig, Explorer, WalletsConfig};
use crate::debugger::debug_context::DebugContext;
use crate::file_build_cache::FileBuildCache;
use abi::ContractAbi;
use emulator::blockchain::Blockchain;
use emulator::emulator::{Emulator, SendMessageResult};
use emulator::executor::ExecutorVerbosity;
use emulator::get_executor::GetMethodResultSuccess;
use num_bigint::BigInt;
use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashMap};
use tolkc::source_map::SourceMap;
use ton_api::{Network, TonApiClient};
use tonlib_core::cell::ArcCell;
use tonlib_core::wallet::ton_wallet::TonWallet;
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
    pub bounce: Option<bool>,
    pub bounced: Option<bool>,
    pub opcode: Option<u32>,
    pub action_exit_code: Option<i32>,
    pub compute_phase_skipped: Option<bool>,
    pub body: Option<Cell>,
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
pub struct WalletNotFoundFailure {
    pub wallet_name: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AssertFailure {
    Bin(AssertBinFailure),
    Fail(FailAssertFailure),
    TransactionNotFound(TransactionGenericAssertFailure),
    TransactionIsFound(TransactionGenericAssertFailure),
    WalletNotFound(WalletNotFoundFailure),
}

impl AssertFailure {
    pub fn message(&self) -> Option<String> {
        match self {
            AssertFailure::Bin(arg) => arg.message.clone(),
            AssertFailure::Fail(arg) => arg.message.clone(),
            AssertFailure::TransactionNotFound(arg) => arg.message.clone(),
            AssertFailure::TransactionIsFound(arg) => arg.message.clone(),
            AssertFailure::WalletNotFound(_) => None, // Will be formatted in print_script_result
        }
    }

    pub fn location(&self) -> Option<String> {
        match self {
            AssertFailure::Bin(arg) => arg.location.clone(),
            AssertFailure::Fail(arg) => arg.location.clone(),
            AssertFailure::TransactionNotFound(arg) => arg.location.clone(),
            AssertFailure::TransactionIsFound(arg) => arg.location.clone(),
            AssertFailure::WalletNotFound(arg) => arg.location.clone(),
        }
    }

    pub fn format_wallet_not_found_message(failure: &WalletNotFoundFailure, env: &Env) -> String {
        let has_wallets_config = env.wallets.is_some();
        let available_wallets = env.open_wallets.keys().cloned().collect::<Vec<_>>();

        if !has_wallets_config || available_wallets.is_empty() {
            color_print::cformat!(
                "Wallet {} not found in Acton.toml. Wallets are not configured yet.

To add wallets, run {} or add the following section to your Acton.toml:

<dim># Example wallet configuration</>
[wallets.{}]
type = \"v4r2\"
workchain = 0
keys = {{ mnemonic-env = \"WALLET_MNEMONIC\" }}

[wallets.deployer.expected]
address-testnet = \"<<ADDRESS>>\"

See https://i582.github.io/acton/docs/scripting/setup-wallets/ for more information
",
                failure.wallet_name.yellow(),
                "acton wallet new".green(),
                failure.wallet_name
            )
        } else {
            let available = if available_wallets.is_empty() {
                "no wallets defined yet".to_string()
            } else {
                available_wallets
                    .iter()
                    .map(|s| format!("  {}", s.yellow()))
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            format!(
                "Wallet {} not found in Acton.toml\nAvailable wallets:\n{}",
                failure.wallet_name.yellow(),
                available
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildCache {
    pub built: HashMap<String, CompilationResult>,
}

impl Default for BuildCache {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildCache {
    pub fn new() -> Self {
        Self {
            built: HashMap::new(),
        }
    }

    pub fn memoize(
        &mut self,
        name: &str,
        path: &str,
        code: &str,
        code_hash: &str,
        source_map: SourceMap,
    ) {
        self.built.insert(
            path.to_owned(),
            CompilationResult {
                name: name.to_owned(),
                code_boc64: code.to_owned(),
                code_hash: code_hash.to_owned(),
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

impl Default for KnownAddresses {
    fn default() -> Self {
        Self::new()
    }
}

impl KnownAddresses {
    pub fn new() -> Self {
        Self {
            addresses: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Emulations {
    pub results: Vec<Vec<SendMessageResult>>,
    pub get_results: Vec<GetMethodResultSuccess>,
}

impl Default for Emulations {
    fn default() -> Self {
        Self::new()
    }
}

impl Emulations {
    pub fn new() -> Self {
        Self {
            results: vec![],
            get_results: vec![],
        }
    }

    pub fn find_tx_by_lt(&self, lt: u64) -> Option<&SendMessageResult> {
        self.results
            .iter()
            .flatten()
            .find(|res| matches!(res, SendMessageResult::Success(res) if res.transaction.lt == lt))
    }

    pub fn find_tx_logs(&self, lt: u64) -> Option<&str> {
        self.find_tx_by_lt(lt).map(|res| res.vm_logs())
    }

    pub fn find_tx_debug_logs(&self, lt: u64) -> Option<String> {
        self.find_tx_by_lt(lt).map(|res| res.debug_logs())
    }

    pub fn find_tx_executor_logs(&self, lt: u64) -> Option<&str> {
        self.find_tx_by_lt(lt).map(|res| res.executor_logs())
    }
}

#[derive(Clone, Debug)]
pub struct Wallet {
    pub name: String,
    pub wallet: TonWallet,
    pub seqno: Option<u32>,
}

impl Wallet {
    pub fn seqno(&self, net: &str) -> anyhow::Result<(u32, bool)> {
        let network = Network::from_str(net)?;
        let client = TonApiClient::new(network, None);
        client.get_wallet_seqno(&self.wallet.address.to_base64_std())
    }
}

pub struct Env<'a> {
    pub config: &'a ActonConfig,
    pub abi: &'a ContractAbi,
    pub default_log_level: ExecutorVerbosity,
    pub wallets: Option<&'a WalletsConfig>,
    pub open_wallets: BTreeMap<String, Wallet>,
    pub build_override: BTreeMap<String, ArcCell>, // contract ID -> code
    pub explorer: Option<Explorer>,
}

pub struct Context<'a> {
    pub env: Env<'a>,

    pub io: IoContext,
    pub asserts: AssertsContext<'a>,
    pub chain: ChainContext<'a>,
    pub build: BuildContext<'a>,
    pub debug: DebugCtx<'a>,
    pub is_broadcasting: bool,
    pub network: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IoContext {
    pub stdout_buffer: String,
    pub stderr_buffer: String,
    pub capture_output: bool,
}

pub struct AssertsContext<'a> {
    pub assert_failure: &'a mut Option<AssertFailure>,
    pub expected_exit_code: &'a mut Option<BigInt>,
}

pub struct ChainContext<'a> {
    pub blockchain: &'a mut Blockchain,
    pub emulator: &'a mut Emulator,
    pub emulations: &'a mut Emulations,
}

pub struct BuildContext<'a> {
    pub build_cache: &'a mut BuildCache,
    pub file_build_cache: &'a mut FileBuildCache,
    pub known_addresses: &'a mut KnownAddresses,
    pub known_code_cells: &'a mut HashMap<String, String>,
    pub need_debug_info: bool,
    pub backtrace: Option<String>,
}

pub enum DebugCtx<'a> {
    Disabled,
    Enabled { inner: &'a mut DebugContext },
}

impl<'a> Context<'a> {
    pub fn network(&self) -> String {
        self.network.clone().unwrap_or("testnet".to_owned())
    }
}

impl<'a> Env<'a> {
    pub fn find_contract(&self, name: &str) -> Option<ContractConfig> {
        let contracts = self.config.contracts.clone()?.contracts;
        contracts.get(name).cloned()
    }

    pub fn find_wallet_by_address(&self, addr: &IntAddr) -> Option<Wallet> {
        let found = self
            .open_wallets
            .iter()
            .find(|(_, w)| w.wallet.address.to_hex() == addr.to_string())?;

        Some(found.1.clone())
    }

    pub fn find_wallet(&self, name: &str) -> Option<&crate::config::WalletConfig> {
        self.wallets?.wallets.get(name)
    }
}

impl<'a> AssertsContext<'a> {
    pub fn fail(&mut self, message: String) {
        *self.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
            message: Some(message),
            location: None,
        }));
    }
}

impl<'a> ChainContext<'a> {
    pub fn build_libs(&self, owner: &IntAddr) -> Dict<HashBytes, LibDescr> {
        let std_address = owner.as_std().expect("VarAddr is unexpected");
        self.build_libs_with_hash_owner(&std_address.address)
    }

    pub fn build_libs_with_hash_owner(&self, owner: &HashBytes) -> Dict<HashBytes, LibDescr> {
        let mut libs = Dict::<HashBytes, LibDescr>::new();
        for lib in self.blockchain.libs().iter() {
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
    pub fn new(inner: &'a mut DebugContext) -> DebugCtx<'a> {
        DebugCtx::Enabled { inner }
    }

    pub fn is_enabled(&self) -> bool {
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
