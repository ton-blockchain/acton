use abi::{ContractAbi, TypeAbi};
use anyhow::anyhow;
use num_bigint::{BigInt, BigUint};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};
use tonlib_core::cell::{ArcCell, CellBuilder, CellParser};
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, IntAddr, MsgInfo, ShardAccount, Transaction, TxInfo,
};

#[derive(Default, Debug, Clone)]
pub struct Tuple(pub Vec<TupleItem>);

impl Tuple {
    pub fn empty() -> Tuple {
        Tuple(vec![])
    }

    pub fn unwrap_empty(&self) -> Tuple {
        if self.0.is_empty() {
            return (*self).clone();
        }

        if let TupleItem::Tuple(item) = &self.0[0]
            && item.len() == 0
        {
            return Tuple(vec![]);
        }

        (*self).clone()
    }
    pub fn unwrap_single(&self) -> Tuple {
        if self.0.is_empty() {
            return (*self).clone();
        }

        if let TupleItem::Tuple(item) = &self.0[0]
            && item.len() == 1
        {
            return Tuple(vec![item[0].clone()]);
        }

        (*self).clone()
    }
}

impl Deref for Tuple {
    type Target = Vec<TupleItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Tuple {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PartialEq for Tuple {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl fmt::Display for Tuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.len() == 1 {
            write!(f, "{}", self.0[0])
        } else {
            write!(f, "(")?;
            for (i, item) in self.0.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", item)?;
            }
            write!(f, ")")
        }
    }
}

impl Tuple {
    pub fn push_string(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let total_bits = bytes.len() * 8;

        if total_bits <= 1023 {
            let mut b = CellBuilder::new();
            b.store_bits(total_bits, bytes).unwrap();
            self.push(TupleItem::Slice(TupleSLice {
                cell: ArcCell::from(b.build().unwrap()),
                start_bits: 0,
                end_bits: total_bits as u32,
                end_refs: 0,
                start_refs: 0,
            }));
        } else {
            let mut remaining_bytes = bytes;
            let mut cell_data = Vec::new();

            while !remaining_bytes.is_empty() {
                let chunk_size = std::cmp::min(remaining_bytes.len(), 127); // 127 bytes = 1016 bits < 1023
                let chunk = &remaining_bytes[..chunk_size];
                cell_data.push((chunk, chunk.len() * 8));
                remaining_bytes = &remaining_bytes[chunk_size..];
            }

            // build cells from last to first
            let cell_count = cell_data.len();
            let first_cell_bits = cell_data[0].1 as u32;
            let mut next_cell: Option<ArcCell> = None;

            for (chunk, bits) in cell_data.into_iter().rev() {
                let mut b = CellBuilder::new();
                b.store_bits(bits, chunk).unwrap();

                if let Some(next) = next_cell {
                    b.store_reference(&next).unwrap();
                }

                next_cell = Some(ArcCell::from(b.build().unwrap()));
            }

            let root_cell = next_cell.unwrap();
            let refs_count = if cell_count > 1 { 1 } else { 0 };
            self.push(TupleItem::Slice(TupleSLice {
                cell: root_cell,
                start_bits: 0,
                end_bits: first_cell_bits,
                end_refs: refs_count,
                start_refs: 0,
            }));
        }
    }

    pub fn push_bool(&mut self, v: bool) {
        self.push(TupleItem::Int(if v {
            BigInt::from(-1)
        } else {
            BigInt::from(0)
        }));
    }
}

