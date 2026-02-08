use crate::commands::test::reporting::{FailedTransactionContext, TestReport};
use crate::commands::test::trace::TransactionInfo;
use crate::context::{
    AssertFailure, BuildCache, EmulationsState, KnownAddresses, TransactionGenericAssertFailure,
    to_cell,
};
use crate::retrace::{ExecutedAction, InstalledActions};
use crate::{exit_codes, retrace};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use owo_colors::OwoColorize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;
use ton_abi::{ContractAbi, TypeAbi};
use ton_api::Network;
use ton_source_map::{DebugLocation, SourceLocation};
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, ExecutedComputePhase, IntAddr, MsgInfo,
    RelaxedMessage, RelaxedMsgInfo, ReserveCurrencyFlags, SendMsgFlags, ShardAccount, Transaction,
    TxInfo,
};
use tycho_types::num::Tokens;

#[derive(Debug, Clone)]
struct SendResult {
    tx: Transaction,
    children_ids: Vec<i64>,
    parent_lt: Option<i64>,
    #[allow(dead_code)]
    actions: ArcCell,
    #[allow(dead_code)]
    out_messages: Vec<ArcCell>,
    externals: Vec<Cell>,
}

#[derive(Debug, Clone)]
struct TransactionNode {
    send_result: SendResult,
    children: Vec<TransactionNode>,
}

/// Context for formatting `TupleItems` with rich information
#[derive(Debug, Clone)]
pub struct FormatterContext {
    pub contract_abi: ContractAbi,
    pub accounts: HashMap<String, ShardAccount>,
    pub build_cache: BuildCache,
    pub emulations: EmulationsState,
    pub known_addresses: KnownAddresses,
    pub known_code_cells: HashMap<String, String>,
    pub backtrace: Option<String>,
    pub fork_net: Option<Network>,
    pub api_key: Option<String>,
    pub network: Option<Network>,
}

