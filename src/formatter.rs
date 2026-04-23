use crate::commands::test::reporting::{FailedTransactionContext, TestReport};
use crate::commands::test::trace::TransactionInfo;
use crate::context;
use crate::context::{
    AssertFailure, BuildCache, DisplayParam, EmulationsState, GetMethodAssertFailure,
    KnownAddresses, TransactionGenericAssertFailure, WalletNotFoundFailure, to_cell,
};
use crate::retrace::{
    self, ExecutedAction, InstalledAction, InstalledActions, InvalidAction, TolkBacktraceFrame,
};
use acton_config::color::OwoColorize;
use acton_config::test::BacktraceMode;
use acton_debug::exit_codes;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;
use std::sync::Arc;
use tolkc::abi::{ContractABI as CompilerContractABI, Ty as CompilerAbiType};
use ton_abi::abi_serde::Data as ParsedAbiData;
use ton_abi::compiler_abi_serde;
use ton_abi::{ContractAbi, TypeAbi};
use ton_api::Network;
use ton_source_map::SourceLocation;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellSlice, HashBytes, Load};
use tycho_types::dict;
use tycho_types::models::{
    AccountState, AccountStatus, Base64StdAddrFlags, ComputePhase, ComputePhaseSkipReason,
    DisplayBase64StdAddr, ExecutedComputePhase, IntAddr, Message, MsgInfo, RelaxedMessage,
    RelaxedMsgInfo, ReserveCurrencyFlags, SendMsgFlags, ShardAccount, StdAddr, Transaction, TxInfo,
};
use tycho_types::num::Tokens;

#[derive(Debug, Clone)]
struct SendResult {
    tx: Transaction,
    children_ids: Vec<i64>,
    parent_lt: Option<i64>,
    #[allow(dead_code)]
    actions: Cell,
    #[allow(dead_code)]
    out_messages: Vec<Cell>,
    externals: Vec<Cell>,
}

#[derive(Debug, Clone)]
struct TransactionNode {
    send_result: SendResult,
    children: Vec<TransactionNode>,
}

#[derive(Debug)]
struct DecodedMessageBody {
    name: String,
    data: ParsedAbiData,
}

enum FormattedExtraInfo {
    Tree(String),
    Annotation(String),
}

#[derive(Debug, Clone)]
pub(crate) struct AbiExitCodeInfo {
    pub symbolic_name: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy)]
enum MapScalarType {
    Int { bits: u16, signed: bool },
    VarInt { len_bits: u8, signed: bool },
    Bool,
    Address,
    Cell,
    String,
}

impl MapScalarType {
    const fn bit_len(self) -> u16 {
        match self {
            Self::Int { bits, .. } => bits,
            Self::Bool => 1,
            // Std addr without anycast.
            Self::Address => StdAddr::BITS_WITHOUT_ANYCAST,
            // Variable-length integers are not valid dict keys.
            Self::VarInt { .. } | Self::Cell | Self::String => 0,
        }
    }
}

/// Context for formatting `TupleItems` with rich information
#[derive(Debug, Clone)]
pub struct FormatterContext<'a> {
    pub contract_abi: Arc<ContractAbi>,
    pub accounts: Cow<'a, FxHashMap<StdAddr, ShardAccount>>,
    pub build_cache: Cow<'a, BuildCache>,
    pub emulations: Cow<'a, EmulationsState>,
    pub known_addresses: Cow<'a, KnownAddresses>,
    pub known_code_cells: Cow<'a, FxHashMap<HashBytes, String>>,
    pub show_bodies: bool,
    pub has_wallets_config: bool,
    pub available_wallets: Vec<String>,
    pub backtrace: Option<BacktraceMode>,
    pub fork_net: Option<Network>,
    pub network: Option<Network>,
}