#[derive(Debug, Clone, Eq)]
pub struct TupleSLice {
    pub cell: ArcCell,
    pub start_bits: u32,
    pub end_bits: u32,
    pub start_refs: u32,
    pub end_refs: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationResult {
    pub name: String,
    pub code_boc64: String,
    pub code_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct BuildCache {
    pub built: HashMap<String, CompilationResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownAddress {
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct KnownAddresses {
    pub addresses: HashMap<IntAddr, KnownAddress>,
}

/// Represents a stack value in TON VM
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TupleItem {
    Null,
    Int(BigInt),
    Nan,
    Cell(ArcCell),
    Slice(TupleSLice),
    Builder(ArcCell),
    Tuple(Vec<TupleItem>),
    TypedTuple {
        type_name: String,
        items: Vec<TupleItem>,
        abi: Option<TypeAbi>,
        contract_abi: ContractAbi,
        accounts: HashMap<String, ShardAccount>,
        build_cache: BuildCache,
        known_addresses: KnownAddresses,
    },
}

impl TupleItem {
    pub fn unwrap_single(&self) -> TupleItem {
        let TupleItem::Tuple(items) = self else {
            return (*self).clone();
        };

        if items.len() == 1 {
            return items[0].clone();
        }

        (*self).clone()
    }
}

impl PartialEq for TupleSLice {
    fn eq(&self, other: &Self) -> bool {
        let self_bits_len = (self.end_bits - self.start_bits) as usize;
        let other_bits_len = (other.end_bits - other.start_bits) as usize;
        let self_refs_count = (self.end_refs - self.start_refs) as usize;
        let other_refs_count = (other.end_refs - other.start_refs) as usize;

        if self_bits_len != other_bits_len || self_refs_count != other_refs_count {
            // fast path
            return false;
        }

        let mut self_parser = self.cell.parser();
        let mut other_parser = other.cell.parser();

        if self_parser.skip_bits(self.start_bits as usize).is_err()
            || other_parser.skip_bits(other.start_bits as usize).is_err()
        {
            return false;
        }

        match (
            self_parser.load_bits(self_bits_len),
            other_parser.load_bits(other_bits_len),
        ) {
            (Ok(self_data), Ok(other_data)) => {
                if self_data != other_data {
                    return false;
                }
            }
            _ => return false,
        }

        let mut self_parser = self.cell.parser();
        let mut other_parser = other.cell.parser();

        for _ in 0..self_refs_count {
            match (self_parser.next_reference(), other_parser.next_reference()) {
                (Ok(self_ref), Ok(other_ref)) => {
                    if self_ref.cell_hash().unwrap() != other_ref.cell_hash().unwrap() {
                        return false;
                    }
                }
                _ => return false,
            }
        }

        true
    }
}

impl Default for TupleItem {
    fn default() -> Self {
        TupleItem::Null
    }
}

pub fn format_item_with_type(item: &TupleItem, type_name: &str) -> String {
    let item = item.unwrap_single();

    match item {
        TupleItem::Int(value) if type_name == "bool" => {
            if value == BigInt::from(0) {
                "false".to_string()
            } else if value == BigInt::from(18446744073709551615u64) {
                "true".to_string()
            } else {
                format!("{}", value)
            }
        }
        TupleItem::Slice(TupleSLice {
            cell,
            start_bits,
            end_bits,
            ..
        }) if type_name == "address" => {
            let length = end_bits - start_bits;
            let mut parser = cell.parser();
            let Ok(()) = parser.skip_bits(start_bits as usize) else {
                return "Slice(...)".to_string();
            };
            if length == 2 && parser.load_u8(2).unwrap_or(0) == 0 {
                return "addr_none".to_string();
            }
            if length != 267 {
                return "Slice(...)".to_string();
            }
            let Ok(address) = parser.load_address() else {
                return "Slice(...)".to_string();
            };
            address.to_string()
        }
        _ => format!("{}", item),
    }
}

impl fmt::Display for TupleItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TupleItem::Int(value) => {
                if *value == BigInt::from(18446744073709551615u64) {
                    write!(f, "-1")
                } else {
                    write!(f, "{}", value)
                }
            }
            TupleItem::Null => write!(f, "null"),
            TupleItem::Nan => write!(f, "NaN"),
            TupleItem::Cell(cell) => write!(f, "{:?}", cell),
            TupleItem::Slice(slice) => {
                if let Some(string) = snake_string_from_slice(slice) {
                    write!(f, "\"{}\"", string)
                } else {
                    write!(f, "Slice(...)")
                }
            }
            TupleItem::Builder(_) => write!(f, "Builder(...)"),
            TupleItem::Tuple(items) => {
                if items.len() == 1 {
                    write!(f, "{}", items[0])
                } else {
                    write!(f, "(")?;
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", item)?;
                    }
                    write!(f, ")")
                }
            }
            TupleItem::TypedTuple {
                type_name,
                items,
                abi,
                contract_abi,
                accounts,
                build_cache,
                known_addresses,
            } => {
                if type_name == "address" && items.len() == 1 {
                    let addr = &items[0];
                    return write!(f, "{}", format_item_with_type(addr, type_name));
                }

                if type_name == "TransactionList" && items.len() == 1 {
                    return write!(
                        f,
                        "{}",
                        format_transaction_list(
                            &items,
                            contract_abi,
                            accounts,
                            build_cache,
                            known_addresses
                        )
                    );
                }

                if items.len() == 1 {
                    write!(f, "{}", items[0])
                } else {
                    if let Some(struct_desc) = abi {
                        if items.len() == struct_desc.fields.len() {
                            write!(f, "{} {{\n", type_name)?;
                            for (i, (field, item)) in
                                struct_desc.fields.iter().zip(items.iter()).enumerate()
                            {
                                write!(
                                    f,
                                    "    {}: {}",
                                    field.name,
                                    format_item_with_type(item, &field.type_info.human_readable)
                                )?;
                                if i < struct_desc.fields.len() - 1 {
                                    write!(f, ",")?;
                                }
                                write!(f, "\n")?;
                            }
                            write!(f, "}}")?;
                            return Ok(());
                        }
                    }

                    write!(
                        f,
                        "{}({})",
                        type_name,
                        items
                            .iter()
                            .map(|item| format!("{}", item))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
        }
    }
}

pub fn snake_string_from_slice(slice: &TupleSLice) -> Option<String> {
    let TupleSLice {
        cell,
        start_bits,
        end_bits,
        start_refs,
        ..
    } = slice;

    let mut all_bits = Vec::new();

    let mut parser = cell.parser();
    parser.skip_bits(*start_bits as usize).ok()?;
    let bits_to_load = (end_bits - start_bits) as usize;
    if (bits_to_load % 8) != 0 {
        // this is most likely not a snake string
        return None;
    }

    let bytes_to_load = bits_to_load / 8;

    let bits = parser.load_bits(bytes_to_load * 8).ok()?;
    all_bits.extend_from_slice(&bits);

    if bytes_to_load < 127 {
        // no need to look up to refs
        let result = String::from_utf8(all_bits).ok()?;
        return Some(result);
    }

    // skip references if needed
    for _ in 0..*start_refs {
        parser.next_reference().ok()?;
    }

    let mut next_data_ref = parser.next_reference().ok()?;

    loop {
        let mut parser = next_data_ref.parser();

        let bytes_to_load = parser.remaining_bits() / 8;
        let bits = parser.load_bits(bytes_to_load * 8).ok()?;
        all_bits.extend_from_slice(&bits);

        if parser.remaining_refs() == 0 {
            // this cell is the end
            break;
        }

        next_data_ref = parser.next_reference().unwrap()
    }

    let result = String::from_utf8(all_bits).ok()?;
    Some(result)
}

fn show_addr(addr: &IntAddr) -> String {
    let raw = addr.as_std().unwrap().display_base64(true).to_string();
    raw[..6].to_string() + ".." + &raw[raw.len() - 6..]
}

fn format_transaction_list(
    items: &&Vec<TupleItem>,
    contract_abi: &ContractAbi,
    accounts: &HashMap<String, ShardAccount>,
    build_cache: &BuildCache,
    known_addresses: &KnownAddresses,
) -> String {
    let item = &items[0];
    let TupleItem::Tuple(items) = item else {
        return format!("{}", items[0]);
    };

    let txs = items
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

    let mut builder = "".to_string();

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

            let message_abi = contract_abi
                .messages
                .iter()
                .find(|msg| msg.opcode != Some(0) && msg.opcode == Some(opcode));

            let amount = info.value.tokens.into_inner() as f64 / 1e9;

            let src_contract_type =
                get_contract_type(accounts, build_cache, known_addresses, &info.src);
            if src_contract_type != "" {
                tx_builder += format!("{}", src_contract_type.cyan()).as_str();
            } else {
                tx_builder += show_addr(&info.src).dimmed().to_string().as_str();
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

            let dst_contract_type =
                get_contract_type(accounts, build_cache, known_addresses, &info.dst);
            if dst_contract_type != "" {
                tx_builder += format!("{}", dst_contract_type.cyan()).as_str();
            } else {
                tx_builder += show_addr(&info.dst).dimmed().to_string().as_str();
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

            // tx_builder += format!("  lt: {} prev_lt: {}", tx.lt, tx.prev_trans_lt).as_str();

            if tx.orig_status == AccountStatus::NotExists && tx.end_status == AccountStatus::Active
            {
                tx_builder += "\n";
                tx_builder += "└─".dimmed().to_string().as_str();
                tx_builder += " account created";
            }
            if tx.orig_status == AccountStatus::Active && tx.end_status == AccountStatus::NotExists
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

fn get_contract_type(
    accounts: &HashMap<String, ShardAccount>,
    build_cache: &BuildCache,
    known_addresses: &KnownAddresses,
    addr: &IntAddr,
) -> String {
    let known_address = known_addresses.addresses.iter().find(|(address, _info)| {
        let a1 = address.to_string();
        let s2 = addr.to_string();
        a1 == s2
    });
    if let Some(known_address) = known_address {
        return known_address.1.name.clone();
    }

    let account = accounts.get(&addr.to_string());
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

    let compilation_result = build_cache.built.iter().find(|(_name, result)| {
        result.code_hash.to_ascii_lowercase() == code.repr_hash().to_string()
    });

    if let Some(result) = compilation_result {
        return result.1.name.clone();
    }

    "".to_string()
}

/// Serialize a tuple item to a cell builder
pub fn serialize_tuple_item(
    builder: &mut CellBuilder,
    src: &TupleItem,
) -> Result<(), anyhow::Error> {
    match src {
        TupleItem::Null => {
            builder.store_u8(8, 0x00)?;
        }
        TupleItem::Int(value) => {
            // Check if value fits in int64
            if value <= &BigInt::from(9223372036854775807i64)
                && value >= &BigInt::from(-9223372036854775808i64)
            {
                builder.store_u8(8, 0x01)?;
                builder.store_int(64, value)?;
                return Ok(());
            }
            // Use int257 for larger values
            builder.store_u16(15, 0x0100)?;
            builder.store_int(257, &value.clone().into())?;
        }
        TupleItem::Nan => {
            builder.store_u16(16, 0x02ff)?;
        }
        TupleItem::Cell(cell) => {
            builder.store_u8(8, 0x03)?;
            builder.store_reference(&cell)?;
        }
        TupleItem::Slice(TupleSLice {
            cell,
            start_bits,
            end_bits,
            start_refs,
            end_refs,
        }) => {
            builder.store_u8(8, 0x04)?;
            builder.store_u32(10, *start_bits)?;
            builder.store_u32(10, *end_bits)?;
            builder.store_u32(3, *start_refs)?;
            builder.store_u32(3, *end_refs)?;
            builder.store_reference(&cell)?;
        }
        TupleItem::Builder(cell) => {
            builder.store_u8(8, 0x05)?;
            builder.store_reference(&cell)?;
        }
        TupleItem::Tuple(items) => {
            let mut head: Option<ArcCell> = None;
            let mut tail: Option<ArcCell> = None;

            for (i, item) in items.iter().enumerate() {
                let s = head;
                head = tail;
                tail = s;

                if i > 1 {
                    let mut bc = CellBuilder::new();
                    bc.store_reference(tail.as_ref().unwrap())?;
                    bc.store_reference(head.as_ref().unwrap())?;
                    head = Some(ArcCell::new(bc.build()?));
                }

                let mut bc = CellBuilder::new();
                serialize_tuple_item(&mut bc, item)?;
                tail = Some(ArcCell::new(bc.build()?));
            }

            builder.store_u8(8, 0x07)?;
            builder.store_u16(16, items.len() as u16)?;
            if let Some(h) = &head {
                builder.store_reference(h)?;
            }
            if let Some(t) = &tail {
                builder.store_reference(t)?;
            }
        }
        TupleItem::TypedTuple { items, .. } => {
            serialize_tuple_item(builder, &TupleItem::Tuple(items.clone()))?
        }
    }
    Ok(())
}

/// Parse a tuple item from a cell parser
pub fn parse_tuple_item(parser: &mut CellParser) -> Result<TupleItem, anyhow::Error> {
    let kind = parser.load_u8(8)?;

    match kind {
        0 => Ok(TupleItem::Null),
        1 => {
            let value = parser.load_i64(64)?;
            Ok(TupleItem::Int(BigInt::from(value as u64)))
        }
        2 => {
            if parser.load_u64(7)? == 0 {
                let value = parser.load_int(257)?;
                Ok(TupleItem::Int(value))
            } else {
                parser.load_bit()?;
                Ok(TupleItem::Nan)
            }
        }
        3 => {
            let cell = parser.next_reference()?;
            Ok(TupleItem::Cell(cell))
        }
        4 => {
            let start_bits = parser.load_u32(10)?;
            let end_bits = parser.load_u32(10)?;
            let start_refs = parser.load_u32(3)?;
            let end_refs = parser.load_u32(3)?;

            let cell_ref = parser.next_reference()?;

            Ok(TupleItem::Slice(TupleSLice {
                cell: cell_ref,
                start_bits,
                end_bits,
                start_refs,
                end_refs,
            }))
        }
        5 => {
            let cell = parser.next_reference()?;
            Ok(TupleItem::Builder(cell))
        }
        7 => {
            let length = parser.load_u16(16)? as usize;
            let mut items: Vec<TupleItem> = Vec::with_capacity(length);

            if length > 1 {
                let head_ref = parser.next_reference()?;
                let tail_ref = parser.next_reference()?;

                let mut tail_parser = tail_ref.parser();
                items.insert(0, parse_tuple_item(&mut tail_parser)?);

                let mut head_refs = vec![head_ref];
                let mut current_parser = head_refs[0].parser();

                for _ in 0..length - 2 {
                    let old_head = current_parser.next_reference()?;
                    let new_tail = current_parser.next_reference()?;

                    let mut new_tail_parser = new_tail.parser();
                    items.insert(0, parse_tuple_item(&mut new_tail_parser)?);

                    head_refs.push(old_head);
                    current_parser = head_refs[head_refs.len() - 1].parser();
                }

                items.insert(0, parse_tuple_item(&mut current_parser)?);
            } else if length == 1 {
                let ref_cell = parser.next_reference()?;
                let mut item_parser = ref_cell.parser();
                items.push(parse_tuple_item(&mut item_parser)?);
            }

            Ok(TupleItem::Tuple(items))
        }
        6 => {
            // TODO: support continuation
            Ok(TupleItem::Null)
        }
        _ => Err(anyhow!("Unsupported stack item kind: {}", kind).into()),
    }
}

/// Serialize a tuple (stack) to a cell
pub fn serialize_tuple(src: &[TupleItem]) -> Result<ArcCell, anyhow::Error> {
    let mut builder = CellBuilder::new();
    builder.store_uint(24, &BigUint::from(src.len()))?;
    serialize_tuple_tail(src, &mut builder)?;
    Ok(ArcCell::new(builder.build()?))
}

fn serialize_tuple_tail(src: &[TupleItem], builder: &mut CellBuilder) -> Result<(), anyhow::Error> {
    if !src.is_empty() {
        // rest:^(VmStackList n)
        let mut tail_builder = CellBuilder::new();
        serialize_tuple_tail(&src[..src.len() - 1], &mut tail_builder)?;
        let tail_cell = ArcCell::new(tail_builder.build()?);
        builder.store_reference(&tail_cell)?;

        // tos
        serialize_tuple_item(builder, &src[src.len() - 1])?;
    }
    Ok(())
}

/// Parse a tuple (stack) from a cell
pub fn parse_tuple(src: &ArcCell) -> Result<Vec<TupleItem>, anyhow::Error> {
    let mut cur_cell = ArcCell::clone(src);
    let mut cs = cur_cell.parser();

    let size = cs.load_u32(24)? as usize;
    let mut result: Vec<TupleItem> = Vec::with_capacity(size);

    for _ in 0..size {
        let next_ref = cs.next_reference()?;
        let item = parse_tuple_item(&mut cs)?;
        result.insert(0, item);

        cur_cell = ArcCell::clone(&next_ref);
        cs = cur_cell.parser();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_test(item: TupleItem) {
        let mut builder = CellBuilder::new();
        serialize_tuple_item(&mut builder, &item).unwrap();
        let cell = ArcCell::new(builder.build().unwrap());

        let mut parser = cell.parser();
        let deserialized = parse_tuple_item(&mut parser).unwrap();

        assert_eq!(item, deserialized);
    }

    fn roundtrip_tuple_test(items: Vec<TupleItem>) {
        let serialized = serialize_tuple(&items).unwrap();
        let deserialized = parse_tuple(&serialized).unwrap();

        assert_eq!(items, deserialized);
    }

    #[test]
    fn test_null_roundtrip() {
        roundtrip_test(TupleItem::Null);
    }

    #[test]
    fn test_small_int_roundtrip() {
        roundtrip_test(TupleItem::Int(BigInt::from(42u64)));
        // roundtrip_test(TupleItem::Int(BigInt::from(i64::MIN)));
        // roundtrip_test(TupleItem::Int(BigInt::from(i64::MAX)));
    }

    #[test]
    fn test_large_int_roundtrip() {
        // Large integer that doesn't fit in i64
        let large_int = BigInt::from(1u128 << 100);
        roundtrip_test(TupleItem::Int(large_int));
    }

    #[test]
    fn test_nan_roundtrip() {
        roundtrip_test(TupleItem::Nan);
    }

    #[test]
    fn test_cell_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_u8(8, 42).unwrap();
        let test_cell = ArcCell::new(builder.build().unwrap());

        roundtrip_test(TupleItem::Cell(test_cell));
    }

    #[test]
    fn test_slice_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_u8(8, 42).unwrap();
        builder.store_u8(8, 43).unwrap();
        let test_cell = ArcCell::new(builder.build().unwrap());

        roundtrip_test(TupleItem::Slice(TupleSLice {
            cell: test_cell,
            start_bits: 0,
            end_bits: 16,
            start_refs: 0,
            end_refs: 0,
        }));
    }

    #[test]
    fn test_builder_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_u8(8, 42).unwrap();
        let test_cell = ArcCell::new(builder.build().unwrap());

        roundtrip_test(TupleItem::Builder(test_cell));
    }

    #[test]
    fn test_empty_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(vec![]));
    }

    #[test]
    fn test_single_item_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(vec![TupleItem::Null]));
        roundtrip_test(TupleItem::Tuple(vec![TupleItem::Int(BigInt::from(123u64))]));
    }

