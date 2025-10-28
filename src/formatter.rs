use crate::context::{BuildCache, KnownAddresses};
use abi::ContractAbi;
use num_bigint::BigInt;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fmt::Write;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, IntAddr, MsgInfo, ShardAccount, Transaction, TxInfo,
};

/// Context for formatting TupleItems with rich information
#[derive(Debug, Clone)]
pub struct FormatterContext {
    pub contract_abi: ContractAbi,
    pub accounts: HashMap<String, ShardAccount>,
    pub build_cache: BuildCache,
    pub known_addresses: KnownAddresses,
}

impl FormatterContext {
    pub fn empty() -> Self {
        Self {
            contract_abi: ContractAbi::default(),
            accounts: HashMap::new(),
            build_cache: BuildCache::new(),
            known_addresses: KnownAddresses::new(),
        }
    }

    /// Create formatter context from the main Context
    pub fn from_context(ctx: &crate::context::Context) -> Self {
        Self {
            contract_abi: ctx.abi.clone(),
            accounts: ctx.blockchain.get_accounts().clone(),
            build_cache: ctx.build_cache.clone(),
            known_addresses: ctx.known_addresses.clone(),
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

    /// Format transaction list
    pub fn format_transaction_list(&self, items: &[TupleItem]) -> String {
        let item = &items[0];
        let TupleItem::Tuple(tx_items) = item else {
            return self.format(&items[0]);
        };

        let txs = tx_items
            .iter()
            .filter_map(|el| match el {
                TupleItem::Cell(cell) => Some(cell),
                _ => None,
            })
            .map(|x| {
                let result = x.to_boc_b64(false).unwrap();
                let tx_cell: Cell = Boc::decode_base64(&result).unwrap();
                let mut tx_slice = tx_cell.as_slice().unwrap();
                Transaction::load_from(&mut tx_slice).unwrap()
            })
            .collect::<Vec<_>>();

        let mut builder = String::new();

        let mut known_contracts: Vec<IntAddr> = vec![];

        for tx in &txs {
            let in_msg = tx.load_in_msg().unwrap();
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

        let mut contract_letters: HashMap<IntAddr, String> = HashMap::new();

        for (index, addr) in known_contracts.iter().enumerate() {
            let letter = char::from_u32('A' as u32 + index as u32)
                .unwrap_or_else(|| char::from_digit(index as u32, 10).unwrap());
            contract_letters.insert(addr.clone(), letter.to_string());
        }

        for tx in txs {
            let mut tx_builder = "\x1b[0m".to_string();

            tx_builder += "\x1b[0m";
            let in_msg = tx.load_in_msg().unwrap();
            if let Some(in_msg) = &in_msg
                && let MsgInfo::Int(info) = &in_msg.info
            {
                if info.bounced {
                    tx_builder += "(!) ".red().to_string().as_str()
                }

                let mut body = in_msg.body.clone();
                let mut opcode = body.load_u32().unwrap_or(0);
                if opcode == 0xFFFFFFFF {
                    // if bounce read another 32 bit to get actual opcode
                    opcode = body.load_u32().unwrap_or(0);
                }

                let message_abi = self
                    .contract_abi
                    .messages
                    .iter()
                    .find(|msg| msg.opcode != Some(0) && msg.opcode == Some(opcode));

                let amount = info.value.tokens.into_inner() as f64 / 1e9;

                let src_contract_type = self.get_contract_type(&info.src);
                if src_contract_type != "" {
                    tx_builder += format!("{}", src_contract_type.cyan()).as_str();
                } else {
                    tx_builder += Self::format_addr_hash(&info.src)
                        .dimmed()
                        .to_string()
                        .as_str();
                }

                let letter = contract_letters.get(&info.src);
                if let Some(letter) = letter {
                    tx_builder += format!(" {}  ", letter.bold()).as_str();
                }

                tx_builder += " ";
                tx_builder += "-> ";

                if let Some(message_abi) = message_abi {
                    tx_builder += message_abi
                        .name
                        .as_str()
                        .purple()
                        .bold()
                        .to_string()
                        .as_str();
                } else if opcode == 0 {
                    tx_builder += "empty".purple().bold().to_string().as_str();
                } else {
                    tx_builder += format!("0x{:x}", opcode)
                        .purple()
                        .bold()
                        .to_string()
                        .as_str();
                }
                tx_builder += " ";

                tx_builder += &format!("{} TON", amount.to_string()).green().to_string();
                tx_builder += " -> ";

                let dst_contract_type = self.get_contract_type(&info.dst);
                if dst_contract_type != "" {
                    tx_builder += format!("{}", dst_contract_type.cyan()).as_str();
                } else {
                    tx_builder += Self::format_addr_hash(&info.dst)
                        .dimmed()
                        .to_string()
                        .as_str();
                }

                let letter = contract_letters.get(&info.dst);
                if let Some(letter) = letter {
                    tx_builder += format!(" {}  ", letter.bold()).as_str();
                }
            }

            let TxInfo::Ordinary(info) = tx.load_info().unwrap() else {
                panic!("tick-tock message is unexpected")
            };

            if let ComputePhase::Executed(compute) = info.compute_phase {
                tx_builder += format!(" gas={}", compute.gas_used.to_string().as_str())
                    .dimmed()
                    .to_string()
                    .as_str();

                if compute.exit_code != 0 {
                    tx_builder += format!(" exit_code={}", compute.exit_code)
                        .red()
                        .to_string()
                        .as_str();
                }

                if tx.orig_status == AccountStatus::NotExists
                    && tx.end_status == AccountStatus::Active
                {
                    tx_builder += "\n";
                    tx_builder += "└─".dimmed().to_string().as_str();
                    tx_builder += " account created";
                }
                if tx.orig_status == AccountStatus::Active
                    && tx.end_status == AccountStatus::NotExists
                {
                    tx_builder += "\n";
                    tx_builder += "└─".dimmed().to_string().as_str();
                    tx_builder += " account destroyed"
                }
            } else {
                tx_builder += format!(" {}", "compute phase skipped".dimmed()).as_str();
            }

            builder.push_str(&tx_builder);
            builder.push_str("\n");
        }

        builder
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
            TupleItem::TypedTuple { type_name, items } => {
                if items.is_empty() {
                    return type_name.clone();
                }

                if type_name == "TransactionList" && items.len() == 1 {
                    return self.format_transaction_list(items);
                }

                let abi = self.contract_abi.find_type(type_name);

                // Format structure as Foo { ... }
                if let Some(struct_desc) = abi
                    && items.len() == struct_desc.fields.len()
                {
                    // TODO: support structures with nested structures
                    let mut f = "".to_string();

                    write!(f, "{} {{\n", type_name).ok();
                    for (i, (field, item)) in
                        struct_desc.fields.iter().zip(items.iter()).enumerate()
                    {
                        write!(
                            f,
                            "    {}: {}",
                            field.name,
                            self.format(&item.to_typed(&field.type_info.human_readable))
                        )
                        .ok();
                        if i < struct_desc.fields.len() - 1 {
                            write!(f, ",").ok();
                        }
                        write!(f, "\n").ok();
                    }
                    write!(f, "}}").ok();
                    return f;
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

                format!("{}", self.format_tuple(&Tuple((*items).clone())))
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

    /// Show address in short format
    fn format_addr_hash(addr: &IntAddr) -> String {
        let raw = addr.as_std().unwrap().display_base64(true).to_string();
        raw[..6].to_string() + ".." + &raw[raw.len() - 6..]
    }

    pub fn format_address(&self, txs: &TupleItem, addr: &Option<IntAddr>) -> String {
        let Some(addr) = addr else {
            return "<any>".cyan().to_string();
        };

        let TupleItem::TypedTuple { items, .. } = txs else {
            return Self::format_addr_hash(&addr);
        };

        let TupleItem::Tuple(items) = &items[0] else {
            return self.format(&items[0]);
        };

        let txs = items
            .iter()
            .filter_map(|el| match el {
                TupleItem::Cell(cell) => Some(cell),
                _ => None,
            })
            .map(|x| {
                let result = x.to_boc_b64(false).unwrap();
                let tx_cell: tycho_types::cell::Cell = Boc::decode_base64(&result).unwrap();
                let mut tx_slice = tx_cell.as_slice().unwrap();
                Transaction::load_from(&mut tx_slice).unwrap()
            })
            .collect::<Vec<_>>();

        let mut known_contracts: Vec<IntAddr> = vec![];

        for tx in &txs {
            let in_msg = tx.load_in_msg().unwrap();
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

        let mut contract_letters: HashMap<IntAddr, String> = HashMap::new();

        for (index, addr) in known_contracts.iter().enumerate() {
            let letter = char::from_u32('A' as u32 + index as u32)
                .unwrap_or_else(|| char::from_digit(index as u32, 10).unwrap());
            contract_letters.insert(addr.clone(), letter.to_string());
        }

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