impl<'a> FormatterContext<'a> {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            contract_abi: Arc::new(ContractAbi::default()),
            accounts: Cow::Owned(FxHashMap::default()),
            build_cache: Cow::Owned(BuildCache::new()),
            emulations: Cow::Owned(EmulationsState::new()),
            known_addresses: Cow::Owned(KnownAddresses::new()),
            known_code_cells: Cow::Owned(FxHashMap::default()),
            show_bodies: false,
            has_wallets_config: false,
            available_wallets: vec![],
            backtrace: None,
            fork_net: None,
            network: None,
        }
    }

    /// Create formatter context from the main Context
    #[must_use]
    pub fn from_context<'b: 'a>(ctx: &'b context::Context<'a>) -> Self {
        Self {
            contract_abi: ctx.env.abi.clone(),
            accounts: Cow::Borrowed(ctx.chain.world_state.get_accounts()),
            build_cache: Cow::Borrowed(ctx.build.build_cache),
            emulations: Cow::Borrowed(ctx.chain.emulations),
            known_addresses: Cow::Borrowed(ctx.build.known_addresses),
            known_code_cells: Cow::Borrowed(ctx.build.known_code_cells),
            show_bodies: ctx.env.show_bodies,
            has_wallets_config: ctx.env.wallets.is_some(),
            available_wallets: ctx.env.open_wallets.keys().cloned().collect(),
            backtrace: ctx.build.backtrace,
            fork_net: ctx.env.fork_net.clone(),
            network: ctx.network.clone(),
        }
    }

    fn compiler_abi_symbol_description(
        compiler_abi: &CompilerContractABI,
        symbol: &str,
    ) -> Option<String> {
        if let Some((enum_name, member_name)) = symbol.rsplit_once('.')
            && let Some(description) = compiler_abi
                .declarations
                .iter()
                .find_map(|declaration| match declaration {
                    tolkc::abi::ABIDeclaration::Enum { name, members, .. } if name == enum_name => {
                        members
                            .iter()
                            .find(|member| member.name == member_name)
                            .map(|member| member.description.as_str())
                    }
                    _ => None,
                })
                .filter(|description| !description.is_empty())
        {
            return Some(description.to_owned());
        }

        compiler_abi
            .constants
            .iter()
            .find(|constant| constant.name == symbol && !constant.description.is_empty())
            .map(|constant| constant.description.clone())
    }

    fn compiler_abi_symbol_name(compiler_abi: &CompilerContractABI, code: i32) -> Option<String> {
        compiler_abi
            .thrown_errors
            .iter()
            .find(|error| error.err_code == code && !error.name.is_empty())
            .map(|thrown| thrown.name.clone())
    }

    #[must_use]
    pub(crate) fn find_custom_exit_code_info(
        code: i32,
        abi: Option<&ContractAbi>,
        compiler_abi: Option<&CompilerContractABI>,
    ) -> Option<AbiExitCodeInfo> {
        if let Some(compiler_abi) = compiler_abi
            && let Some(symbolic_name) = Self::compiler_abi_symbol_name(compiler_abi, code)
        {
            let description = Self::compiler_abi_symbol_description(compiler_abi, &symbolic_name)
                .unwrap_or_else(|| symbolic_name.clone());
            return Some(AbiExitCodeInfo {
                symbolic_name,
                description,
            });
        }

        let exit_code = abi?.exit_codes.iter().find(|item| item.value == code)?;
        let symbolic_name = exit_code.constant_name.clone();
        let description = compiler_abi
            .and_then(|abi| Self::compiler_abi_symbol_description(abi, &symbolic_name))
            .unwrap_or_else(|| symbolic_name.clone());

        Some(AbiExitCodeInfo {
            symbolic_name,
            description,
        })
    }

    fn find_tx_custom_exit_code_info(
        &self,
        tx: &Transaction,
        code: i32,
    ) -> Option<AbiExitCodeInfo> {
        let code_cell = Self::account_code(&self.accounts, &StdAddr::new(0, tx.account));
        let (_, build) = self.build_cache.result_for_code(&code_cell)?;
        Self::find_custom_exit_code_info(code, build.abi.as_deref(), build.compiler_abi.as_deref())
    }

    fn find_code_custom_exit_code_info(
        &self,
        code_boc64: &str,
        exit_code: i32,
    ) -> Option<AbiExitCodeInfo> {
        let code_cell = Boc::decode_base64(code_boc64).ok();
        let (_, build) = self.build_cache.result_for_code(&code_cell)?;
        Self::find_custom_exit_code_info(
            exit_code,
            build.abi.as_deref(),
            build.compiler_abi.as_deref(),
        )
    }

    #[must_use]
    pub fn format_wallet_not_found_message(&self, failure: &WalletNotFoundFailure) -> String {
        if !self.has_wallets_config || self.available_wallets.is_empty() {
            format!(
                "Wallet {} not found in wallets.toml or global.wallets.toml. Wallets are not configured yet.

To add wallets, run {} or add the following section to your wallets.toml:

{}
[wallets.{}]
type = \"v4r2\"
workchain = 0
keys = {{ mnemonic-env = \"WALLET_MNEMONIC\" }}

[wallets.deployer.expected]
address-testnet = \"<<ADDRESS>>\"

See https://ton-blockchain.github.io/acton/docs/tutorial/setup-wallets for more information
",
                failure.wallet_name.yellow(),
                "acton wallet new".green(),
                "# Example wallet configuration".dimmed(),
                failure.wallet_name
            )
        } else {
            let available = self
                .available_wallets
                .iter()
                .map(|s| format!("  {}", s.yellow()))
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                "Wallet {} not found in Acton.toml\nAvailable wallets:\n{}",
                failure.wallet_name.yellow(),
                available
            )
        }
    }

    fn format_slice(&self, slice: &Cell) -> String {
        let mut parser = slice.as_slice_allow_exotic();

        if parser.size_bits() == 2 && parser.load_small_uint(2).unwrap_or(1) == 0 {
            return "addr_none".to_string();
        }

        if parser.size_bits() == 267
            && let Ok(address) = IntAddr::load_from(&mut parser)
        {
            return self.address_to_string(&address);
        }

        Boc::encode_hex(slice)
    }

    fn address_to_string(&self, address: &IntAddr) -> String {
        match address {
            IntAddr::Std(addr) => {
                let need_mainnet_address = self.fork_net == Some(Network::Mainnet)
                    || self.network == Some(Network::Mainnet);

                let display = DisplayBase64StdAddr {
                    addr,
                    flags: Base64StdAddrFlags {
                        testnet: !need_mainnet_address,
                        base64_url: true,
                        bounceable: true,
                    },
                };
                display.to_string()
            }
            _ => address.to_string(),
        }
    }

    fn format_annotation_address(&self, address: &IntAddr) -> String {
        let rendered = self.address_to_string(address);
        let Some(contract_type) = self.get_contract_type(address) else {
            return rendered;
        };

        format!("{rendered} ({contract_type})")
    }

    fn format_address_slice(&self, slice: &Cell, colorize: bool) -> String {
        let mut parser = slice.as_slice_allow_exotic();
        let Ok(addr) = IntAddr::load_from(&mut parser) else {
            return Boc::encode_hex(slice);
        };

        let addr_base64 = self.address_to_string(&addr);

        let addr_str = if colorize {
            addr_base64.cyan().to_string()
        } else {
            addr_base64
        };

        let contract_type = self.get_contract_type(&addr);
        if let Some(contract_type) = contract_type {
            return format!("{addr_str} ({contract_type})");
        }

        addr_str
    }

    /// Format transaction list as a tree
    #[must_use]
    pub fn format_transaction_list(&self, items: &[TupleItem]) -> String {
        let send_results = self.parse_send_results(items);
        let known_contracts = self.collect_known_contracts(&send_results);
        let contract_letters = self.create_contract_letters(&known_contracts);

        if let [send_result] = &send_results[..]
            && self.is_broadcast_synthetic_send_result(send_result)
        {
            return self.format_broadcast_synthetic_send_result(send_result, &contract_letters);
        }

        let tree = self.build_transaction_tree(send_results);
        self.format_transaction_tree(&tree, &contract_letters, 0, "")
    }

    fn is_broadcast_synthetic_send_result(&self, send_result: &SendResult) -> bool {
        let tx = &send_result.tx;

        if tx.lt != 0 || tx.prev_trans_lt != 0 {
            return false;
        }
        if tx.orig_status != AccountStatus::Uninit || tx.end_status != AccountStatus::Uninit {
            return false;
        }
        if tx.out_msg_count != 0 {
            return false;
        }
        if send_result.parent_lt.is_some() || !send_result.children_ids.is_empty() {
            return false;
        }

        let Ok(TxInfo::Ordinary(info)) = tx.load_info() else {
            return false;
        };

        if info.action_phase.is_some()
            || info.storage_phase.is_some()
            || info.credit_phase.is_some()
            || info.aborted
            || info.destroyed
        {
            return false;
        }

        matches!(
            info.compute_phase,
            ComputePhase::Skipped(skipped)
                if skipped.reason == ComputePhaseSkipReason::NoState
        )
    }

    fn format_broadcast_synthetic_send_result(
        &self,
        send_result: &SendResult,
        contract_letters: &HashMap<IntAddr, String>,
    ) -> String {
        let mut lines = vec!["Broadcast send (synthetic result)".to_owned()];
        let mut message = self.format_message_part(&send_result.tx, contract_letters, true);

        if let Some(in_msg_cell) = &send_result.tx.in_msg
            && let Ok(in_msg) = in_msg_cell.parse::<RelaxedMessage>()
            && let RelaxedMsgInfo::Int(info) = &in_msg.info
            && info.src.is_none()
        {
            message = format!("{}{}", "N/A".dimmed(), message);
        }

        if message.is_empty() {
            lines.push(format!(
                "└── submitted to network; call {} to confirm inclusion",
                "res.waitForFirstTransaction()".yellow()
            ));
            return lines.join("\n");
        }

        lines.push(format!("└── {message}"));
        lines.push(format!(
            "    └── submitted to network; call {} to confirm inclusion",
            "res.waitForFirstTransaction()".yellow()
        ));
        lines.join("\n")
    }

    /// Parse transaction items into `SendResult` structures
    fn parse_send_results(&self, tx_items: &[TupleItem]) -> Vec<SendResult> {
        let tx_items = Self::flatten_big_array_items(tx_items).unwrap_or_else(|| tx_items.to_vec());

        tx_items
            .iter()
            .filter_map(|el| {
                let TupleItem::Tuple(tuple) = el else {
                    return None;
                };

                let (
                    Some(TupleItem::Cell(tx)),
                    Some(TupleItem::Tuple(child_ids)),
                    Some(TupleItem::Cell(actions)),
                    Some(TupleItem::Tuple(out_messages)),
                    Some(TupleItem::Tuple(externals)),
                ) = (
                    tuple.first(),
                    tuple.get(2),
                    tuple.get(4),
                    tuple.get(5),
                    tuple.get(7), // externals
                )
                else {
                    return None;
                };

                let tx = tx.parse::<Transaction>().ok()?;
                Some(SendResult {
                    tx,
                    children_ids: child_ids
                        .iter()
                        .filter_map(|id| match id {
                            TupleItem::Int(int) => int.to_i64(),
                            _ => None,
                        })
                        .collect(),
                    parent_lt: match tuple.get(3) {
                        Some(TupleItem::Int(int)) => int.to_i64(),
                        _ => None,
                    },
                    actions: actions.clone(),
                    out_messages: out_messages
                        .iter()
                        .filter_map(|msg| match msg {
                            TupleItem::Cell(cell) => Some(cell.clone()),
                            _ => None,
                        })
                        .collect(),
                    externals: externals
                        .iter()
                        .filter_map(|ext| match ext {
                            TupleItem::Cell(cell) => Some(cell.clone()),
                            _ => None,
                        })
                        .collect(),
                })
            })
            .collect::<Vec<_>>()
    }

    fn flatten_big_array_items(items: &[TupleItem]) -> Option<Vec<TupleItem>> {
        // [topLevel: array<array<T>>, size: int]
        let [TupleItem::Tuple(top_level), TupleItem::Int(size)] = items else {
            return None;
        };

        let size = size.to_usize()?;
        let mut result = Vec::with_capacity(size);

        for bin in top_level.iter() {
            let TupleItem::Tuple(bin_items) = bin else {
                return None;
            };

            for item in bin_items.iter() {
                if result.len() == size {
                    break;
                }
                result.push(item.clone());
            }

            if result.len() == size {
                break;
            }
        }

        if result.len() != size {
            return None;
        }

        Some(result)
    }

    /// Collect all known contract addresses from send results
    fn collect_known_contracts(&self, send_results: &[SendResult]) -> Vec<IntAddr> {
        let mut known_contracts: Vec<IntAddr> = vec![];

        for send_result in send_results {
            let Ok(Some(in_msg)) = send_result.tx.load_in_msg() else {
                continue;
            };

            if let MsgInfo::Int(info) = &in_msg.info {
                // It's O(N) but we need order, and we don't have many (thousands) transactions
                if !known_contracts.contains(&info.src) {
                    known_contracts.push(info.src.clone());
                }
                if !known_contracts.contains(&info.dst) {
                    known_contracts.push(info.dst.clone());
                }
            }

            #[allow(clippy::collapsible_if)]
            if let MsgInfo::ExtIn(info) = &in_msg.info {
                if !known_contracts.contains(&info.dst) {
                    known_contracts.push(info.dst.clone());
                }
            }
        }

        known_contracts
    }

    /// Create letter mappings for contract addresses
    fn create_contract_letters(&self, known_contracts: &[IntAddr]) -> HashMap<IntAddr, String> {
        let mut contract_letters: HashMap<IntAddr, String> = HashMap::new();

        for (index, addr) in known_contracts.iter().enumerate() {
            let letter = ((index % 26) as u8 + b'A') as char;

            let letter_str = if index < 26 {
                letter.to_string()
            } else {
                let cycle = index / 26;
                format!("{letter}{cycle}")
            };

            contract_letters.insert(addr.clone(), letter_str);
        }

        contract_letters
    }

    /// Build transaction tree from `SendResult` list
    fn build_transaction_tree(&self, send_results: Vec<SendResult>) -> Vec<TransactionNode> {
        let mut lt_to_result: HashMap<i64, SendResult> = HashMap::new();

        for result in send_results {
            lt_to_result.insert(result.tx.lt as i64, result);
        }

        let mut roots = Vec::new();
        let mut processed = HashSet::new();

        for (lt, result) in &lt_to_result {
            if processed.contains(lt) {
                continue;
            }

            if result.parent_lt.is_none()
                || !lt_to_result.contains_key(&result.parent_lt.unwrap_or(-1))
            {
                let node = Self::build_node_recursive(*lt, &lt_to_result, &mut processed);
                if let Some(node) = node {
                    roots.push(node);
                }
            }
        }

        roots.sort_by_key(|node| node.send_result.tx.lt);
        roots
    }

    /// Recursively build transaction tree node
    fn build_node_recursive(
        lt: i64,
        lt_to_result: &HashMap<i64, SendResult>,
        processed: &mut HashSet<i64>,
    ) -> Option<TransactionNode> {
        if !processed.insert(lt) {
            return None;
        }

        let result = lt_to_result.get(&lt)?;

        let mut children = Vec::new();
        for child_lt in &result.children_ids {
            let child_node = Self::build_node_recursive(*child_lt, lt_to_result, processed);
            if let Some(child_node) = child_node {
                children.push(child_node);
            }
        }
        children.sort_by_key(|node| node.send_result.tx.lt);

        Some(TransactionNode {
            send_result: result.clone(),
            children,
        })
    }

    /// Format transaction tree with proper indentation
    fn format_transaction_tree(
        &self,
        nodes: &[TransactionNode],
        contract_letters: &HashMap<IntAddr, String>,
        depth: usize,
        prefix: &str,
    ) -> String {
        let mut result = String::new();

        for (i, node) in nodes.iter().enumerate() {
            let is_last_child = i == nodes.len() - 1;

            if depth > 0 {
                result.push_str(prefix);
                if is_last_child {
                    result.push_str("└── ".dimmed().to_string().as_str());
                } else {
                    result.push_str("├── ".dimmed().to_string().as_str());
                }
            }

            let child_prefix = if depth > 0 {
                if is_last_child {
                    format!("{}{}", prefix, "    ")
                } else {
                    format!("{}{}", prefix, "│   ".dimmed())
                }
            } else {
                "    ".to_string()
            };

            let has_children = !node.children.is_empty();
            let prefix_len = if depth > 0 {
                visible_len(prefix) + 4
            } else {
                4 // 4 for "└── " added in format_single_transaction
            };
            let tx_formatted = if depth == 0 {
                self.format_single_transaction(
                    &node.send_result,
                    contract_letters,
                    true,
                    &child_prefix,
                    has_children,
                    prefix_len,
                    true, // is_root
                )
            } else {
                self.format_single_transaction(
                    &node.send_result,
                    contract_letters,
                    false,
                    &child_prefix,
                    has_children,
                    prefix_len,
                    false, // is_root
                )
            };
            result.push_str(&tx_formatted);
            result.push('\n');

            if !node.children.is_empty() {
                let children_formatted = self.format_transaction_tree(
                    &node.children,
                    contract_letters,
                    depth + 1,
                    &child_prefix,
                );
                result.push_str(&children_formatted);
            }
        }

        result
    }

    /// Format a single transaction
    #[allow(clippy::too_many_arguments)]
    fn format_single_transaction(
        &self,
        send_result: &SendResult,
        contract_letters: &HashMap<IntAddr, String>,
        show_full_names: bool,
        child_prefix: &str,
        has_children: bool,
        prefix_len: usize,
        is_root: bool,
    ) -> String {
        let tx = &send_result.tx;
        let mut tx_builder = String::new();

        let main_part = self.format_message_part(tx, contract_letters, false);
        let main_part_visible_len = visible_len(&main_part);

        if is_root {
            let in_msg = &tx.load_in_msg();
            if let Ok(Some(in_msg)) = in_msg {
                if let MsgInfo::Int(info) = &in_msg.info {
                    let src_addr = info.src.clone();
                    let src_formatted = self.format_address_with_letter(
                        &src_addr,
                        contract_letters,
                        show_full_names,
                    );
                    tx_builder += &format!(
                        "{} {} {}\n",
                        "N/A".dimmed(),
                        "->".dimmed(),
                        src_formatted.trim()
                    );
                } else if matches!(&in_msg.info, MsgInfo::ExtIn(_)) {
                    tx_builder += &format!(
                        "{} {} {}\n",
                        "N/A".dimmed(),
                        "->".dimmed(),
                        "external".dimmed()
                    );
                }
                tx_builder += "└── ".dimmed().to_string().as_str();
            }
        }

        tx_builder += &main_part;
        tx_builder += &self.format_transaction_info(
            tx,
            send_result,
            child_prefix,
            has_children,
            main_part_visible_len,
            prefix_len,
            contract_letters,
        );

        tx_builder
    }

    /// Format the message part of a transaction (sender -> message -> receiver)
    fn format_message_part(
        &self,
        tx: &Transaction,
        contract_letters: &HashMap<IntAddr, String>,
        show_full_names: bool,
    ) -> String {
        if let Some(in_msg) = &tx.in_msg
            && let Ok(in_msg) = in_msg.parse::<RelaxedMessage>()
        {
            let resolved_body = self.resolve_incoming_message_body(&in_msg);
            let message_part = self.format_single_message(
                &in_msg,
                contract_letters,
                show_full_names,
                resolved_body.as_ref(),
            );
            if !message_part.is_empty() {
                return message_part;
            }
        }

        if let Ok(Some(in_msg)) = tx.load_in_msg()
            && let MsgInfo::ExtIn(info) = &in_msg.info
        {
            let resolved_body = self.resolve_external_incoming_message_body(tx, &in_msg);
            let message_name = resolved_body.as_ref().map_or_else(
                || {
                    let mut body = in_msg.body;
                    self.get_message_name(body.load_u32().unwrap_or(0))
                },
                |body| Self::color_message_name(&body.name),
            );
            let destination = self.format_address_with_letter(&info.dst, contract_letters, true);

            return format!(
                "{} {} {} {}",
                "ext-in".blue(),
                message_name,
                "->".dimmed(),
                destination
            );
        }

        String::new()
    }

    fn format_single_message(
        &self,
        in_msg: &RelaxedMessage,
        contract_letters: &HashMap<IntAddr, String>,
        show_full_names: bool,
        resolved_body: Option<&DecodedMessageBody>,
    ) -> String {
        let RelaxedMsgInfo::Int(info) = &in_msg.info else {
            if let RelaxedMsgInfo::ExtOut(_) = &in_msg.info {
                let Some(msg_info) = self.format_ext_out_message(in_msg) else {
                    return String::new();
                };

                return msg_info;
            }
            return String::new();
        };

        let mut result = String::new();

        if info.bounced {
            result += "(!) ".red().to_string().as_str();
        }

        if let Some(src) = &info.src {
            result += &self.format_address_with_letter(src, contract_letters, show_full_names);
        }
        if show_full_names {
            result += " -> ".dimmed().to_string().as_str();
        }

        let message_name = resolved_body.map_or_else(
            || self.get_message_name(Self::extract_opcode(in_msg)),
            |body| Self::color_message_name(&body.name),
        );
        result += &message_name;
        result += " ";

        result += self.format_ton_tokens(info.value.tokens).as_str();
        result += " -> ".dimmed().to_string().as_str();

        result += &self.format_address_with_letter(&info.dst, contract_letters, true);

        result
    }

    fn format_ton_tokens(&self, tokens: Tokens) -> String {
        let amount = tokens.into_inner() as f64 / 1e9;
        format!("{amount} TON").green().to_string()
    }

    fn format_ton(&self, amount: &BigInt) -> String {
        let amount = amount.to_f64().unwrap_or(0.0) / 1e9;
        format!("{amount} TON").green().to_string()
    }

    fn format_inbound_message_body(&self, tx: &Transaction) -> Option<String> {
        if !self.show_bodies {
            return None;
        }

        if let Some(in_msg) = tx.in_msg.as_ref()
            && let Ok(in_msg) = in_msg.parse::<RelaxedMessage>()
            && let Some(body) = self.resolve_incoming_message_body(&in_msg)
        {
            return Some(self.format_decoded_message_body(&body));
        }

        let in_msg = tx.load_in_msg().ok()??;
        let body = self.resolve_external_incoming_message_body(tx, &in_msg)?;
        Some(self.format_decoded_message_body(&body))
    }

    fn resolve_incoming_message_body(&self, in_msg: &RelaxedMessage) -> Option<DecodedMessageBody> {
        let build = self.build_result_for_incoming_message(in_msg)?;
        let compiler_abi = build.compiler_abi.as_ref()?;
        match &in_msg.info {
            RelaxedMsgInfo::Int(info) => self.try_decode_message_body_types(
                in_msg.body,
                compiler_abi,
                compiler_abi
                    .incoming_messages
                    .iter()
                    .map(|message| &message.body_ty),
                if info.bounced { 32 } else { 0 },
            ),
            RelaxedMsgInfo::ExtOut(_) => None,
        }
    }

    fn format_outgoing_external_message(
        &self,
        tx: &Transaction,
        msg: &RelaxedMessage,
    ) -> Option<Vec<FormattedExtraInfo>> {
        let RelaxedMsgInfo::ExtOut(info) = &msg.info else {
            return None;
        };

        let resolved_body = self.resolve_outgoing_external_message_body(tx, msg);
        let message_name = resolved_body.as_ref().map_or_else(
            || self.get_message_name(Self::extract_opcode(msg)),
            |body| Self::color_message_name(&body.name),
        );

        let mut infos = Vec::new();
        if let Some(ext_addr) = &info.dst {
            let hex_data = hex::encode(&ext_addr.data);
            infos.push(FormattedExtraInfo::Tree(format!(
                "{} {} {} {} {}",
                "ext-out".blue(),
                message_name,
                "->".dimmed(),
                format!("0x{hex_data}").cyan(),
                format!("({} bits)", ext_addr.data_bit_len).dimmed(),
            )));
        } else {
            infos.push(FormattedExtraInfo::Tree(format!(
                "{} {} {} {}",
                "ext-out".blue(),
                message_name,
                "->".dimmed(),
                "none".cyan()
            )));
        }

        if self.show_bodies
            && let Some(body) = resolved_body
        {
            infos.push(FormattedExtraInfo::Annotation(
                self.format_decoded_message_body(&body),
            ));
        }

        Some(infos)
    }

    fn resolve_external_incoming_message_body(
        &self,
        tx: &Transaction,
        in_msg: &Message<'_>,
    ) -> Option<DecodedMessageBody> {
        let MsgInfo::ExtIn(_) = &in_msg.info else {
            return None;
        };

        let build = self.build_result_for_tx_account(tx)?;
        let compiler_abi = build.compiler_abi.as_ref()?;
        self.try_decode_message_body_types(
            in_msg.body,
            compiler_abi,
            compiler_abi
                .incoming_external
                .iter()
                .map(|message| &message.body_ty),
            0,
        )
    }

    fn resolve_outgoing_external_message_body(
        &self,
        tx: &Transaction,
        msg: &RelaxedMessage,
    ) -> Option<DecodedMessageBody> {
        let build = self.build_result_for_tx_account(tx)?;
        let compiler_abi = build.compiler_abi.as_ref()?;
        self.try_decode_message_body_types(
            msg.body,
            compiler_abi,
            compiler_abi
                .emitted_events
                .iter()
                .map(|message| &message.body_ty),
            0,
        )
    }

    fn build_result_for_incoming_message(
        &self,
        in_msg: &RelaxedMessage,
    ) -> Option<context::CompilationResult> {
        let dst = match &in_msg.info {
            RelaxedMsgInfo::Int(info) => Some(&info.dst),
            RelaxedMsgInfo::ExtOut(_) => return None,
        };
        self.build_result_for_address(dst)
    }

    fn build_result_for_tx_account(&self, tx: &Transaction) -> Option<context::CompilationResult> {
        let code = self
            .accounts
            .iter()
            .find_map(|(addr, _)| {
                (addr.address == tx.account).then(|| Self::account_code(&self.accounts, addr))
            })
            .flatten();
        self.build_cache
            .result_for_code(&code)
            .map(|(_, result)| result)
    }

    fn build_result_for_address(
        &self,
        dst: Option<&IntAddr>,
    ) -> Option<context::CompilationResult> {
        let code = match dst? {
            IntAddr::Std(addr) => Self::account_code(&self.accounts, addr),
            IntAddr::Var(_) => None,
        };
        self.build_cache
            .result_for_code(&code)
            .map(|(_, result)| result)
    }

    fn try_decode_message_body_types<'msg, I>(
        &self,
        body: CellSlice<'msg>,
        abi: &CompilerContractABI,
        candidates: I,
        prefix_to_skip: u16,
    ) -> Option<DecodedMessageBody>
    where
        I: IntoIterator<Item = &'msg CompilerAbiType>,
    {
        for body_ty in candidates {
            let mut parser = body;
            if prefix_to_skip > 0 && parser.skip_first(prefix_to_skip, 0).is_err() {
                continue;
            }

            let Ok(data) = compiler_abi_serde::decode(&mut parser, abi, body_ty) else {
                continue;
            };
            if parser.size_bits() != 0 || parser.size_refs() != 0 {
                continue;
            }

            return Some(DecodedMessageBody {
                name: Self::compiler_body_type_name(body_ty),
                data,
            });
        }

        None
    }

    fn compiler_body_type_name(body_ty: &CompilerAbiType) -> String {
        match body_ty {
            CompilerAbiType::StructRef { struct_name, .. } => struct_name.clone(),
            CompilerAbiType::AliasRef { alias_name, .. } => alias_name.clone(),
            CompilerAbiType::EnumRef { enum_name } => enum_name.clone(),
            _ => body_ty.render_type(),
        }
    }

    fn format_decoded_message_body(&self, body: &DecodedMessageBody) -> String {
        self.format_annotation_body(&body.data)
    }

    fn format_annotation_body(&self, data: &ParsedAbiData) -> String {
        let data = Self::unwrap_annotation_data(data);
        match data {
            ParsedAbiData::Object(object) => self.format_annotation_object(object, 0, true),
            _ => self.format_annotation_value(data, 0),
        }
    }

    fn format_annotation_object(
        &self,
        object: &ton_abi::abi_serde::DataObject,
        indent: usize,
        is_root: bool,
    ) -> String {
        if object.fields.is_empty() {
            return "{}".to_owned();
        }

        if object.fields.len() <= 2
            && object
                .fields
                .iter()
                .all(|field| Self::is_annotation_scalar(&field.value))
        {
            let inner = object
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        field.name,
                        self.format_annotation_scalar(&field.value)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            return if is_root {
                inner
            } else {
                format!("{{ {inner} }}")
            };
        }

        let indent_str = "    ".repeat(Self::annotation_container_closing_indent(indent));
        let field_indent = if is_root {
            "    ".repeat(indent)
        } else {
            "    ".repeat(Self::annotation_container_inner_indent(indent))
        };
        let mut lines = if is_root {
            Vec::new()
        } else {
            vec!["{".to_owned()]
        };
        for field in &object.fields {
            let value = self.format_annotation_value(&field.value, indent + 1);
            let mut value_lines = value.lines();
            if let Some(first) = value_lines.next() {
                lines.push(format!("{field_indent}{}: {first}", field.name));
                lines.extend(value_lines.map(str::to_owned));
            }
        }
        if !is_root {
            lines.push(format!("{indent_str}}}"));
        }
        lines.join("\n")
    }

    fn format_annotation_value(&self, data: &ParsedAbiData, indent: usize) -> String {
        let data = Self::unwrap_annotation_data(data);
        match data {
            ParsedAbiData::Object(object) => self.format_annotation_object(object, indent, false),
            ParsedAbiData::Array(items) => self.format_annotation_array(items, indent),
            ParsedAbiData::Map(entries) => self.format_annotation_map(entries, indent),
            _ => self.format_annotation_scalar(data),
        }
    }

    fn format_annotation_array(&self, items: &[ParsedAbiData], indent: usize) -> String {
        if items.is_empty() {
            return "[]".to_owned();
        }

        if items.len() <= 3 && items.iter().all(Self::is_annotation_scalar) {
            let inner = items
                .iter()
                .map(|item| self.format_annotation_scalar(item))
                .collect::<Vec<_>>()
                .join(", ");
            return format!("[{inner}]");
        }

        let indent_str = "    ".repeat(Self::annotation_container_closing_indent(indent));
        let item_indent = "    ".repeat(Self::annotation_container_inner_indent(indent));
        let mut lines = vec!["[".to_owned()];
        for item in items {
            let value = self.format_annotation_value(item, indent + 1);
            let mut value_lines = value.lines();
            if let Some(first) = value_lines.next() {
                lines.push(format!("{item_indent}{first}"));
                lines.extend(value_lines.map(str::to_owned));
            }
        }
        lines.push(format!("{indent_str}]"));
        lines.join("\n")
    }

    fn format_annotation_map(
        &self,
        entries: &[(ParsedAbiData, ParsedAbiData)],
        indent: usize,
    ) -> String {
        if entries.is_empty() {
            return "{}".to_owned();
        }

        let indent_str = "    ".repeat(Self::annotation_container_closing_indent(indent));
        let entry_indent = "    ".repeat(Self::annotation_container_inner_indent(indent));
        let mut lines = vec!["{".to_owned()];
        for (key, value) in entries {
            let key = self.format_annotation_value(key, indent + 1);
            let value = self.format_annotation_value(value, indent + 1);
            let mut value_lines = value.lines();
            if let Some(first) = value_lines.next() {
                lines.push(format!("{entry_indent}{key} => {first}"));
                lines.extend(value_lines.map(str::to_owned));
            }
        }
        lines.push(format!("{indent_str}}}"));
        lines.join("\n")
    }

    fn format_annotation_scalar(&self, data: &ParsedAbiData) -> String {
        let data = Self::unwrap_annotation_data(data);
        match data {
            ParsedAbiData::Null => "null".to_owned(),
            ParsedAbiData::Number(value) => value.to_string(),
            ParsedAbiData::Bool(value) => value.to_string(),
            ParsedAbiData::String(value) => format!("{value:?}"),
            ParsedAbiData::Symbol(value) => value.clone(),
            ParsedAbiData::Address(value) => self.format_annotation_address(value),
            ParsedAbiData::ExtAddress(value) => value.to_string(),
            ParsedAbiData::Cell(value) | ParsedAbiData::RemainingBitsAndRefs(value) => {
                Boc::encode_hex(value)
            }
            ParsedAbiData::Bits((bytes, bit_len)) => {
                let hex = hex::encode_upper(bytes);
                if bit_len % 8 == 0 {
                    format!("0x{hex}")
                } else {
                    format!("0x{hex} ({bit_len} bits)")
                }
            }
            ParsedAbiData::Object(_) | ParsedAbiData::Array(_) | ParsedAbiData::Map(_) => {
                self.format_annotation_value(data, 0)
            }
        }
    }

    fn is_annotation_scalar(data: &ParsedAbiData) -> bool {
        let data = Self::unwrap_annotation_data(data);
        !matches!(
            data,
            ParsedAbiData::Object(_) | ParsedAbiData::Array(_) | ParsedAbiData::Map(_)
        )
    }

    fn unwrap_annotation_data(mut data: &ParsedAbiData) -> &ParsedAbiData {
        while let ParsedAbiData::Object(object) = data {
            let Some(next) = Self::annotation_wrapper_value(object) else {
                break;
            };
            data = next;
        }
        data
    }

    fn annotation_wrapper_value(object: &ton_abi::abi_serde::DataObject) -> Option<&ParsedAbiData> {
        if object.name == "Cell" && object.fields.len() == 1 && object.fields[0].name == "ref" {
            return Some(&object.fields[0].value);
        }
        None
    }

    const fn annotation_container_inner_indent(indent: usize) -> usize {
        if indent == 0 { 1 } else { indent }
    }

    const fn annotation_container_closing_indent(indent: usize) -> usize {
        indent.saturating_sub(1)
    }

    /// Format transaction execution info (gas, exit code, account changes)
    #[allow(clippy::too_many_arguments)]
    fn format_transaction_info(
        &self,
        tx: &Transaction,
        send_result: &SendResult,
        child_prefix: &str,
        has_children: bool,
        main_part_visible_len: usize,
        prefix_len: usize,
        contract_letters: &HashMap<IntAddr, String>,
    ) -> String {
        let Ok(TxInfo::Ordinary(info)) = tx.load_info() else {
            return "tick-tock message".to_string();
        };

        let mut result = String::new();
        let mut extra_infos = vec![];

        if let Some(body) = self.format_inbound_message_body(tx) {
            extra_infos.push(FormattedExtraInfo::Annotation(body));
        }

        let padding_len = 80usize.saturating_sub(prefix_len + main_part_visible_len);

        if let ComputePhase::Executed(compute) = info.compute_phase {
            // Add padding to align metadata
            result += &" ".repeat(padding_len);
            result += &format!("gas={}", compute.gas_used.to_string().as_str())
                .dimmed()
                .to_string();

            let debug_logs = self.emulations.find_tx_debug_logs(tx.lt);

            if let Some(debug_logs) = debug_logs
                && !debug_logs.is_empty()
            {
                extra_infos.push(FormattedExtraInfo::Tree(format!(
                    "Debug logs:\n{}",
                    debug_logs
                        .lines()
                        .map(|line| format!(
                            "{}    {}",
                            child_prefix,
                            line.trim_start_matches("#DEBUG#: ").dimmed()
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                )));
            }

            if compute.exit_code != 0 {
                result += &self.format_transaction_exit_code(
                    tx,
                    child_prefix,
                    &mut extra_infos,
                    &compute,
                );
            }
        } else {
            result += format!(
                "{}{}",
                " ".repeat(padding_len),
                "compute phase skipped".dimmed()
            )
            .as_str();
        }

        if info.aborted {
            result += " aborted".red().to_string().as_str();
        }

        if tx.orig_status == AccountStatus::NotExists && tx.end_status == AccountStatus::Active {
            extra_infos.push(FormattedExtraInfo::Tree("account created".to_string()));
        }
        if tx.orig_status == AccountStatus::Active && tx.end_status == AccountStatus::NotExists {
            extra_infos.push(FormattedExtraInfo::Tree("account destroyed".to_string()));
        }

        match info.action_phase {
            None => {}
            Some(action) => {
                if action.result_code != 0 {
                    result += &format!(" action_result_code={}", action.result_code)
                        .red()
                        .to_string();

                    extra_infos.push(FormattedExtraInfo::Tree("Action phase failed".to_string()));

                    if let Some(info) = exit_codes::find(action.result_code) {
                        extra_infos.push(FormattedExtraInfo::Tree(format!(
                            "Description: {}",
                            info.description.to_string().yellow()
                        )));
                    }

                    // Trying to collect installed and executed out actions
                    let vm_logs = self.emulations.find_tx_logs(tx.lt);
                    let installed_actions = if let Some(vm_logs) = vm_logs {
                        retrace::find_installed_actions(vm_logs)
                    } else {
                        InstalledActions::empty()
                    };

                    let executor_logs = self.emulations.find_tx_executor_logs(tx.lt);
                    if let Some(logs) = executor_logs {
                        if self.backtrace.is_none() {
                            extra_infos.push(FormattedExtraInfo::Tree(format!(
                                "Re-run with {} to get actions location",
                                "--backtrace full".yellow()
                            )));
                        }

                        let actions = self.format_actions_retrace(
                            child_prefix,
                            tx,
                            installed_actions,
                            logs,
                            contract_letters,
                        );
                        if !actions.is_empty() {
                            extra_infos.push(FormattedExtraInfo::Tree(actions));
                        }
                    }
                }
            }
        }

        for ext_msg in &send_result.externals {
            let Ok(msg) = ext_msg.parse::<RelaxedMessage>() else {
                continue;
            };

            let Some(msg_infos) = self.format_outgoing_external_message(tx, &msg) else {
                continue;
            };

            extra_infos.extend(msg_infos);
        }

        if !extra_infos.is_empty() {
            result += "\n";
        }

        for (idx, info) in extra_infos.iter().enumerate() {
            match info {
                FormattedExtraInfo::Tree(info) => {
                    let has_next_sibling = has_children
                        || extra_infos
                            .iter()
                            .skip(idx + 1)
                            .any(|next| matches!(next, FormattedExtraInfo::Tree(_)));
                    let branch = if has_next_sibling {
                        "├── ".dimmed().to_string()
                    } else {
                        "└── ".dimmed().to_string()
                    };

                    result += child_prefix;
                    result += &branch;

                    let mut lines = info.lines();
                    if let Some(first_line) = lines.next() {
                        result += first_line;
                    }

                    for line in lines {
                        result += "\n";
                        result += child_prefix;

                        let line_without_prefix = line.strip_prefix(child_prefix).unwrap_or(line);
                        if has_next_sibling {
                            result += "│   ".dimmed().to_string().as_str();
                            if let Some(rest) = line_without_prefix.strip_prefix("    ") {
                                result += rest;
                            } else {
                                result += line_without_prefix;
                            }
                        } else {
                            result += line_without_prefix;
                        }
                    }
                }
                FormattedExtraInfo::Annotation(info) => {
                    let is_multiline = info.contains('\n');
                    let has_next_tree = has_children
                        || extra_infos
                            .iter()
                            .skip(idx + 1)
                            .any(|next| matches!(next, FormattedExtraInfo::Tree(_)));
                    for (line_idx, line) in info.lines().enumerate() {
                        if line_idx > 0 {
                            result += "\n";
                        }
                        result += child_prefix;
                        if is_multiline {
                            if has_next_tree {
                                result += "│   ".dimmed().to_string().as_str();
                            } else {
                                result += "    ";
                            }
                        }
                        result += line.dimmed().to_string().as_str();
                    }
                }
            }

            if idx < extra_infos.len() - 1 {
                result += "\n";
            }
        }

        result
    }

    fn format_transaction_exit_code(
        &self,
        tx: &Transaction,
        child_prefix: &str,
        extra_infos: &mut Vec<FormattedExtraInfo>,
        compute: &ExecutedComputePhase,
    ) -> String {
        let mut result = String::new();
        result += &format!(" exit_code={}", compute.exit_code)
            .red()
            .to_string();

        if let Some(info) = exit_codes::find(compute.exit_code) {
            extra_infos.push(FormattedExtraInfo::Tree(format!(
                "Compute phase failed: {}",
                info.description.to_string().yellow()
            )));
        } else if let Some(info) = self.find_tx_custom_exit_code_info(tx, compute.exit_code) {
            extra_infos.push(FormattedExtraInfo::Tree(format!(
                "Compute phase failed: {}",
                info.description.yellow()
            )));
        }

        if let Some(missing_libraries) = self.emulations.find_tx_missing_libraries(tx.lt)
            && !missing_libraries.is_empty()
        {
            let mut missing_libraries = missing_libraries.iter().cloned().collect::<Vec<_>>();
            missing_libraries.sort_unstable();

            if missing_libraries.len() == 1 {
                extra_infos.push(FormattedExtraInfo::Tree(format!(
                    "Library {} is missing, which is what causes this error",
                    missing_libraries.join(", ").yellow()
                )));
            } else {
                extra_infos.push(FormattedExtraInfo::Tree(format!(
                    "Missing libraries: {}",
                    missing_libraries.join(", ").yellow()
                )));
            }
            extra_infos.push(FormattedExtraInfo::Tree(
                "This most likely happened because the library is not registered in tests"
                    .to_owned(),
            ));
            extra_infos.push(FormattedExtraInfo::Tree(format!(
                "To manually register library use {} somewhere in {}-like function",
                "testing.registerLibrary(code)".yellow(),
                "setupTests()".yellow(),
            )));
            extra_infos.push(FormattedExtraInfo::Tree("Learn more about libraries in documentation: https://ton-blockchain.github.io/acton/docs/libraries".to_owned()));
        }

        self.format_transaction_backtrace(tx, child_prefix, extra_infos);

        result
    }

    fn format_transaction_backtrace(
        &self,
        tx: &Transaction,
        child_prefix: &str,
        extra_infos: &mut Vec<FormattedExtraInfo>,
    ) -> Option<()> {
        // Trying to retrace exit code to find out exact Tolk source location
        let logs = self.emulations.find_tx_logs(tx.lt)?;
        let in_msg = tx.load_in_msg().ok()??;

        let dst = match in_msg.info {
            MsgInfo::Int(info) => info.dst,
            MsgInfo::ExtIn(info) => info.dst,
            MsgInfo::ExtOut(_) => return None,
        };
        let dst = match dst {
            IntAddr::Std(addr) => addr,
            IntAddr::Var(_) => return None,
        };

        let code = Self::account_code(&self.accounts, &dst);
        let result = self.build_cache.result_for_code(&code)?;

        let info = retrace::find_exception_info(logs, &result.1.source_map)?;
        let backtrace_result = Self::format_backtrace(&info.backtrace)
            .iter()
            .map(|line| format!("{child_prefix}       {line}"))
            .collect::<Vec<String>>()
            .join("\n");

        let mut message = format!("at {}", Self::format_location(&info.loc).dimmed());
        if !backtrace_result.is_empty() {
            message.push('\n');
            message.push_str(&backtrace_result);
        }

        extra_infos.push(FormattedExtraInfo::Tree(message));

        Some(())
    }

    #[must_use]
    pub(crate) fn format_backtrace(backtrace: &[TolkBacktraceFrame]) -> Vec<String> {
        let max_function_name_len = backtrace
            .iter()
            .map(|frame| frame.function_name.len() + 2)
            .max()
            .unwrap_or(0);

        backtrace
            .iter()
            .map(|frame| {
                format!(
                    "{:<width$} at {}",
                    frame.function_name.green(),
                    Self::format_location(&frame.loc).dimmed(),
                    width = max_function_name_len
                )
            })
            .collect()
    }

    pub(crate) fn format_location(loc: &SourceLocation) -> String {
        format!(
            "{}:{}:{}",
            SourceLocation::normalize_path(&loc.file),
            loc.line,
            loc.column
        )
    }

    pub(crate) fn find_failed_get_method_exception(
        &self,
        test: &TestReport,
    ) -> Option<retrace::TolkExceptionInfo> {
        let failed_get = self
            .emulations
            .results_of(test.name.as_ref())?
            .get_methods
            .iter()
            .rev()
            .find(|result| result.vm_exit_code != 0)?;
        let code = Boc::decode_base64(failed_get.code.as_ref()).ok()?;
        let build = self.build_cache.result_for_code(&Some(code))?.1;

        retrace::find_exception_info(&failed_get.vm_log, &build.source_map)
    }

    fn format_ext_out_message(&self, msg: &RelaxedMessage) -> Option<String> {
        let RelaxedMsgInfo::ExtOut(info) = &msg.info else {
            return None;
        };

        let message_name = self.get_message_name(Self::extract_opcode(msg));

        let msg_info = if let Some(ext_addr) = &info.dst {
            let hex_data = hex::encode(&ext_addr.data);
            format!(
                "{} {} {} {} {}",
                "ext-out".blue(),
                message_name,
                "->".dimmed(),
                format!("0x{hex_data}").cyan(),
                format!("({} bits)", ext_addr.data_bit_len).dimmed(),
            )
        } else {
            format!(
                "{} {} {} {}",
                "ext-out".blue(),
                message_name,
                "->".dimmed(),
                "none".cyan()
            )
        };
        Some(msg_info)
    }

    fn format_actions_retrace(
        &self,
        child_prefix: &str,
        tx: &Transaction,
        installed_actions: InstalledActions,
        logs: &str,
        contract_letters: &HashMap<IntAddr, String>,
    ) -> String {
        let executed = retrace::ExecutedActions::from(logs);

        if executed.actions.is_empty() {
            return self.format_invalid_actions_retrace(
                child_prefix,
                tx,
                &installed_actions,
                &executed.invalid_actions,
            );
        }

        let mut action_parts = Vec::new();

        for action in &executed.actions {
            match action {
                ExecutedAction::SendMessage {
                    hash,
                    remaining_balance,
                    ..
                } => {
                    let message = installed_actions.find_message(hash);

                    let (loc, formatted) = if let Some(message) = message {
                        let msg = message.message();

                        let formatted = match msg {
                            Some(msg) => {
                                self.format_single_message(&msg, contract_letters, false, None)
                            }
                            None => hash.clone(),
                        };

                        (
                            self.find_source_loc(tx, &message.loc_hash, message.loc_offset),
                            formatted,
                        )
                    } else {
                        (None, "msg: ".to_owned() + hash)
                    };

                    let message_part = formatted;
                    let balance_part = format!("balance: {}", self.format_ton(remaining_balance));
                    let location_part = loc
                        .map(|l| format!("at {}", l.format()))
                        .unwrap_or_default();

                    action_parts.push((message_part, balance_part, location_part));
                }
                ExecutedAction::ReserveCurrency {
                    mode,
                    reserve,
                    changed_remaining_balance,
                    ..
                } => {
                    let reserve_action = installed_actions.find_reserve(*mode, reserve);

                    let loc = if let Some(action) = reserve_action {
                        self.find_source_loc(tx, &action.loc_hash, action.loc_offset)
                    } else {
                        None
                    };

                    let mode_flags = ReserveCurrencyFlags::from_bits(*mode as u8)
                        .unwrap_or(ReserveCurrencyFlags::empty());

                    let message_part = format!(
                        "{} {} {}",
                        "reserve".blue(),
                        self.format_ton(reserve),
                        Self::format_reserve_currency_flags(mode_flags).dimmed()
                    );
                    let balance_part =
                        format!("balance: {}", self.format_ton(changed_remaining_balance));
                    let location_part = loc
                        .map(|l| format!("at {}", l.format()))
                        .unwrap_or_default();

                    action_parts.push((message_part, balance_part, location_part));
                }
            }
        }

        let mut max_message_width = 0;
        let mut max_balance_width = 0;

        for (message, balance, _) in &action_parts {
            max_message_width = max_message_width.max(visible_len(message));
            max_balance_width = max_balance_width.max(visible_len(balance));
        }

        let mut result = String::new();
        result.push_str("Executed actions:\n");

        for (idx, (message, balance, location)) in action_parts.iter().enumerate() {
            if idx == executed.actions.len() - 1 {
                result.push_str(format!("{}    {} ", child_prefix, "└──".dimmed()).as_str());
            } else {
                result.push_str(format!("{}    {} ", child_prefix, "├──".dimmed()).as_str());
            }

            let message_padding =
                " ".repeat(max_message_width.saturating_sub(visible_len(message)));
            let balance_padding =
                " ".repeat(max_balance_width.saturating_sub(visible_len(balance)));

            result.push_str(message);
            result.push_str(&message_padding);
            result.push_str("  ");
            result.push_str(balance);
            result.push_str(&balance_padding);

            if !location.is_empty() {
                result.push_str("  ");
                result.push_str(location.dimmed().to_string().as_str());
            }

            result.push('\n');
        }

        result.trim_end().to_string()
    }

    fn format_invalid_actions_retrace(
        &self,
        child_prefix: &str,
        tx: &Transaction,
        installed_actions: &InstalledActions,
        invalid_actions: &[InvalidAction],
    ) -> String {
        if invalid_actions.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        result.push_str("Invalid actions:\n");

        for (idx, action) in invalid_actions.iter().enumerate() {
            if idx == invalid_actions.len() - 1 {
                result.push_str(format!("{}    {} ", child_prefix, "└──".dimmed()).as_str());
            } else {
                result.push_str(format!("{}    {} ", child_prefix, "├──".dimmed()).as_str());
            }

            let reason = if action.during_preprocessing {
                "during action list preprocessing"
            } else {
                "in action list"
            };

            result.push_str(
                format!(
                    "invalid action {}: error code {} ({reason})",
                    action.action_index, action.error_code
                )
                .as_str(),
            );

            if let Some(loc) =
                self.find_invalid_action_source_loc(tx, installed_actions, action.action_index)
            {
                result.push_str("  ");
                result.push_str(
                    format!("at {}", loc.format_normalized())
                        .dimmed()
                        .to_string()
                        .as_str(),
                );
            }

            result.push('\n');
        }

        result.trim_end().to_string()
    }

    fn find_invalid_action_source_loc(
        &self,
        tx: &Transaction,
        installed_actions: &InstalledActions,
        action_index: usize,
    ) -> Option<SourceLocation> {
        match installed_actions.find_by_index(action_index)? {
            InstalledAction::Message(action) => {
                self.find_source_loc(tx, &action.loc_hash, action.loc_offset)
            }
            InstalledAction::Reserve(action) => {
                self.find_source_loc(tx, &action.loc_hash, action.loc_offset)
            }
        }
    }

    fn find_source_loc(
        &self,
        tx: &Transaction,
        loc_hash: &str,
        loc_offset: u16,
    ) -> Option<SourceLocation> {
        let in_msg = tx.load_in_msg().ok()??;
        if let MsgInfo::Int(info) = &in_msg.info {
            let addr = match &info.dst {
                IntAddr::Std(addr) => addr,
                IntAddr::Var(_) => return None,
            };

            let code = Self::account_code(&self.accounts, addr);
            let result = self.build_cache.result_for_code(&code);

            if let Some(result) = result {
                return retrace::find_source_loc(&result.1.source_map, loc_hash, loc_offset);
            }
        }

        None
    }

    /// Format address with contract type and letter
    fn format_address_with_letter(
        &self,
        addr: &IntAddr,
        contract_letters: &HashMap<IntAddr, String>,
        show_full_names: bool,
    ) -> String {
        if let Some(letter) = contract_letters.get(addr) {
            if show_full_names {
                let contract_type = self.get_contract_type(addr);
                let mut result = if let Some(contract_type) = contract_type {
                    format!("{}", contract_type.cyan())
                } else {
                    Self::format_addr_hash(addr).dimmed().to_string()
                };
                result += &format!(" {} ", letter.bold());
                result
            } else {
                String::new()
            }
        } else {
            // No letter assigned, show full address info
            let contract_type = self.get_contract_type(addr);
            if let Some(contract_type) = contract_type {
                format!("{}", contract_type.cyan())
            } else {
                Self::format_addr_hash(addr).dimmed().to_string()
            }
        }
    }

    #[allow(clippy::useless_let_if_seq)]
    fn extract_opcode(in_msg: &RelaxedMessage) -> u32 {
        let mut body = in_msg.body;
        let bounced = match &in_msg.info {
            RelaxedMsgInfo::Int(info) => info.bounced,
            RelaxedMsgInfo::ExtOut(_) => false,
        };
        let mut opcode = body.load_u32().unwrap_or(0);
        if bounced {
            // if bounced read another 32 bit to get the actual opcode
            opcode = body.load_u32().unwrap_or(0);
        }
        opcode
    }

    fn get_message_name(&self, opcode: u32) -> String {
        let message_abi = self.contract_abi.find_type_by_opcode(opcode);
        let name = if let Some(message_abi) = &message_abi {
            message_abi.name.as_str()
        } else if opcode == 0 {
            "empty"
        } else {
            &format!("0x{opcode:x}")
        };

        Self::color_message_name(name)
    }

    fn color_message_name(name: &str) -> String {
        name.purple().bold().to_string()
    }

    fn get_contract_type(&self, addr: &IntAddr) -> Option<String> {
        let addr = match addr {
            IntAddr::Std(addr) => addr,
            IntAddr::Var(_) => return None,
        };

        // contract can be registered as an address with a name
        if let Some(known_address) = self.known_addresses.addresses.get(addr) {
            return Some(known_address.name.clone());
        }

        let shard_account = self.accounts.get(addr)?;
        let account = shard_account.load_account().ok()??;

        let state = account.state;
        let AccountState::Active(info) = state else {
            return None;
        };

        let code = info.code?;
        let code_hash = code.repr_hash();

        // contract can be registered as a cell with a name
        if let Some(cell_name) = self.known_code_cells.get(code_hash) {
            return Some(cell_name.clone());
        }

        // when we compile contracts from Acton.toml we store results in build cache
        // so we can find compiled contract and its name
        let compilation_result = self.build_cache.result_for_code(&Some(code));
        if let Some((_, result)) = compilation_result {
            return Some(result.name);
        }

        None
    }

    #[must_use]
    pub fn format_tuple(&self, tuple: &Tuple, root: bool, colorize: bool) -> String {
        self.format_tuple_with_brackets(tuple, root, colorize, '[', ']')
    }

    #[must_use]
    pub fn format_tensor(&self, tuple: &Tuple, root: bool, colorize: bool) -> String {
        self.format_tuple_with_brackets(tuple, root, colorize, '(', ')')
    }

    fn format_tuple_with_brackets(
        &self,
        tuple: &Tuple,
        root: bool,
        colorize: bool,
        open: char,
        close: char,
    ) -> String {
        if tuple.len() == 1 {
            return self.format_internal(&tuple[0], root, colorize);
        }

        let mut res = String::new();
        write!(res, "{open}").ok();
        for (i, item) in tuple.iter().enumerate() {
            if i > 0 {
                write!(res, ", ").ok();
            }
            write!(res, "{}", self.format_internal(item, false, colorize)).ok();
        }
        write!(res, "{close}").ok();
        res
    }

    /// Format any `TupleItem` with rich formatting
    #[must_use]
    pub fn format(&self, item: &TupleItem) -> String {
        self.format_internal(item, true, false)
    }

    /// Format any `TupleItem` with rich formatting and colors
    #[must_use]
    pub fn format_with_color(&self, item: &TupleItem) -> String {
        self.format_internal(item, true, true)
    }

    fn format_internal(&self, item: &TupleItem, root: bool, colorize: bool) -> String {
        match item {
            TupleItem::TypedTuple {
                type_name,
                inner: items,
            } => {
                if items.is_empty() {
                    return format!("{type_name}()");
                }

                if type_name.ends_with('?') {
                    return self.format_nullable(item, root, colorize);
                }

                if type_name == "SendResultList" {
                    return self.format_transaction_list(items);
                }

                if type_name.starts_with("map<")
                    && let Some(formatted_map) = self.format_map(type_name, items, root, colorize)
                {
                    return formatted_map;
                }

                let abi = self.contract_abi.find_any_type(type_name);

                // Format structure as Foo { ... }
                if let Some(struct_desc) = abi {
                    return self.format_structure(
                        struct_desc,
                        0,
                        &mut VecDeque::from(items.0.clone()),
                        colorize,
                    );
                }

                if let TupleItem::Slice(cell) = &items[0]
                    && type_name == "address"
                {
                    return self.format_address_slice(cell, colorize);
                }
                if let TupleItem::Int(value) = &items[0]
                    && type_name == "bool"
                {
                    let s = if value == &BigInt::ZERO {
                        "false".to_owned()
                    } else if value == &BigInt::from(-1) {
                        "true".to_owned()
                    } else {
                        format!("{value}")
                    };
                    return if colorize { s.yellow().to_string() } else { s };
                }

                if type_name == "string"
                    && let TupleItem::Cell(cell) | TupleItem::Slice(cell) = &items[0]
                    && let Some(string) = Tuple::parse_snake_string(cell)
                {
                    if root {
                        // for `println("hello")` show `hello`
                        return string;
                    }

                    let s = format!("\"{string}\"");
                    return if colorize { s.green().to_string() } else { s };
                }

                if let TupleItem::Slice(_) = &items[0] {
                    return self.format_internal(&items[0], root, colorize);
                }

                if type_name.starts_with('(') {
                    // (int, slice, etc.) tensor
                    return self.format_tensor(items, root, colorize);
                }

                self.format_tuple(items, root, colorize)
            }
            TupleItem::Slice(cell) => {
                if cell.bit_len() == 0 && cell.reference_count() == 0 {
                    return "empty slice".to_owned();
                }

                self.format_slice(cell)
            }
            TupleItem::Int(value) => {
                let s = format!("{value}");
                if colorize { s.yellow().to_string() } else { s }
            }
            TupleItem::Null => {
                if colorize {
                    "null".bold().to_string()
                } else {
                    "null".to_owned()
                }
            }
            TupleItem::Cont(cont) => {
                let s = Boc::encode_hex(&cont.code);
                if colorize { s.dimmed().to_string() } else { s }
            }
            TupleItem::Nan => "NaN".to_owned(),
            TupleItem::Cell(cell) | TupleItem::Builder(cell) => {
                let s = Boc::encode_hex(cell);
                if colorize { s.dimmed().to_string() } else { s }
            }
            TupleItem::Tuple(items) => self.format_tuple(items, root, colorize),
        }
    }

    fn format_nullable(&self, item: &TupleItem, root: bool, colorize: bool) -> String {
        let TupleItem::TypedTuple { type_name, inner } = item else {
            return String::new();
        };

        // From Tolk compiler:
        // pass `null` to `T?` when T is wide (stores some nulls and UTag=0 at runtime)
        // - `null` to `(int, int)?`
        // - `null` to `int | slice | null`
        // to represent a non-primitive null value, we need N nulls + 1 null flag (UTag=0, type_id of TypeDataNullLiteral)
        //
        // So we can just check if the last element is zero to understand if whole tuple represents null.
        if inner.last() == Some(&TupleItem::Int(0.into())) {
            return if colorize {
                "null".bold().to_string()
            } else {
                "null".to_owned()
            };
        }

        let inner_type = &type_name[..type_name.len() - 1];

        // map<K, V> and (null, X) -> empty map
        if inner_type.starts_with("map<")
            && inner.len() == 2
            && inner.first() == Some(&TupleItem::Null)
            && matches!(inner.last(), Some(&TupleItem::Int(_)))
        {
            return format!("{inner_type} {{}}");
        }

        self.format_internal(
            &TupleItem::TypedTuple {
                type_name: inner_type.to_owned(),
                inner: inner.clone(),
            },
            root,
            colorize,
        )
    }

    fn format_map(
        &self,
        type_name: &str,
        items: &Tuple,
        is_root: bool,
        colorize: bool,
    ) -> Option<String> {
        let map_item = items.iter().find(|item| {
            matches!(
                item,
                TupleItem::Null | TupleItem::Cell(_) | TupleItem::Slice(_)
            )
        })?;

        let dict_root = match map_item {
            TupleItem::Null => None,
            TupleItem::Cell(cell) | TupleItem::Slice(cell) => Some(cell.clone()),
            _ => return Some(format!("{type_name} {{...}}")),
        };

        let Some((key_type_name, value_type_name)) = Self::parse_map_type(type_name) else {
            return Some(self.format_map_raw(type_name, &dict_root, colorize));
        };

        let Some(key_type) = Self::parse_map_key_type(&key_type_name) else {
            return Some(self.format_map_raw(type_name, &dict_root, colorize));
        };
        let value_type = Self::parse_map_value_type(&value_type_name);
        let allow_raw_value_fallback = value_type.is_none()
            && !value_type_name.ends_with('?')
            && !value_type_name.starts_with("map<");

        let mut entries = Vec::new();
        for entry in dict::RawIter::new(&dict_root, key_type.bit_len()) {
            let Ok((key_data, mut value_slice)) = entry else {
                return Some(format!("{type_name} {{...}}"));
            };

            let key = {
                let mut key_slice = key_data.as_data_slice();
                self.format_map_scalar(&mut key_slice, key_type, colorize)
                    .unwrap_or_else(|_| "<key>".to_owned())
            };

            let value = if let Some(value_type) = value_type {
                self.format_map_scalar(&mut value_slice, value_type, colorize)
                    .unwrap_or_else(|err| format!("<value: {err}>"))
            } else if allow_raw_value_fallback {
                self.format_map_raw_value(value_slice, colorize)
                    .unwrap_or_else(|err| format!("<value: {err}>"))
            } else {
                "<value>".to_owned()
            };

            entries.push((key, value));
        }

        if entries.is_empty() {
            return Some(format!("{type_name} {{}}"));
        }

        if !is_root {
            let mut formatted_entries = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                formatted_entries.push(format!("{key}: {value}"));
            }
            return Some(format!("{type_name} {{{}}}", formatted_entries.join(", ")));
        }

        let mut result = String::new();
        writeln!(result, "{type_name} {{").ok();
        for (key, value) in &entries {
            writeln!(result, "    {key}: {value},").ok();
        }
        result.push('}');

        Some(result)
    }

    fn parse_map_type(type_name: &str) -> Option<(String, String)> {
        let type_name = type_name.trim();
        let inner = type_name.strip_prefix("map<")?.strip_suffix('>')?;
        let split_idx = Self::find_top_level_comma(inner)?;
        let key_type = inner[..split_idx].trim().to_owned();
        let value_type = inner[split_idx + 1..].trim().to_owned();
        Some((key_type, value_type))
    }

    fn find_top_level_comma(source: &str) -> Option<usize> {
        let mut angle_depth = 0usize;
        let mut paren_depth = 0usize;
        let mut square_depth = 0usize;

        for (idx, ch) in source.char_indices() {
            match ch {
                '<' => angle_depth += 1,
                '>' => angle_depth = angle_depth.saturating_sub(1),
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                '[' => square_depth += 1,
                ']' => square_depth = square_depth.saturating_sub(1),
                ',' if angle_depth == 0 && paren_depth == 0 && square_depth == 0 => {
                    return Some(idx);
                }
                _ => {}
            }
        }

        None
    }

    fn parse_map_key_type(type_name: &str) -> Option<MapScalarType> {
        let type_name = type_name.trim();
        match type_name {
            "bool" => Some(MapScalarType::Bool),
            "address" | "any_address" => Some(MapScalarType::Address),
            "int" => Some(MapScalarType::Int {
                bits: 257,
                signed: true,
            }),
            "uint" => Some(MapScalarType::Int {
                bits: 256,
                signed: false,
            }),
            _ => {
                if let Some(bits) = type_name.strip_prefix("int") {
                    return bits
                        .parse::<u16>()
                        .ok()
                        .map(|bits| MapScalarType::Int { bits, signed: true });
                }
                if let Some(bits) = type_name.strip_prefix("uint") {
                    return bits.parse::<u16>().ok().map(|bits| MapScalarType::Int {
                        bits,
                        signed: false,
                    });
                }
                None
            }
        }
    }

    fn parse_map_value_type(type_name: &str) -> Option<MapScalarType> {
        let type_name = type_name.trim();
        if type_name.ends_with('?') || type_name.starts_with("map<") {
            return None;
        }

        if type_name == "cell" || type_name.starts_with("Cell<") {
            return Some(MapScalarType::Cell);
        }
        if type_name == "string" {
            return Some(MapScalarType::String);
        }

        match type_name {
            "bool" => Some(MapScalarType::Bool),
            "address" | "any_address" => Some(MapScalarType::Address),
            "coins" => Some(MapScalarType::VarInt {
                len_bits: 4,
                signed: false,
            }),
            "int" => Some(MapScalarType::Int {
                bits: 257,
                signed: true,
            }),
            "uint" => Some(MapScalarType::Int {
                bits: 256,
                signed: false,
            }),
            _ => {
                if let Some(bits) = type_name.strip_prefix("int") {
                    return bits
                        .parse::<u16>()
                        .ok()
                        .map(|bits| MapScalarType::Int { bits, signed: true });
                }
                if let Some(bits) = type_name.strip_prefix("uint") {
                    return bits.parse::<u16>().ok().map(|bits| MapScalarType::Int {
                        bits,
                        signed: false,
                    });
                }
                if let Some(bytes) = type_name.strip_prefix("varint") {
                    return match bytes {
                        "16" => Some(MapScalarType::VarInt {
                            len_bits: 4,
                            signed: true,
                        }),
                        "32" => Some(MapScalarType::VarInt {
                            len_bits: 5,
                            signed: true,
                        }),
                        _ => None,
                    };
                }
                if let Some(bytes) = type_name.strip_prefix("varuint") {
                    return match bytes {
                        "16" => Some(MapScalarType::VarInt {
                            len_bits: 4,
                            signed: false,
                        }),
                        "32" => Some(MapScalarType::VarInt {
                            len_bits: 5,
                            signed: false,
                        }),
                        _ => None,
                    };
                }
                None
            }
        }
    }

    fn format_map_scalar(
        &self,
        slice: &mut CellSlice<'_>,
        ty: MapScalarType,
        colorize: bool,
    ) -> Result<String, String> {
        match ty {
            MapScalarType::Int { bits, signed } => {
                if !signed && bits == 256 {
                    let value = format!("0x{}", slice.load_u256().map_err(|e| e.to_string())?);
                    return Ok(if colorize {
                        value.yellow().to_string()
                    } else {
                        value
                    });
                }

                let value = slice
                    .load_bigint(bits, signed)
                    .map_err(|e| e.to_string())?
                    .to_string();
                Ok(if colorize {
                    value.yellow().to_string()
                } else {
                    value
                })
            }
            MapScalarType::VarInt { len_bits, signed } => {
                let value = slice
                    .load_var_bigint(u16::from(len_bits), signed)
                    .map_err(|e| e.to_string())?
                    .to_string();
                Ok(if colorize {
                    value.yellow().to_string()
                } else {
                    value
                })
            }
            MapScalarType::Bool => {
                let value = slice.load_bit().map_err(|e| e.to_string())?.to_string();
                Ok(if colorize {
                    value.yellow().to_string()
                } else {
                    value
                })
            }
            MapScalarType::Address => {
                let value =
                    self.address_to_string(&IntAddr::load_from(slice).map_err(|e| e.to_string())?);
                Ok(if colorize {
                    value.cyan().to_string()
                } else {
                    value
                })
            }
            MapScalarType::Cell => {
                let value =
                    Boc::encode_hex(&slice.load_reference_cloned().map_err(|e| e.to_string())?);
                Ok(if colorize {
                    value.dimmed().to_string()
                } else {
                    value
                })
            }
            MapScalarType::String => {
                let cell = slice.load_reference_cloned().map_err(|e| e.to_string())?;
                if let Some(string) = Tuple::parse_snake_string(&cell) {
                    let value = format!("\"{string}\"");
                    return Ok(if colorize {
                        value.green().to_string()
                    } else {
                        value
                    });
                }

                let value = Boc::encode_hex(&cell);
                Ok(if colorize {
                    value.dimmed().to_string()
                } else {
                    value
                })
            }
        }
    }

    fn format_map_raw_value(&self, slice: CellSlice<'_>, colorize: bool) -> Result<String, String> {
        let mut builder = CellBuilder::new();
        builder.store_slice(slice).map_err(|e| e.to_string())?;
        let cell = builder.build().map_err(|e| e.to_string())?;
        let value = Boc::encode_hex(&cell);

        Ok(if colorize {
            value.dimmed().to_string()
        } else {
            value
        })
    }

    fn format_map_raw(&self, type_name: &str, root: &Option<Cell>, colorize: bool) -> String {
        let Some(cell) = root else {
            return format!("{type_name} {{}}");
        };

        let raw = Boc::encode_hex(cell);
        let raw = if colorize {
            raw.dimmed().to_string()
        } else {
            raw
        };

        format!("{type_name} {{raw: {raw}}}")
    }

    fn format_structure(
        &self,
        struct_desc: TypeAbi,
        level: usize,
        items: &mut VecDeque<TupleItem>,
        colorize: bool,
    ) -> String {
        let mut f = String::new();

        if colorize {
            writeln!(f, "{} {}", struct_desc.name.magenta(), "{".dimmed()).ok();
        } else {
            writeln!(f, "{} {{", struct_desc.name).ok();
        }

        for (i, field) in struct_desc.fields.iter().enumerate() {
            let field_type = field.type_info.human_readable.clone();
            let field_value = if let Some(abi) = self.contract_abi.find_any_type(&field_type) {
                let result = self.format_structure(abi, level, items, colorize);
                Self::add_indent_to_lines_except_first(result.as_str(), (level + 1) * 4)
            } else if let Some(field_value) = items.pop_front() {
                self.format_internal(&field_value.to_typed(&field_type), false, colorize)
            } else {
                "<unknown value>".to_string()
            };

            write!(f, "    {}: {}", field.name, field_value).ok();
            if i < struct_desc.fields.len() - 1 {
                write!(f, ",").ok();
            }
            writeln!(f).ok();

            if items.is_empty() {
                break;
            }
        }

        if colorize {
            write!(f, "{}", "}".dimmed()).ok();
        } else {
            write!(f, "}}").ok();
        }
        f
    }

    #[must_use]
    pub fn format_tuple_value(&self, tuple: &Tuple, type_name: &str, indent: usize) -> String {
        fn add_indent_to_lines(text: &str, indent: usize) -> String {
            let indent_str = " ".repeat(indent);
            text.lines()
                .map(|line| format!("{indent_str}{line}"))
                .collect::<Vec<_>>()
                .join("\n")
        }

        let item = tuple.to_typed(type_name);
        let formatted = self.format(&item);

        if !formatted.contains('\n') {
            // Fast path for values with single line
            return formatted;
        }

        let lines: Vec<_> = formatted.lines().collect();
        let mut result = lines[0].to_string() + "\n";
        result += &add_indent_to_lines(&lines[1..].join("\n"), indent);
        result
    }

    fn add_indent_to_lines_except_first(text: &str, indent: usize) -> String {
        if !text.contains('\n') {
            // Fast path for values with single line
            return text.to_string();
        }

        let indent_str = " ".repeat(indent);
        text.lines()
            .enumerate()
            .map(|(idx, line)| {
                if idx == 0 {
                    line.to_string()
                } else {
                    format!("{indent_str}{line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Show address in short format
    fn format_addr_hash(addr: &IntAddr) -> String {
        let Some(std_addr) = addr.as_std() else {
            return addr.to_string();
        };
        let raw = std_addr.display_base64(true).to_string();
        raw[..6].to_string() + ".." + &raw[raw.len() - 6..]
    }

    #[must_use]
    pub fn format_address(&self, txs: &TupleItem, addr: &Option<IntAddr>) -> String {
        let Some(addr) = addr else {
            return "<any>".cyan().to_string();
        };

        let TupleItem::TypedTuple { inner: items, .. } = txs else {
            return Self::format_addr_hash(addr);
        };

        let send_results = self.parse_send_results(items);
        let known_contracts = self.collect_known_contracts(&send_results);
        let contract_letters = self.create_contract_letters(&known_contracts);

        let mut builder = String::new();

        let contract_type = self.get_contract_type(addr);
        if let Some(contract_type) = contract_type {
            builder += format!("{} ", contract_type.cyan()).as_str();
        }

        let letter = contract_letters.get(addr);
        if let Some(letter) = letter {
            builder += format!("{} ", letter.bold()).as_str();
        }

        builder += Self::format_addr_hash(addr).dimmed().to_string().as_str();

        builder
    }
}

impl FormatterContext<'_> {
    #[must_use]
    pub fn format_tuple_diff(
        &self,
        left: &Tuple,
        right: &Tuple,
        left_type: &str,
        right_type: &str,
    ) -> String {
        let left_items = &left.0;
        let right_items = &right.0;

        if left_type != right_type {
            return format!(
                "{} != {}",
                self.format_tuple(left, false, false),
                self.format_tuple(right, false, false)
            );
        }

        let abi = self.contract_abi.find_any_type(&left_type.to_string());
        if let Some(struct_desc) = abi {
            let mut left_queue = VecDeque::from(left_items.clone());
            let mut right_queue = VecDeque::from(right_items.clone());
            self.format_structure_diff(struct_desc, 0, &mut left_queue, &mut right_queue)
        } else {
            let mut result = "(\n".to_string();
            let max_len = left_items.len().max(right_items.len());

            for i in 0..max_len {
                let left_val = left_items.get(i).map(|i| i.to_typed(left_type));
                let right_val = right_items.get(i).map(|i| i.to_typed(right_type));

                match (left_val, right_val) {
                    (Some(left_val), Some(right_val)) => {
                        if left_val == right_val {
                            result.push_str(&format!("    {},\n", self.format(&left_val).dimmed()));
                        } else {
                            result.push_str(&format!("    {},\n", self.format(&left_val).red()));
                            result.push_str(&format!("    {}\n", self.format(&right_val).green()));
                        }
                    }
                    (Some(left_val), None) => {
                        result.push_str(&format!("    {},\n", self.format(&left_val).red()));
                    }
                    (None, Some(right_val)) => {
                        result.push_str(&format!("    {}\n", self.format(&right_val).green()));
                    }
                    (None, None) => {}
                }
            }

            result.push(')');
            result
        }
    }

    fn format_structure_diff(
        &self,
        struct_desc: TypeAbi,
        level: usize,
        left_items: &mut VecDeque<TupleItem>,
        right_items: &mut VecDeque<TupleItem>,
    ) -> String {
        let mut f = String::new();

        writeln!(f, "{} {{", struct_desc.name).ok();

        for (i, field) in struct_desc.fields.iter().enumerate() {
            let field_type = field.type_info.human_readable.clone();

            if let Some(abi) = self.contract_abi.find_any_type(&field_type) {
                let result = self.format_structure_diff(abi, level, left_items, right_items);
                let has_diff = result.contains("\x1b[31m") || result.contains("\x1b[32m");

                let field_name = if has_diff {
                    field.name.clone()
                } else {
                    field.name.dimmed().to_string()
                };
                let colon = if has_diff {
                    ": ".to_string()
                } else {
                    ": ".dimmed().to_string()
                };

                let field_value =
                    Self::add_indent_to_lines_except_first(result.as_str(), (level + 1) * 4);
                write!(f, "    {field_name}{colon}{field_value}").ok();
            } else {
                let left_val = left_items.pop_front();
                let right_val = right_items.pop_front();

                match (left_val, right_val) {
                    (Some(l), Some(r)) => {
                        let l_typed = l.to_typed(&field_type);
                        let r_typed = r.to_typed(&field_type);

                        if l_typed == r_typed {
                            write!(
                                f,
                                "    {}{}{}",
                                field.name.dimmed(),
                                ": ".dimmed(),
                                self.format(&l_typed).dimmed()
                            )
                            .ok();
                        } else {
                            writeln!(f, "    {}: {}", field.name, self.format(&l_typed).red()).ok();
                            write!(
                                f,
                                "    {:<width$}  {}",
                                "",
                                self.format(&r_typed).green(),
                                width = field.name.len()
                            )
                            .ok();
                        }
                    }
                    (Some(l), None) => {
                        write!(
                            f,
                            "    {}: {}",
                            field.name,
                            self.format(&l.to_typed(&field_type)).red()
                        )
                        .ok();
                    }
                    (None, Some(r)) => {
                        writeln!(f, "    {}:", field.name.yellow()).ok();
                        write!(
                            f,
                            "    {:<width$}  {}",
                            "",
                            self.format(&r.to_typed(&field_type)).green(),
                            width = field.name.len()
                        )
                        .ok();
                    }
                    (None, None) => {}
                }
            }

            if i < struct_desc.fields.len() - 1 {
                write!(f, "{}", ",".dimmed()).ok();
            }
            writeln!(f).ok();

            if left_items.is_empty() && right_items.is_empty() {
                break;
            }
        }

        write!(f, "}}").ok();
        f
    }

    #[must_use]
    pub fn format_send_msg_flags(flags: SendMsgFlags) -> String {
        let mut flag_names = Vec::new();

        if flags.contains(SendMsgFlags::PAY_FEE_SEPARATELY) {
            flag_names.push("PAY_FEES_SEPARATELY");
        }
        if flags.contains(SendMsgFlags::IGNORE_ERROR) {
            flag_names.push("IGNORE_ERRORS");
        }
        if flags.contains(SendMsgFlags::BOUNCE_ON_ERROR) {
            flag_names.push("BOUNCE_ON_ACTION_FAIL");
        }
        if flags.contains(SendMsgFlags::DELETE_IF_EMPTY) {
            flag_names.push("DESTROY");
        }
        if flags.contains(SendMsgFlags::WITH_REMAINING_BALANCE) {
            flag_names.push("CARRY_ALL_REMAINING_MESSAGE_VALUE");
        }
        if flags.contains(SendMsgFlags::ALL_BALANCE) {
            flag_names.push("CARRY_ALL_BALANCE");
        }

        if flag_names.is_empty() {
            "REGULAR".to_string()
        } else {
            flag_names.join(" | ")
        }
    }

    #[must_use]
    pub fn format_reserve_currency_flags(flags: ReserveCurrencyFlags) -> String {
        let mut flag_names = Vec::new();

        if flags.contains(ReserveCurrencyFlags::ALL_BUT) {
            flag_names.push("ALL_BUT_AMOUNT");
        }
        if flags.contains(ReserveCurrencyFlags::IGNORE_ERROR) {
            flag_names.push("AT_MOST");
        }
        if flags.contains(ReserveCurrencyFlags::WITH_ORIGINAL_BALANCE) {
            flag_names.push("INCREASE_BY_ORIGINAL_BALANCE");
        }
        if flags.contains(ReserveCurrencyFlags::REVERSE) {
            flag_names.push("NEGATE_AMOUNT");
        }
        if flags.contains(ReserveCurrencyFlags::BOUNCE_ON_ERROR) {
            flag_names.push("BOUNCE_ON_ACTION_FAIL");
        }

        if flag_names.is_empty() {
            "EXACT_AMOUNT".to_string()
        } else {
            flag_names.join(" | ")
        }
    }

    #[must_use]
    pub fn format_search_transaction_parameters(
        &self,
        assert_failure: &TransactionGenericAssertFailure,
        abi: Arc<ContractAbi>,
    ) -> Vec<String> {
        let mut params = vec![];
        use crate::context::DisplayParam;

        let fmt_bool = |v: bool| {
            if v {
                "true".green().to_string()
            } else {
                "false".red().to_string()
            }
        };
        let fmt_int = |v: &dyn std::fmt::Display, zero_ok: bool| {
            let s = v.to_string();
            if zero_ok && s == "0" {
                "0".green().to_string()
            } else {
                s.red().to_string()
            }
        };

        macro_rules! push_param {
            (bool $name:literal, $field:expr) => {
                match &$field {
                    Some(DisplayParam::Value(v)) => {
                        params.push(format!("  {}={}", $name, fmt_bool(*v)))
                    }
                    Some(DisplayParam::Function) => {
                        params.push(format!("  {}={}", $name, "<function>".cyan()))
                    }
                    None => {}
                }
            };
            (int $name:literal, $field:expr) => {
                match &$field {
                    Some(DisplayParam::Value(v)) => {
                        params.push(format!("  {}={}", $name, fmt_int(v, true)))
                    }
                    Some(DisplayParam::Function) => {
                        params.push(format!("  {}={}", $name, "<function>".cyan()))
                    }
                    None => {}
                }
            };
        }

        if let Some(ref dp) = assert_failure.params.opcode {
            match dp {
                DisplayParam::Value(opcode) => {
                    let opcode_type = abi.find_type_by_opcode(*opcode);
                    params.push(format!(
                        "  opcode={} {}",
                        format!("0x{opcode:x}").green(),
                        opcode_type
                            .map(|typ| typ.name)
                            .unwrap_or_else(|| if *opcode == 0 {
                                "empty".to_owned()
                            } else {
                                "unknown".to_owned()
                            })
                            .purple()
                            .bold()
                    ));
                }
                DisplayParam::Function => params.push(format!("  opcode={}", "<function>".cyan())),
            }
        }
        push_param!(bool "bounced", assert_failure.params.bounced);
        push_param!(bool "bounce", assert_failure.params.bounce);
        match &assert_failure.params.value {
            Some(DisplayParam::Value(v)) => params.push(format!("  value={v}")),
            Some(DisplayParam::Function) => params.push(format!("  value={}", "<function>".cyan())),
            None => {}
        }
        push_param!(bool "deploy", assert_failure.params.deploy);
        push_param!(bool "success", assert_failure.params.success);
        push_param!(bool "aborted", assert_failure.params.aborted);
        push_param!(int "exit_code", assert_failure.params.exit_code);
        push_param!(int "action_exit_code", assert_failure.params.action_exit_code);
        push_param!(bool "compute_phase_skipped", assert_failure.params.compute_phase_skipped);
        match &assert_failure.params.body {
            Some(DisplayParam::Value(body)) => {
                params.push(format!("  body={}", Boc::encode_hex(body)));
            }
            Some(DisplayParam::Function) => params.push(format!("  body={}", "<function>".cyan())),
            None => {}
        }
        params
    }

    #[must_use]
    pub fn highlight_actual_expected(message: &str) -> String {
        message
            .replace("<actual>", &"actual".red().to_string())
            .replace("<expected>", &"expected".green().to_string())
    }

    #[must_use]
    pub fn format_exit_code(code: i32) -> String {
        if let Some(info) = exit_codes::find(code) {
            return info.name.to_owned();
        }

        code.to_string()
    }

    #[must_use]
    pub fn format_exit_code_with_number(code: i32) -> String {
        if let Some(info) = exit_codes::find(code) {
            return format!("{code} ({}): {}", info.name, info.description);
        }

        code.to_string()
    }

    #[must_use]
    pub fn format_get_method_assert_failure_title(failure: &GetMethodAssertFailure) -> String {
        if failure.vm_exit_code == 11 {
            if let Some(suggested_name) = &failure.suggested_name {
                return format!(
                    "Cannot execute unknown get method {}, did you mean '{suggested_name}'",
                    failure.get_method_presentation
                );
            }
            return format!(
                "Cannot execute unknown get method {}",
                failure.get_method_presentation
            );
        }

        if failure.vm_exit_code == 2 {
            return format!(
                "Get method {} failed due to stack underflow. Make sure you passed all parameters to the get method.",
                failure.get_method_presentation
            );
        }

        format!(
            "Cannot execute get method {}",
            failure.get_method_presentation
        )
    }

    #[must_use]
    pub fn format_get_method_assert_failure(&self, failure: &GetMethodAssertFailure) -> String {
        let mut output = Self::format_get_method_assert_failure_title(failure);

        if failure.vm_exit_code == 11 || failure.vm_exit_code == 2 {
            return output;
        }

        let mut details = String::new();
        writeln!(
            details,
            "exit_code={}",
            failure.vm_exit_code.to_string().yellow()
        )
        .ok();

        let replayed_exception = retrace::find_exception_info(&failure.vm_log, &failure.source_map);

        if let Some(info) = &replayed_exception {
            writeln!(details, "at {}", Self::format_location(&info.loc)).ok();

            if !info.backtrace.is_empty() {
                writeln!(details, "Backtrace:").ok();
                for line in Self::format_backtrace(&info.backtrace) {
                    writeln!(details, "  {line}").ok();
                }
            }
        } else if self.backtrace.is_none() {
            writeln!(
                details,
                "Re-run with {} to get more information",
                "--backtrace full".yellow()
            )
            .ok();
        }

        if let Some(info) = &failure.caller_trace {
            writeln!(details, "Called from:").ok();
            let backtrace_lines = Self::format_backtrace(&info.backtrace);
            if backtrace_lines.is_empty() {
                writeln!(details, "  at {}", Self::format_location(&info.loc)).ok();
            } else {
                for line in backtrace_lines {
                    writeln!(details, "  {line}").ok();
                }
            }
        }

        if let Some(info) = exit_codes::find(failure.vm_exit_code) {
            writeln!(details, "Description: {}", info.description).ok();
            writeln!(details, "Phase: {}", info.phase).ok();
        } else if let Some(info) = Self::find_custom_exit_code_info(
            failure.vm_exit_code,
            failure
                .abi
                .as_deref()
                .or_else(|| Some(self.contract_abi.as_ref())),
            failure.compiler_abi.as_deref(),
        ) {
            writeln!(details, "Description: {}", info.description).ok();
            if info.symbolic_name != info.description {
                writeln!(details, "Error: {}", info.symbolic_name).ok();
            }
            writeln!(details, "Phase: Compute phase").ok();
        } else if let Some(info) = &replayed_exception {
            let description = if info.description.is_empty() {
                format!("uncaught exception {}", info.errno)
            } else {
                info.description.clone()
            };
            writeln!(details, "Description: {description}").ok();
        }

        let details = details.trim();
        if details.is_empty() {
            return output;
        }

        output.push('\n');
        output.push_str(details);
        output
    }

    #[must_use]
    pub fn account_code(
        accounts: &FxHashMap<StdAddr, ShardAccount>,
        addr: &StdAddr,
    ) -> Option<Cell> {
        let account = accounts.get(addr);
        let state = account?.account.load().ok()?.0?.state;
        match state {
            AccountState::Active(state) => state.code,
            AccountState::Uninit | AccountState::Frozen(_) => None,
        }
    }

    #[must_use]
    pub fn get_failed_transaction_context(
        &self,
        failure: &TransactionGenericAssertFailure,
        abi: Arc<ContractAbi>,
    ) -> FailedTransactionContext {
        let from_address = failure.params.from.as_ref().map(|dp| match dp {
            DisplayParam::Value(addr) => self.address_to_string(addr),
            DisplayParam::Function => "<function>".to_string(),
        });
        let to_address = failure.params.to.as_ref().map(|dp| match dp {
            DisplayParam::Value(addr) => self.address_to_string(addr),
            DisplayParam::Function => "<function>".to_string(),
        });
        let params = self
            .format_search_transaction_parameters(failure, abi)
            .into_iter()
            .map(|p| {
                let p = strip_ansi_codes(&p);
                let p = p.trim();
                if let Some((k, v)) = p.split_once('=') {
                    (k.trim().to_string(), v.trim().to_string())
                } else {
                    (p.to_string(), String::new())
                }
            })
            .collect();

        FailedTransactionContext {
            from_address,
            to_address,
            params,
        }
    }

    #[must_use]
    pub fn parse_failed_transactions(&self, txs: &TupleItem) -> Vec<TransactionInfo> {
        let TupleItem::TypedTuple { inner: items, .. } = txs else {
            return vec![];
        };

        let send_results = self.parse_send_results(items);
        send_results
            .into_iter()
            .map(|res| {
                let tx = res.tx;
                let code = Self::account_code(&self.accounts, &StdAddr::new(0, tx.account));
                let build = self.build_cache.result_for_code(&code);

                TransactionInfo {
                    lt: tx.lt.to_string(),
                    raw_transaction: Boc::encode_base64(to_cell(&tx)).into(),
                    parent_transaction: res.parent_lt.map(|lt| lt.to_string()),
                    dest_contract_info: build.map(|(_, info)| info.name),
                    child_transactions: res.children_ids.iter().map(ToString::to_string).collect(),
                    shard_account_before: String::new(),
                    shard_account: String::new(),
                    vm_log_diff: self
                        .emulations
                        .find_tx_logs(tx.lt)
                        .map(vmlogs::convert_to_diff_logs)
                        .unwrap_or_default(),
                    executor_logs: self
                        .emulations
                        .find_tx_executor_logs(tx.lt)
                        .map(Arc::from)
                        .unwrap_or_default(),
                    executor_actions: self
                        .emulations
                        .find_tx_executor_logs(tx.lt)
                        .map(crate::commands::test::trace::parse_executor_actions)
                        .unwrap_or_default(),
                    actions: Some(Boc::encode_base64(&res.actions).into()),
                }
            })
            .collect()
    }

    #[must_use]
    pub fn format_detailed_assert_failure(
        &self,
        failure: &AssertFailure,
        abi: Arc<ContractAbi>,
    ) -> String {
        let mut result = String::new();
        let append_location = !matches!(failure, AssertFailure::GetMethod(_));

        if let Some(message) = &failure.message()
            && !message.is_empty()
        {
            let highlighted_message = Self::highlight_actual_expected(message);
            writeln!(result, "{highlighted_message}").ok();
        }

        match failure {
            AssertFailure::Bin(bin_failure) if bin_failure.operator == "==" => {
                let diff = self.format_tuple_diff(
                    &bin_failure.left,
                    &bin_failure.right,
                    &bin_failure.left_type,
                    &bin_failure.right_type,
                );
                writeln!(result, "{diff}").ok();
            }
            AssertFailure::Bin(bin_failure) if bin_failure.operator == "!=" => {
                let value = self.format_tuple_value(&bin_failure.left, &bin_failure.left_type, 0);
                writeln!(result, "Values are equal but expected to be different:").ok();
                writeln!(result, "  {value}").ok();
            }
            AssertFailure::Bin(bin_failure) if bin_failure.is_ord() => {
                let left = self.format_tuple_value(&bin_failure.left, &bin_failure.left_type, 0);
                let right = self.format_tuple_value(&bin_failure.right, &bin_failure.right_type, 0);
                writeln!(result, "        Actual:   {left}").ok();
                writeln!(result, "        Expected: {right}").ok();
            }
            AssertFailure::Decimal(decimal_failure) => {
                writeln!(result, "        Actual:   {}", decimal_failure.left).ok();
                writeln!(result, "        Expected: {}", decimal_failure.right).ok();
            }
            AssertFailure::TransactionNotFound(tx_failure) => {
                let params = self.format_search_transaction_parameters(tx_failure, abi);
                let tx_tree = self.format(&tx_failure.txs);
                writeln!(result, "{tx_tree}").ok();
                let from_addr = tx_failure.params.from.as_ref().and_then(|dp| match dp {
                    DisplayParam::Value(a) => Some(a.clone()),
                    DisplayParam::Function => None,
                });
                let to_addr = tx_failure.params.to.as_ref().and_then(|dp| match dp {
                    DisplayParam::Value(a) => Some(a.clone()),
                    DisplayParam::Function => None,
                });
                let from_str = if tx_failure
                    .params
                    .from
                    .as_ref()
                    .is_some_and(|dp| matches!(dp, DisplayParam::Function))
                {
                    "<function>".cyan().to_string()
                } else {
                    self.format_address(&tx_failure.txs, &from_addr)
                };
                let to_str = if tx_failure
                    .params
                    .to
                    .as_ref()
                    .is_some_and(|dp| matches!(dp, DisplayParam::Function))
                {
                    "<function>".cyan().to_string()
                } else {
                    self.format_address(&tx_failure.txs, &to_addr)
                };
                writeln!(
                    result,
                    "Cannot find transaction from {from_str} to {to_str}"
                )
                .ok();
                writeln!(result, "with:").ok();
                for param in params {
                    writeln!(result, "  {param}").ok();
                }
            }
            AssertFailure::TransactionIsFound(tx_failure) => {
                let params = self.format_search_transaction_parameters(tx_failure, abi);
                let tx_tree = self.format(&tx_failure.txs);
                writeln!(result, "{tx_tree}").ok();
                let from_to = if tx_failure.params.from.is_none() && tx_failure.params.to.is_none()
                {
                    String::new()
                } else {
                    let from_addr = tx_failure.params.from.as_ref().and_then(|dp| match dp {
                        DisplayParam::Value(a) => Some(a.clone()),
                        DisplayParam::Function => None,
                    });
                    let to_addr = tx_failure.params.to.as_ref().and_then(|dp| match dp {
                        DisplayParam::Value(a) => Some(a.clone()),
                        DisplayParam::Function => None,
                    });
                    let from_s = if tx_failure
                        .params
                        .from
                        .as_ref()
                        .is_some_and(|dp| matches!(dp, DisplayParam::Function))
                    {
                        "<function>".cyan().to_string()
                    } else {
                        self.format_address(&tx_failure.txs, &from_addr)
                    };
                    let to_s = if tx_failure
                        .params
                        .to
                        .as_ref()
                        .is_some_and(|dp| matches!(dp, DisplayParam::Function))
                    {
                        "<function>".cyan().to_string()
                    } else {
                        self.format_address(&tx_failure.txs, &to_addr)
                    };
                    format!(" from {from_s} to {to_s}")
                };
                writeln!(result, "Unexpected transaction{from_to}").ok();
                if !params.is_empty() {
                    writeln!(result, "with:").ok();
                    for param in params {
                        writeln!(result, "  {param}").ok();
                    }
                }
            }
            AssertFailure::WalletNotFound(failure) => {
                let message = self.format_wallet_not_found_message(failure);
                let highlighted_message = Self::highlight_actual_expected(&message);
                writeln!(result, "Error: {highlighted_message}").ok();
            }
            AssertFailure::GetMethod(failure) => {
                let message = self.format_get_method_assert_failure(failure);
                writeln!(result, "{message}").ok();
            }
            _ => {}
        }

        if append_location && let Some(location) = &failure.location() {
            writeln!(result, "at {}", location.format()).ok();
        }

        result.trim().to_string()
    }

    #[must_use]
    pub fn strip_ansi_text(text: &str) -> String {
        strip_ansi_codes(text)
    }

    #[must_use]
    pub fn format_detailed_exit_code(
        &self,
        test: &TestReport,
        result: &ton_executor::get::GetMethodResultSuccess,
        exit_code: i32,
    ) -> String {
        let mut output = String::new();
        writeln!(output, "exit_code={exit_code}").ok();

        let exit_code_info = retrace::find_exception_info(&result.vm_log, &test.source_map);
        let get_method_info = self.find_failed_get_method_exception(test);

        if let Some(info) = &get_method_info {
            writeln!(output, "Get method:").ok();
            writeln!(output, "  at {}", Self::format_location(&info.loc)).ok();

            let backtrace_lines = Self::format_backtrace(&info.backtrace);
            if !backtrace_lines.is_empty() {
                writeln!(output, "  Backtrace:").ok();
                for line in backtrace_lines {
                    writeln!(output, "    {}", strip_ansi_codes(&line)).ok();
                }
            }
        }

        if let Some(info) = &exit_code_info {
            if get_method_info.is_some() {
                writeln!(output, "Called from:").ok();
            } else {
                writeln!(output, "at {}", Self::format_location(&info.loc)).ok();
            }

            let backtrace_lines = Self::format_backtrace(&info.backtrace);
            if !backtrace_lines.is_empty() {
                if get_method_info.is_none() {
                    writeln!(output, "Backtrace:").ok();
                }
                for line in backtrace_lines {
                    writeln!(output, "  {}", strip_ansi_codes(&line)).ok();
                }
            } else if get_method_info.is_some() {
                writeln!(output, "  at {}", Self::format_location(&info.loc)).ok();
            }
        } else if test.backtrace.is_none() {
            writeln!(
                output,
                "Re-run with {} to get more information",
                "--backtrace full".yellow()
            )
            .ok();
        }

        if let Some(info) = exit_codes::find(exit_code) {
            writeln!(output, "Description: {}", info.description).ok();
            writeln!(output, "Phase: {}", info.phase).ok();
        } else if let Some(info) = self
            .find_code_custom_exit_code_info(result.code.as_ref(), exit_code)
            .or_else(|| {
                Self::find_custom_exit_code_info(
                    exit_code,
                    Some(test.abi.as_ref()),
                    test.compiler_abi.as_deref(),
                )
            })
        {
            writeln!(output, "Description: {}", info.description).ok();
            if info.symbolic_name != info.description {
                writeln!(output, "Error: {}", info.symbolic_name).ok();
            }
            writeln!(output, "Phase: Compute phase").ok();
        } else if let Some(info) = &exit_code_info {
            let description = if info.description.is_empty() {
                format!("uncaught exception {}", info.errno)
            } else {
                info.description.clone()
            };
            writeln!(output, "Description: {description}").ok();
        }

        output.trim().to_string()
    }
}

fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape && ch == 'm' {
            in_escape = false;
        } else if !in_escape {
            result.push(ch);
        }
    }
    result
}

/// Calculate visible length of a string (excluding ANSI escape codes)
fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape && ch == 'm' {
            in_escape = false;
        } else if !in_escape {
            len += 1;
        }
    }
    len
}