    #[test]
    fn test_multi_item_tuple_roundtrip() {
        let items = vec![
            TupleItem::Null,
            TupleItem::Int(BigInt::from(42u64)),
            TupleItem::Nan,
        ];
        roundtrip_test(TupleItem::Tuple(items));
    }

    #[test]
    fn test_nested_tuple_roundtrip() {
        let inner_tuple =
            TupleItem::Tuple(vec![TupleItem::Null, TupleItem::Int(BigInt::from(1u64))]);
        let outer_tuple = TupleItem::Tuple(vec![inner_tuple, TupleItem::Int(BigInt::from(2u64))]);
        roundtrip_test(outer_tuple);
    }

    #[test]
    fn test_tuple_stack_roundtrip() {
        roundtrip_tuple_test(vec![]);
        roundtrip_tuple_test(vec![TupleItem::Null]);
        let items = vec![
            TupleItem::Null,
            TupleItem::Int(BigInt::from(42u64)),
            TupleItem::Nan,
        ];
        roundtrip_tuple_test(items);
    }

    #[test]
    fn test_string_roundtrip() {
        // Test small string (fits in one cell)
        let small_string = "Hello World";
        let mut tuple = Tuple::empty();
        tuple.push_string(small_string);
        let serialized = serialize_tuple(&tuple.0).unwrap();
        let deserialized = parse_tuple(&serialized).unwrap();
        assert_eq!(tuple.0, deserialized);

        // Test large string (requires SnakeString)
        let large_string = "A".repeat(200); // 200 bytes = 1600 bits > 1023
        let mut tuple = Tuple::empty();
        tuple.push_string(&large_string);
        let serialized = serialize_tuple(&tuple.0).unwrap();
        let deserialized = parse_tuple(&serialized).unwrap();
        assert_eq!(tuple.0, deserialized);
    }
}
