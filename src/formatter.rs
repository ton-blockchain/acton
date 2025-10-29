use crate::context::{BuildCache, KnownAddresses};
use abi::{ContractAbi, TypeAbi};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use owo_colors::OwoColorize;
use std::collections::{HashMap, VecDeque};
use std::fmt::Write;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;

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
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, IntAddr, Message, MsgInfo, ShardAccount,
    Transaction, TxInfo,
};

#[derive(Debug, Clone)]
struct SendResult {
    tx: Transaction,
    children_ids: Vec<i64>,
    parent_lt: Option<i64>,
    actions: ArcCell,
    out_messages: Vec<ArcCell>,
}

#[derive(Debug, Clone)]
struct TransactionNode {
    send_result: SendResult,
    children: Vec<TransactionNode>,
}

/// Context for formatting TupleItems with rich information
#[derive(Debug, Clone)]
pub struct FormatterContext {
    pub contract_abi: ContractAbi,
    pub accounts: HashMap<String, ShardAccount>,
    pub build_cache: BuildCache,
    pub known_addresses: KnownAddresses,
    pub known_code_cells: HashMap<String, String>,
}

impl FormatterContext {
    pub fn empty() -> Self {
        Self {
            contract_abi: ContractAbi::default(),
            accounts: HashMap::new(),
            build_cache: BuildCache::new(),
            known_addresses: KnownAddresses::new(),
            known_code_cells: HashMap::new(),
        }
    }

    /// Create formatter context from the main Context
    pub fn from_context(ctx: &crate::context::Context) -> Self {
        Self {
            contract_abi: ctx.abi.clone(),
            accounts: ctx.blockchain.get_accounts().clone(),
            build_cache: ctx.build_cache.clone(),
            known_addresses: ctx.known_addresses.clone(),
            known_code_cells: ctx.known_code_cells.clone(),
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
            return address.to_string();
        }

        slice.to_boc_hex(false).unwrap()
    }

    fn format_address_slice(&self, slice: &ArcCell) -> String {
        let mut parser = slice.parser();
        if let Ok(address) = parser.load_address() {
            return address.to_string();
        }
        slice.to_boc_hex(false).unwrap()
    }

    /// Format transaction list as a tree
    pub fn format_transaction_list(&self, items: &[TupleItem]) -> String {
        let send_results = self.parse_send_results(items);
        let known_contracts = self.collect_known_contracts(&send_results);
        let contract_letters = self.create_contract_letters(&known_contracts);

        let tree = self.build_transaction_tree(send_results);
        self.format_transaction_tree(&tree, &contract_letters, 0, "")
    }