impl FormatterContext {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            contract_abi: ContractAbi::default(),
            accounts: HashMap::new(),
            build_cache: BuildCache::new(),
            emulations: EmulationsState::new(),
            known_addresses: KnownAddresses::new(),
            known_code_cells: HashMap::new(),
            backtrace: None,
            fork_net: None,
            network: None,
            api_key: None,
        }
    }

    /// Create formatter context from the main Context
    #[must_use]
    pub fn from_context(ctx: &crate::context::Context) -> Self {
        Self {
            contract_abi: ctx.env.abi.clone(),
            accounts: ctx.chain.world_state.get_accounts().clone(),
            build_cache: ctx.build.build_cache.clone(),
            emulations: ctx.chain.emulations.clone(),
            known_addresses: ctx.build.known_addresses.clone(),
            known_code_cells: ctx.build.known_code_cells.clone(),
            backtrace: ctx.build.backtrace.clone(),
            fork_net: ctx.env.fork_net.clone(),
            network: ctx.network.clone(),
            api_key: ctx.env.api_key.clone(),
        }
    }

    fn format_slice(&self, slice: &ArcCell) -> String {
        let mut parser = slice.parser();

        if parser.remaining_bits() == 2 && parser.load_u8(2).unwrap_or(0) == 0 {
            return "addr_none".to_string();
        }

        if parser.remaining_bits() == 267
            && let Ok(address) = parser.load_address()
        {
            return self.address_to_string(&address);
        }

        slice
            .to_boc_hex(false)
            .unwrap_or_else(|_| "<invalid slice>".to_owned())
    }

    fn address_to_string(&self, address: &TonAddress) -> String {
        let need_mainnet_address =
            self.fork_net == Some(Network::Mainnet) || self.network == Some(Network::Mainnet);
        address.to_base64_std_flags(false, !need_mainnet_address)
    }

    fn format_address_slice(&self, slice: &ArcCell, colorize: bool) -> String {
        let mut parser = slice.parser();
        if let Ok(address) = parser.load_address() {
            let addr = Self::arc_cell_to_addr(slice);
            let address_base64 = self.address_to_string(&address);

            let addr_str = if colorize {
                address_base64.cyan().to_string()
            } else {
                address_base64
            };

            if let Some(addr) = &addr {
                let contract_type = self.get_contract_type(addr);
                if let Some(contract_type) = contract_type {
                    return format!("{addr_str} ({contract_type})");
                }
            }

            return addr_str;
        }

        slice
            .to_boc_hex(false)
            .unwrap_or_else(|_| "invalid address".to_owned())
    }

    fn arc_cell_to_addr(slice: &ArcCell) -> Option<IntAddr> {
        let cell = Boc::decode(slice.to_boc(false).ok()?).ok()?;
        let mut slice = cell.as_slice().ok()?;
        let addr = IntAddr::load_from(&mut slice);
        addr.ok()
    }

    /// Format transaction list as a tree
    #[must_use]
    pub fn format_transaction_list(&self, items: &[TupleItem]) -> String {
        let send_results = self.parse_send_results(items);
        let known_contracts = self.collect_known_contracts(&send_results);
        let contract_letters = self.create_contract_letters(&known_contracts);

        let tree = self.build_transaction_tree(send_results);
        self.format_transaction_tree(&tree, &contract_letters, 0, "")
    }

    /// Parse transaction items into `SendResult` structures
    fn parse_send_results(&self, tx_items: &[TupleItem]) -> Vec<SendResult> {
        tx_items
            .iter()
            .filter_map(|el| match el {
                TupleItem::Tuple(tuple) => match (
                    tuple[0].clone(),
                    tuple[1].clone(),
                    tuple[3].clone(),
                    tuple[4].clone(),
                    tuple[6].clone(), // externals
                ) {
                    (
                        TupleItem::Cell(tx),
                        TupleItem::Tuple(child_ids),
                        TupleItem::Cell(actions),
                        TupleItem::Tuple(out_messages),
                        TupleItem::Tuple(externals),
                    ) => {
                        let result = tx.to_boc(false).ok()?;
                        let tx_cell = Boc::decode(&result).ok()?;
                        let tx = tx_cell.parse::<Transaction>().ok()?;
                        Some(SendResult {
                            tx,
                            children_ids: child_ids
                                .iter()
                                .filter_map(|id| match id {
                                    TupleItem::Int(int) => int.to_i64(),
                                    _ => None,
                                })
                                .collect(),
                            parent_lt: match tuple[2].clone() {
                                TupleItem::Null => None,
                                TupleItem::Int(int) => int.to_i64(),
                                _ => None,
                            },
                            actions,
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
                                    TupleItem::Cell(cell) => {
                                        let boc = cell.to_boc(false).ok()?;
                                        let cell = Boc::decode(&boc).ok()?;
                                        Some(cell)
                                    }
                                    _ => None,
                                })
                                .collect(),
                        })
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>()
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
                let cycle = index / 26 + 1;
                format!("{letter}{cycle}")
            };

            contract_letters.insert(addr.clone(), letter_str);
        }

        contract_letters
    }

    /// Build transaction tree from `SendResult` list
    fn build_transaction_tree(&self, send_results: Vec<SendResult>) -> Vec<TransactionNode> {
        let mut lt_to_result: HashMap<i64, SendResult> = HashMap::new();

        for result in send_results.into_iter() {
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
                let src_addr = match &in_msg.info {
                    MsgInfo::Int(info) => info.src.clone(),
                    _ => panic!("Expected internal message"),
                };
                let src_formatted =
                    self.format_address_with_letter(&src_addr, contract_letters, show_full_names);
                tx_builder += &format!(
                    "{} {} {}\n",
                    "N/A".dimmed(),
                    "->".dimmed(),
                    src_formatted.trim()
                );
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
        let Some(in_msg) = &tx.in_msg else {
            return String::new();
        };
        let Ok(in_msg) = in_msg.parse::<RelaxedMessage>() else {
            return String::new();
        };
        self.format_single_message(&in_msg, contract_letters, show_full_names)
    }

    fn format_single_message(
        &self,
        in_msg: &RelaxedMessage,
        contract_letters: &HashMap<IntAddr, String>,
        show_full_names: bool,
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

        let opcode = Self::extract_opcode(in_msg);
        let message_name = self.get_message_name(opcode);
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
                extra_infos.push(format!(
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
                ));
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
            extra_infos.push("account created".to_string());
        }
        if tx.orig_status == AccountStatus::Active && tx.end_status == AccountStatus::NotExists {
            extra_infos.push("account destroyed".to_string());
        }

        match info.action_phase {
            None => {}
            Some(action) => {
                if action.result_code != 0 {
                    result += &format!(" action_result_code={}", action.result_code)
                        .red()
                        .to_string();

                    extra_infos.push("Action phase failed".to_string());

                    if let Some(info) = exit_codes::find(action.result_code) {
                        extra_infos.push(format!(
                            "Description: {}",
                            info.description.to_string().yellow()
                        ));
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
                            extra_infos.push(format!(
                                "Re-run with {} to get actions location",
                                "--backtrace full".yellow()
                            ));
                        }

                        let actions = self.format_actions_retrace(
                            child_prefix,
                            tx,
                            installed_actions,
                            logs,
                            contract_letters,
                        );
                        extra_infos.push(actions);
                    }
                }
            }
        }

        for ext_msg in &send_result.externals {
            let Ok(msg) = ext_msg.parse::<RelaxedMessage>() else {
                continue;
            };

            let Some(msg_info) = self.format_ext_out_message(&msg) else {
                continue;
            };

            extra_infos.push(msg_info);
        }

        if !extra_infos.is_empty() {
            result += "\n";
        }

        for (idx, info) in extra_infos.iter().enumerate() {
            result += child_prefix;

            if has_children || idx < extra_infos.len() - 1 {
                result += "├── ".dimmed().to_string().as_str();
            } else {
                result += "└── ".dimmed().to_string().as_str();
            }

            result += info.as_str();

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
        extra_infos: &mut Vec<String>,
        compute: &ExecutedComputePhase,
    ) -> String {
        let mut result = String::new();
        result += &format!(" exit_code={}", compute.exit_code)
            .red()
            .to_string();

        if let Some(info) = exit_codes::find(compute.exit_code) {
            extra_infos.push(format!(
                "Compute phase failed: {}",
                info.description.to_string().yellow()
            ));
        }

        self.format_transaction_backtrace(tx, child_prefix, extra_infos);

        result
    }

    fn format_transaction_backtrace(
        &self,
        tx: &Transaction,
        child_prefix: &str,
        extra_infos: &mut Vec<String>,
    ) -> Option<()> {
        // Trying to retrace exit code to find out exact Tolk source location
        let logs = self.emulations.find_tx_logs(tx.lt)?;
        let in_msg = tx.load_in_msg().ok()??;

        let dst = match in_msg.info {
            MsgInfo::Int(info) => info.dst,
            MsgInfo::ExtIn(info) => info.dst,
            MsgInfo::ExtOut(_) => return None,
        };

        let code = Self::account_code(&self.accounts, dst.to_string());
        let result = self.build_cache.result_for_code(&code)?;

        let info = retrace::find_exception_info(logs, &result.1.source_map)?;
        let loc = info.loc?;

        let backtrace_result = Self::format_backtrace(&info.backtrace)
            .iter()
            .map(|line| format!("{child_prefix}       {line}"))
            .collect::<Vec<String>>()
            .join("\n");

        extra_infos.push(format!(
            "at {}\n{}",
            loc.format().dimmed(),
            backtrace_result
        ));

        Some(())
    }

    #[must_use]
    pub fn format_backtrace(backtrace: &[DebugLocation]) -> Vec<String> {
        let max_function_name_len = backtrace
            .iter()
            .filter_map(|loc| loc.context.event_function.as_ref())
            .map(|name| name.len() + 2)
            .max()
            .unwrap_or(0);

        let backtrace_lines = backtrace.iter().rev().filter_map(|loc| {
            let func_name = loc.context.event_function.as_ref()?;

            let location = format!(
                "{}:{}:{}",
                SourceLocation::normalize_path(&loc.loc.file),
                loc.loc.line + 1,
                loc.loc.column + 2
            );
            Some(format!(
                "{:<width$} at {}",
                func_name.green(),
                location.dimmed(),
                width = max_function_name_len
            ))
        });

        backtrace_lines.collect()
    }

    fn format_ext_out_message(&self, msg: &RelaxedMessage) -> Option<String> {
        let RelaxedMsgInfo::ExtOut(info) = &msg.info else {
            return None;
        };

        let opcode = Self::extract_opcode(msg);
        let message_name = self.get_message_name(opcode);

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
        let actions = retrace::ExecutedActions::from(logs).actions;

        if actions.is_empty() {
            return String::new();
        }

        let mut action_parts = Vec::new();

        for action in &actions {
            match action {
                ExecutedAction::SendMessage {
                    hash,
                    remaining_balance,
                } => {
                    let message = installed_actions.find_message(hash);

                    let (loc, formated) = if let Some(message) = message {
                        let msg = message.message();

                        let formated = match msg {
                            Some(msg) => self.format_single_message(&msg, contract_letters, false),
                            None => hash.to_string(),
                        };

                        (
                            self.find_source_loc(tx, &message.loc_hash, message.loc_offset),
                            formated,
                        )
                    } else {
                        (None, "msg: ".to_owned() + hash)
                    };

                    let message_part = formated;
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
            if idx == actions.len() - 1 {
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

    fn find_source_loc(
        &self,
        tx: &Transaction,
        loc_hash: &str,
        loc_offset: u16,
    ) -> Option<SourceLocation> {
        let in_msg = tx.load_in_msg().ok()??;
        if let MsgInfo::Int(info) = &in_msg.info {
            let code = Self::account_code(&self.accounts, info.dst.to_string());
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

        name.purple().bold().to_string()
    }

    fn get_contract_type(&self, addr: &IntAddr) -> Option<String> {
        let known_address = self
            .known_addresses
            .addresses
            .iter()
            .find(|(address, _)| address.to_string() == addr.to_string());

        if let Some((_, known_address)) = known_address {
            return Some(known_address.name.clone());
        }

        let addr_str = addr.to_string();

        if let Some(account) = self.accounts.get(&addr_str) {
            let state = account.account.load().ok()?.0?.state;
            let code_hash = match state {
                AccountState::Uninit => None,
                AccountState::Active(state) => state.code.map(|code| code.repr_hash().to_string()),
                AccountState::Frozen(_) => None,
            };

            let known_code_cell = self
                .known_code_cells
                .iter()
                .find(|(hash, _info)| code_hash == Some((*hash).clone()));

            if let Some((_, known_code_cell)) = known_code_cell {
                return Some(known_code_cell.clone());
            }
        }

        let shard_account = self.accounts.get(&addr_str)?;
        let account = shard_account.load_account().ok()??;

        let AccountState::Active(info) = account.state else {
            return None;
        };

        let code = info.code?;
        let compilation_result = self.build_cache.built.iter().find(|(_, result)| {
            result.code_hash.to_ascii_lowercase() == code.repr_hash().to_string()
        });

        if let Some((_, result)) = compilation_result {
            return Some(result.name.clone());
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
                if cell.bit_len() == 0 && cell.references().is_empty() {
                    return "empty slice".to_owned();
                }

                if let Some(string) = Tuple::parse_snake_string(cell) {
                    if root {
                        // for `println("hello")` show `hello`
                        return string;
                    }

                    let s = format!("\"{string}\"");
                    return if colorize { s.green().to_string() } else { s };
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
            TupleItem::Nan => "NaN".to_owned(),
            TupleItem::Cell(cell) => {
                let s = cell
                    .to_boc_hex(false)
                    .unwrap_or_else(|_| "<invalid cell>".to_owned());
                if colorize { s.dimmed().to_string() } else { s }
            }
            TupleItem::Builder(cell) => {
                let s = cell
                    .to_boc_hex(false)
                    .unwrap_or_else(|_| "<invalid builder>".to_owned());
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
            return format!("{inner_type}{{}}");
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
    pub fn format_tuple_value(&self, tuple: &Tuple, type_name: &String, indent: usize) -> String {
        fn add_indent_to_lines(text: &str, indent: usize) -> String {
            let indent_str = " ".repeat(indent);
            text.lines()
                .map(|line| format!("{indent_str}{line}"))
                .collect::<Vec<_>>()
                .join("\n")
        }

        let item = tuple.to_typed(&type_name.to_string());
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

impl FormatterContext {
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
                    field.name.to_string()
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
        abi: &ContractAbi,
    ) -> Vec<String> {
        let mut params = vec![];
        if let Some(opcode) = assert_failure.params.opcode {
            let opcode_type = abi.find_type_by_opcode(opcode);
            params.push(format!(
                "  opcode={} {}",
                format!("0x{opcode:x}").green(),
                opcode_type
                    .map(|typ| typ.name)
                    .unwrap_or_else(|| if opcode == 0 {
                        "empty".to_owned()
                    } else {
                        "unknown".to_owned()
                    })
                    .purple()
                    .bold()
            ));
        }
        if let Some(bounced) = assert_failure.params.bounced {
            params.push(format!(
                "  bounced={}",
                if bounced {
                    "true".green().to_string()
                } else {
                    "false".red().to_string()
                }
            ));
        }
        if let Some(bounce) = assert_failure.params.bounce {
            params.push(format!(
                "  bounce={}",
                if bounce {
                    "true".green().to_string()
                } else {
                    "false".red().to_string()
                }
            ));
        }
        if let Some(deploy) = assert_failure.params.deploy {
            params.push(format!(
                "  deploy={}",
                if deploy {
                    "true".green().to_string()
                } else {
                    "false".red().to_string()
                }
            ));
        }
        if let Some(success) = assert_failure.params.success {
            params.push(format!(
                "  success={}",
                if success {
                    "true".green().to_string()
                } else {
                    "false".red().to_string()
                }
            ));
        }
        if let Some(aborted) = assert_failure.params.aborted {
            params.push(format!(
                "  aborted={}",
                if aborted {
                    "true".green().to_string()
                } else {
                    "false".red().to_string()
                }
            ));
        }
        if let Some(exit_code) = assert_failure.params.exit_code {
            params.push(format!(
                "  exit_code={}",
                if exit_code == 0 {
                    "0".green().to_string()
                } else {
                    exit_code.to_string().red().to_string()
                }
            ));
        }
        if let Some(action_exit_code) = assert_failure.params.action_exit_code {
            params.push(format!(
                "  action_exit_code={}",
                if action_exit_code == 0 {
                    "0".green().to_string()
                } else {
                    action_exit_code.to_string().red().to_string()
                }
            ));
        }
        if let Some(compute_phase_skipped) = assert_failure.params.compute_phase_skipped {
            params.push(format!(
                "  compute_phase_skipped={}",
                if compute_phase_skipped {
                    "true".green().to_string()
                } else {
                    "false".red().to_string()
                }
            ));
        }
        if let Some(body) = &assert_failure.params.body {
            params.push(format!("  body={}", Boc::encode_hex(body)));
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
    pub fn account_code(accounts: &HashMap<String, ShardAccount>, addr: String) -> Option<Cell> {
        let account = accounts.get(&addr);
        let state = account?.account.load().ok()?.0?.state;
        match state {
            AccountState::Uninit => None,
            AccountState::Active(state) => state.code,
            AccountState::Frozen(_) => None,
        }
    }

    #[must_use]
    pub fn get_failed_transaction_context(
        &self,
        failure: &TransactionGenericAssertFailure,
        abi: &ContractAbi,
    ) -> FailedTransactionContext {
        let from_address = failure.params.from.as_ref().map(|addr| match addr {
            IntAddr::Std(addr) => addr.display_base64(false).to_string(),
            _ => addr.to_string(),
        });
        let to_address = failure.params.to.as_ref().map(|addr| match addr {
            IntAddr::Std(addr) => addr.display_base64(false).to_string(),
            _ => addr.to_string(),
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
            .flat_map(|res| {
                let tx = res.tx;
                let code = Self::account_code(&self.accounts, tx.account.to_string());
                let build = self.build_cache.result_for_code(&code);

                Some(TransactionInfo {
                    lt: tx.lt.to_string(),
                    raw_transaction: Boc::encode_base64(to_cell(&tx)),
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
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                    actions: Some(res.actions.to_boc_b64(false).ok()?),
                })
            })
            .collect()
    }

    #[must_use]
    pub fn format_detailed_assert_failure(
        &self,
        failure: &AssertFailure,
        abi: &ContractAbi,
    ) -> String {
        let mut result = String::new();

        if let Some(message) = &failure.message()
            && !message.is_empty()
        {
            let highlighted_message = Self::highlight_actual_expected(message);
            let clean_message = strip_ansi_codes(&highlighted_message);
            writeln!(result, "Error: {clean_message}").ok();
        }

        match failure {
            AssertFailure::Bin(bin_failure) if bin_failure.operator == "==" => {
                let diff = self.format_tuple_diff(
                    &bin_failure.left,
                    &bin_failure.right,
                    &bin_failure.left_type,
                    &bin_failure.right_type,
                );
                writeln!(result, "{}", strip_ansi_codes(&diff)).ok();
            }
            AssertFailure::Bin(bin_failure) if bin_failure.operator == "!=" => {
                let value = self.format_tuple_value(&bin_failure.left, &bin_failure.left_type, 0);
                writeln!(result, "Values are equal but expected to be different:").ok();
                writeln!(result, "  {}", strip_ansi_codes(&value)).ok();
            }
            AssertFailure::Bin(bin_failure) if bin_failure.is_ord() => {
                let left = self.format_tuple_value(&bin_failure.left, &bin_failure.left_type, 0);
                let right = self.format_tuple_value(&bin_failure.right, &bin_failure.right_type, 0);
                writeln!(result, "Actual:   {}", strip_ansi_codes(&left)).ok();
                writeln!(result, "Expected: {}", strip_ansi_codes(&right)).ok();
            }
            AssertFailure::TransactionNotFound(tx_failure) => {
                let params = self.format_search_transaction_parameters(tx_failure, abi);
                let tx_tree = self.format(&tx_failure.txs);
                writeln!(result, "{}", strip_ansi_codes(&tx_tree)).ok();
                writeln!(
                    result,
                    "Cannot find transaction from {} to {}",
                    self.format_address(&tx_failure.txs, &tx_failure.params.from),
                    self.format_address(&tx_failure.txs, &tx_failure.params.to)
                )
                .ok();
                writeln!(result, "with:").ok();
                for param in params {
                    writeln!(result, "  {}", strip_ansi_codes(&param)).ok();
                }
            }
            AssertFailure::TransactionIsFound(tx_failure) => {
                let params = self.format_search_transaction_parameters(tx_failure, abi);
                let tx_tree = self.format(&tx_failure.txs);
                writeln!(result, "{}", strip_ansi_codes(&tx_tree)).ok();
                let from_to = if tx_failure.params.from.is_none() && tx_failure.params.to.is_none()
                {
                    "".to_string()
                } else {
                    format!(
                        " from {} to {}",
                        self.format_address(&tx_failure.txs, &tx_failure.params.from),
                        self.format_address(&tx_failure.txs, &tx_failure.params.to)
                    )
                };
                writeln!(result, "Unexpected transaction{from_to}").ok();
                if !params.is_empty() {
                    writeln!(result, "with:").ok();
                    for param in params {
                        writeln!(result, "  {}", strip_ansi_codes(&param)).ok();
                    }
                }
            }
            _ => {}
        }

        if let Some(location) = &failure.location()
            && !location.is_empty()
        {
            writeln!(result, "at {location}").ok();
        }

        result.trim().to_string()
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

        if let Some(info) = &exit_code_info {
            if let Some(loc) = &info.loc {
                writeln!(
                    output,
                    "at {}:{}:{}",
                    SourceLocation::normalize_path(&loc.file),
                    loc.line + 1,
                    loc.column + 2
                )
                .ok();

                let backtrace_lines = Self::format_backtrace(&info.backtrace);
                if !backtrace_lines.is_empty() {
                    writeln!(output, "Backtrace:").ok();
                    for line in backtrace_lines {
                        writeln!(output, "  {}", strip_ansi_codes(&line)).ok();
                    }
                }
            }

            if !info.description.is_empty() {
                writeln!(output, "Description: {}", info.description).ok();
            }
        }

        if let Some(info) = exit_codes::find(exit_code) {
            if exit_code_info.is_none() {
                writeln!(output, "Description: {}", info.description).ok();
            }
            writeln!(output, "Phase: {}", info.phase).ok();
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