    /// Parse transaction items into SendResult structures
    fn parse_send_results(&self, tx_items: &[TupleItem]) -> Vec<SendResult> {
        tx_items
            .iter()
            .filter_map(|el| match el {
                TupleItem::Tuple(tuple) => match (
                    tuple[0].clone(),
                    tuple[1].clone(),
                    tuple[3].clone(),
                    tuple[4].clone(),
                ) {
                    (
                        TupleItem::Cell(tx),
                        TupleItem::Tuple(child_ids),
                        TupleItem::Cell(actions),
                        TupleItem::Tuple(out_messages),
                    ) => {
                        let result = tx.to_boc_b64(false).unwrap();
                        let tx_cell: Cell = Boc::decode_base64(&result).unwrap();
                        let mut tx_slice = tx_cell.as_slice().unwrap();
                        let tx = Transaction::load_from(&mut tx_slice).unwrap();
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
            let in_msg = send_result.tx.load_in_msg().unwrap();
            if let Some(in_msg) = &in_msg
                && let MsgInfo::Int(info) = &in_msg.info
            {
                // It's O(N) but we need order, and we don't have many (thousands) transactions
                if !known_contracts.contains(&info.src) {
                    known_contracts.push(info.src.clone());
                }
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
            let letter = char::from_u32('A' as u32 + index as u32)
                .unwrap_or_else(|| char::from_digit(index as u32, 10).unwrap());
            contract_letters.insert(addr.clone(), letter.to_string());
        }

        contract_letters
    }

    /// Build transaction tree from SendResult list
    fn build_transaction_tree(&self, mut send_results: Vec<SendResult>) -> Vec<TransactionNode> {
        let mut lt_to_result: HashMap<i64, SendResult> = HashMap::new();

        for result in send_results.drain(..) {
            lt_to_result.insert(result.tx.lt as i64, result);
        }

        let mut roots = Vec::new();
        let mut processed = std::collections::HashSet::new();

        for (lt, result) in &lt_to_result {
            if result.parent_lt.is_none() || !lt_to_result.contains_key(&result.parent_lt.unwrap())
            {
                if !processed.contains(lt) {
                    let node = self.build_node_recursive(*lt, &lt_to_result, &mut processed);
                    if let Some(node) = node {
                        roots.push(node);
                    }
                }
            }
        }

        roots
    }

    /// Recursively build transaction tree node
    fn build_node_recursive(
        &self,
        lt: i64,
        lt_to_result: &HashMap<i64, SendResult>,
        processed: &mut std::collections::HashSet<i64>,
    ) -> Option<TransactionNode> {
        if processed.contains(&lt) {
            return None;
        }

        let result = lt_to_result.get(&lt)?;
        processed.insert(lt);

        let mut children = Vec::new();
        for child_lt in &result.children_ids {
            if let Some(child_node) = self.build_node_recursive(*child_lt, lt_to_result, processed)
            {
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

            // Recursively format children
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
        let mut tx_builder = "".to_string();

        let main_part = self.format_message_part(tx, contract_letters, false);
        let main_part_visible_len = visible_len(&main_part);

        if is_root {
            let src_addr = match &tx.load_in_msg().unwrap().unwrap().info {
                MsgInfo::Int(info) => info.src.clone(),
                _ => panic!("Expected internal message"),
            };
            let src_formatted =
                self.format_address_with_letter(&src_addr, contract_letters, show_full_names);
            tx_builder += &format!("{} {} {}\n", "N/A".dimmed(), "->".dimmed(), src_formatted);
            tx_builder += "└── ".dimmed().to_string().as_str();
        }

        tx_builder += &main_part;
        tx_builder += &self.format_transaction_info(
            tx,
            child_prefix,
            has_children,
            main_part_visible_len,
            prefix_len,
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
        let in_msg = tx.load_in_msg().unwrap().unwrap();
        let MsgInfo::Int(info) = &in_msg.info else {
            return "".to_string();
        };

        let mut result = "".to_string();

        if info.bounced {
            result += "(!) ".red().to_string().as_str();
        }

        result += &self.format_address_with_letter(&info.src, contract_letters, show_full_names);
        if show_full_names {
            result += " -> ".dimmed().to_string().as_str();
        }

        let opcode = self.extract_opcode(&in_msg);
        let message_name = self.get_message_name(opcode);
        result += &message_name;
        result += " ";

        let amount = info.value.tokens.into_inner() as f64 / 1e9;
        result += &format!("{} TON", amount.to_string()).green().to_string();
        result += " -> ".dimmed().to_string().as_str();

        result += &self.format_address_with_letter(&info.dst, contract_letters, true);

        result
    }

    /// Format transaction execution info (gas, exit code, account changes)
    fn format_transaction_info(
        &self,
        tx: &Transaction,
        child_prefix: &str,
        has_children: bool,
        main_part_visible_len: usize,
        prefix_len: usize,
    ) -> String {
        let TxInfo::Ordinary(info) = tx.load_info().unwrap() else {
            panic!("tick-tock message is unexpected")
        };

        match info.compute_phase {
            ComputePhase::Executed(compute) => {
                let mut result = String::new();
                // Add padding to align metadata
                let padding_len = 80usize.saturating_sub(prefix_len + main_part_visible_len);
                result += &" ".repeat(padding_len);
                result += &format!("gas={}", compute.gas_used.to_string().as_str())
                    .dimmed()
                    .to_string();

                if compute.exit_code != 0 {
                    result += &format!(" exit_code={}", compute.exit_code)
                        .red()
                        .to_string();
                }

                if tx.orig_status == AccountStatus::NotExists
                    && tx.end_status == AccountStatus::Active
                {
                    result += "\n";
                    result += child_prefix;
                    if has_children {
                        result += "├──".dimmed().to_string().as_str();
                    } else {
                        result += "└──".dimmed().to_string().as_str();
                    }
                    result += " account created".dimmed().to_string().as_str();
                }
                if tx.orig_status == AccountStatus::Active
                    && tx.end_status == AccountStatus::NotExists
                {
                    result += "\n";
                    result += child_prefix;
                    if has_children {
                        result += "├──".dimmed().to_string().as_str();
                    } else {
                        result += "└──".dimmed().to_string().as_str();
                    }
                    result += " account destroyed".dimmed().to_string().as_str();
                }

                result
            }
            _ => {
                let padding_len = 80usize.saturating_sub(prefix_len + main_part_visible_len);
                format!(
                    "{}{}",
                    " ".repeat(padding_len),
                    "compute phase skipped".dimmed()
                )
            }
        }
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
                let mut result = if contract_type != "" {
                    format!("{}", contract_type.cyan())
                } else {
                    Self::format_addr_hash(addr).dimmed().to_string()
                };
                result += &format!(" {} ", letter.bold());
                result
            } else {
                "".to_string()
            }
        } else {
            // No letter assigned, show full address info
            let contract_type = self.get_contract_type(addr);
            if contract_type != "" {
                format!("{}", contract_type.cyan())
            } else {
                Self::format_addr_hash(addr).dimmed().to_string()
            }
        }
    }

    /// Extract opcode from message body
    fn extract_opcode(&self, in_msg: &Message) -> u32 {
        let mut body = in_msg.body.clone();
        let mut opcode = body.load_u32().unwrap_or(0);
        if opcode == 0xFFFFFFFF {
            // if bounce read another 32 bit to get actual opcode
            opcode = body.load_u32().unwrap_or(0);
        }
        opcode
    }

    /// Get message name from opcode
    fn get_message_name(&self, opcode: u32) -> String {
        let message_abi = self
            .contract_abi
            .messages
            .iter()
            .find(|msg| msg.opcode != Some(0) && msg.opcode == Some(opcode));

        if let Some(message_abi) = message_abi {
            message_abi.name.as_str().purple().bold().to_string()
        } else if opcode == 0 {
            "empty".purple().bold().to_string()
        } else {
            format!("0x{:x}", opcode).purple().bold().to_string()
        }
    }

    /// Get contract type for address
    fn get_contract_type(&self, addr: &IntAddr) -> String {
        let known_address = self
            .known_addresses
            .addresses
            .iter()
            .find(|(address, _info)| address.to_string() == addr.to_string());

        if let Some(known_address) = known_address {
            return known_address.1.name.clone();
        }

        if let Some(account) = self.accounts.get(&addr.to_string()) {
            let state = account.account.load().unwrap().0.unwrap().state;
            let code_hash = match state {
                AccountState::Uninit => None,
                AccountState::Active(state) => state
                    .code
                    .and_then(|code| Some(code.repr_hash().to_string())),
                AccountState::Frozen(_) => None,
            };

            let known_code_cell = self
                .known_code_cells
                .iter()
                .find(|(hash, _info)| code_hash == Some((*hash).clone()));

            if let Some(known_code_cell) = known_code_cell {
                return known_code_cell.1.clone();
            }
        }

        if let Some(known_address) = known_address {
            return known_address.1.name.clone();
        }

        let addr_str = addr.to_string();
        let account = self.accounts.get(&addr_str);
        let Some(account) = account else {
            return "".to_string();
        };

        let account_data = account.load_account();
        let Ok(Some(data)) = account_data else {
            return "".to_string();
        };

        let AccountState::Active(info) = data.state else {
            return "".to_string();
        };

        let Some(code) = &info.code else {
            return "".to_string();
        };

        let compilation_result = self.build_cache.built.iter().find(|(_name, result)| {
            result.code_hash.to_ascii_lowercase() == code.repr_hash().to_string()
        });

        if let Some(result) = compilation_result {
            return result.1.name.clone();
        }

        "".to_string()
    }

    pub fn format_tuple(&self, tuple: &Tuple) -> String {
        if tuple.0.len() == 1 {
            return self.format(&tuple.0[0]);
        }

        let mut res = "".to_string();
        write!(res, "(").ok();
        for (i, item) in tuple.0.iter().enumerate() {
            if i > 0 {
                write!(res, ", ").ok();
            }
            write!(res, "{}", self.format(item)).ok();
        }
        write!(res, ")").ok();
        res
    }

    /// Format any TupleItem with rich formatting
    pub fn format(&self, item: &TupleItem) -> String {
        let formatted = match item {
            TupleItem::TypedTuple {
                type_name,
                inner: items,
            } => {
                if items.is_empty() {
                    return type_name.clone();
                }

                if type_name == "SendResultList" {
                    return self.format_transaction_list(items);
                }

                let abi = self.contract_abi.find_type(type_name);

                // Format structure as Foo { ... }
                if let Some(struct_desc) = abi {
                    return self.format_structure(
                        struct_desc,
                        0,
                        &mut VecDeque::from(items.0.clone()),
                    );
                }

                if let TupleItem::Slice(cell) = &items[0]
                    && type_name == "address"
                {
                    return self.format_address_slice(cell);
                }
                if let TupleItem::Int(value) = &items[0]
                    && type_name == "bool"
                {
                    return if value == &num_bigint::BigInt::from(0) {
                        "false".to_string()
                    } else if value == &num_bigint::BigInt::from(18446744073709551615u64) {
                        "true".to_string()
                    } else {
                        format!("{}", value)
                    };
                }

                if let TupleItem::Slice(_) = &items[0] {
                    return self.format(&items[0]);
                }

                format!("{}", self.format_tuple(items))
            }
            TupleItem::Slice(cell) => {
                if cell.bit_len() == 0 && cell.references().len() == 0 {
                    return "empty slice".to_string();
                }

                if let Some(string) = Tuple::parse_snake_string(cell) {
                    return format!("\"{}\"", string);
                }

                self.format_slice(cell)
            }
            TupleItem::Int(value) => {
                if *value == BigInt::from(18446744073709551615u64) {
                    return "-1".to_string();
                }
                return format!("{}", value);
            }
            TupleItem::Null => return "null".to_string(),
            TupleItem::Nan => return "NaN".to_string(),
            TupleItem::Cell(cell) => cell.to_boc_hex(false).unwrap(),
            TupleItem::Builder(cell) => cell.to_boc_hex(false).unwrap(),
            TupleItem::Tuple(items) => {
                if items.len() == 1 {
                    return self.format(&items[0]);
                }
                let mut res = "".to_string();
                write!(res, "(").ok();
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(res, ", ").ok();
                    }
                    write!(res, "{}", self.format(item)).ok();
                }
                write!(res, ")").ok();
                return res;
            }
        };

        formatted
    }

    fn format_structure(
        &self,
        struct_desc: TypeAbi,
        level: usize,
        items: &mut VecDeque<TupleItem>,
    ) -> String {
        let mut f = "".to_string();

        write!(f, "{} {{\n", struct_desc.name).ok();

        for (i, field) in struct_desc.fields.iter().enumerate() {
            let field_type = field.type_info.human_readable.clone();
            let field_value = if let Some(abi) = self.contract_abi.find_type(&field_type) {
                let result = self.format_structure(abi, level, items);
                Self::add_indent_to_lines_except_first(result.as_str(), (level + 1) * 4)
            } else {
                let field_value = items.pop_front().unwrap();
                self.format(&field_value.to_typed(&field_type))
            };

            write!(f, "    {}: {}", field.name, field_value).ok();
            if i < struct_desc.fields.len() - 1 {
                write!(f, ",").ok();
            }
            write!(f, "\n").ok();

            if items.is_empty() {
                break;
            }
        }
        write!(f, "}}").ok();
        f
    }

    pub fn format_tuple_value(&self, tuple: &Tuple, type_name: &String, indent: usize) -> String {
        fn add_indent_to_lines(text: &str, indent: usize) -> String {
            let indent_str = " ".repeat(indent);
            text.lines()
                .map(|line| format!("{}{}", indent_str, line))
                .collect::<Vec<_>>()
                .join("\n")
        }

        let item = tuple.to_typed(&type_name.to_string());
        let formatted = self.format(&item);

        if !formatted.contains("\n") {
            // Fast path for values with single line
            return formatted;
        }

        let lines: Vec<_> = formatted.lines().collect();
        let mut result = lines[0].to_string() + "\n";
        result += &add_indent_to_lines(&lines[1..].join("\n"), indent);
        result
    }

    fn add_indent_to_lines_except_first(text: &str, indent: usize) -> String {
        if !text.contains("\n") {
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
                    format!("{}{}", indent_str, line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Show address in short format
    fn format_addr_hash(addr: &IntAddr) -> String {
        let raw = addr.as_std().unwrap().display_base64(true).to_string();
        raw[..6].to_string() + ".." + &raw[raw.len() - 6..]
    }

    pub fn format_address(&self, txs: &TupleItem, addr: &Option<IntAddr>) -> String {
        let Some(addr) = addr else {
            return "<any>".cyan().to_string();
        };

        let TupleItem::TypedTuple { inner: items, .. } = txs else {
            return Self::format_addr_hash(&addr);
        };

        let send_results = self.parse_send_results(items);
        let known_contracts = self.collect_known_contracts(&send_results);
        let contract_letters = self.create_contract_letters(&known_contracts);

        let mut builder = "".to_string();

        let contract_type = self.get_contract_type(addr);

        let letter = contract_letters.get(&addr);
        if let Some(letter) = letter {
            builder += format!("{} {} ", contract_type.cyan(), letter.bold()).as_str();
        }

        builder += Self::format_addr_hash(&addr).dimmed().to_string().as_str();

        builder
    }
}

impl FormatterContext {
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
                self.format_tuple(left),
                self.format_tuple(right)
            );
        }

        let abi = self.contract_abi.find_type(&left_type.to_string());
        if let Some(struct_desc) = abi {
            if left_items.len() == struct_desc.fields.len() {
                let mut result = format!("{} {{\n", left_type);

                for (field, (left_item, right_item)) in struct_desc
                    .fields
                    .iter()
                    .zip(left_items.iter().zip(right_items.iter()))
                {
                    if left_item != right_item {
                        result.push_str(&format!(
                            "    {}: {}\n",
                            field.name.yellow(),
                            self.format(left_item).red()
                        ));
                        result.push_str(&format!(
                            "    {:<width$}  {}\n",
                            "",
                            self.format(right_item).green(),
                            width = field.name.len()
                        ));
                    } else {
                        result.push_str(&format!(
                            "    {}{} {}\n",
                            field.name.dimmed(),
                            ":".dimmed(),
                            self.format(left_item).dimmed()
                        ));
                    }
                }

                result.push_str("}");
                result
            } else {
                format!(
                    "{} != {}",
                    self.format_tuple(left),
                    self.format_tuple(right)
                )
            }
        } else {
            let mut result = "(\n".to_string();
            let max_len = left_items.len().max(right_items.len());

            for i in 0..max_len {
                let left_val = left_items.get(i);
                let right_val = right_items.get(i);

                match (left_val, right_val) {
                    (Some(left_val), Some(right_val)) => {
                        if left_val != right_val {
                            result.push_str(&format!("    {},\n", self.format(left_val).red()));
                            result.push_str(&format!("    {}\n", self.format(right_val).green()));
                        } else {
                            result.push_str(&format!("    {},\n", self.format(left_val).dimmed()));
                        }
                    }
                    (Some(left_val), None) => {
                        result.push_str(&format!("    {},\n", self.format(left_val).red()));
                    }
                    (None, Some(right_val)) => {
                        result.push_str(&format!("    {}\n", self.format(right_val).green()));
                    }
                    (None, None) => {}
                }
            }

            result.push_str(")");
            result
        }
    }
}
