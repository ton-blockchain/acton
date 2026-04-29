use std::collections::HashMap;
use std::fmt::{self, Write};
use std::sync::OnceLock;
use tolk_compiler::abi::{ABIDeclaration, ABIEnumMember, ABIOpcode, ABIStructField, ContractABI};
use tolk_compiler::source_map::{AbiStruct, Declaration, SourceMap};
use tolk_compiler::types_kernel::{Ty, calc_width_on_stack, instantiate_generics};
use ton_abi::abi_serde::Data as ParsedAbiData;
use ton_abi::compiler_abi_serde;
use tvm_ffi::from_stack::FromStack;
use tvm_ffi::stack::{Tuple, TupleItem};
use tvm_logs::parser::{CellLike, CellSlice, VmStackValue};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellSlice as TyCellSlice, Load};
use tycho_types::dict;
use tycho_types::models::{
    Base64StdAddrFlags, ChangeLibraryMode, CurrencyCollection, DisplayBase64StdAddr, IntAddr,
    LibRef, OutAction, OutActionsRevIter, OwnedRelaxedMessage, RelaxedMsgInfo,
    ReserveCurrencyFlags, SendMsgFlags, StateInit, StdAddr,
};

// ---------------------------------------------------------------------------
// RenderedValue — structured intermediate format for rendered values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum RenderedValue {
    Leaf {
        value: String,
        type_field: Option<String>,
    },
    CellLike {
        type_name: String,
        value: String,
        fields: Vec<(String, RenderedValue)>, // "bits" and "refs" fields
        raw: Option<CellLike>,
    },
    CellOf {
        type_name: String,
        value: String,
        fields: Vec<(String, RenderedValue)>, // "decoded" field with actual value
        raw: Option<CellLike>,
    },
    EnumValue {
        type_name: String,
        value: String,
        fields: Vec<(String, RenderedValue)>,
    },
    UnionCase {
        type_name: String,
        variant_name: String,
        fields: Vec<(String, RenderedValue)>,
    },
    Struct {
        type_name: String,
        fields: Vec<(String, RenderedValue)>,
    },
    Address {
        type_name: String,
        legacy_value: String,
        value: String,
        fields: Vec<(String, RenderedValue)>, // various formats
    },
    Tensor {
        type_name: String,
        items: Vec<RenderedValue>,
    },
    ArrayOf {
        type_name: String,
        items: Vec<RenderedValue>,
    },

    LastSeen {
        inner: Box<RenderedValue>,
    },
    OptimizedOut,
    LazyNotYetLoaded {
        preview: Box<RenderedValue>,
    },
    LazyCantParseSlice,
    LazyUnresolved {
        type_name: String,
    },
}

impl RenderedValue {
    pub fn leaf(value: impl Into<String>) -> Self {
        Self::Leaf {
            value: value.into(),
            type_field: None,
        }
    }

    pub fn typed_leaf(value: impl Into<String>, type_field: impl Into<String>) -> Self {
        Self::Leaf {
            value: value.into(),
            type_field: Some(type_field.into()),
        }
    }

    /// Build `(value, type)` the way DAP UIs expect it.
    ///
    /// For structs we keep the type name in `type` instead of duplicating it in `value`.
    pub fn dap_parts(&self) -> (String, Option<String>) {
        match self {
            RenderedValue::Leaf { value, type_field } => (value.clone(), type_field.clone()),
            RenderedValue::CellLike {
                type_name, value, ..
            }
            | RenderedValue::CellOf {
                type_name, value, ..
            }
            | RenderedValue::EnumValue {
                type_name, value, ..
            }
            | RenderedValue::Address {
                type_name, value, ..
            } => (value.clone(), Some(type_name.clone())),
            RenderedValue::UnionCase {
                type_name,
                variant_name,
                ..
            } => (variant_name.clone(), Some(type_name.clone())),
            RenderedValue::Struct { type_name, .. } => (String::new(), Some(type_name.clone())),
            RenderedValue::Tensor { type_name, items }
            | RenderedValue::ArrayOf { type_name, items } => {
                (format!("{} items", items.len()), Some(type_name.clone()))
            }
            RenderedValue::LastSeen { inner } => {
                let (value, type_field) = inner.dap_parts();
                let value = if value.is_empty() {
                    "(last seen)".to_string()
                } else {
                    format!("{value} (last seen)")
                };
                (value, type_field)
            }
            RenderedValue::OptimizedOut => ("<optimized out>".to_string(), None),
            RenderedValue::LazyNotYetLoaded { preview } => {
                let (value, type_field) = preview.dap_parts();
                let value = if value.is_empty() {
                    "<not loaded>".to_string()
                } else {
                    format!("{value} (not loaded)")
                };
                (value, type_field)
            }
            RenderedValue::LazyCantParseSlice => ("<not loaded>".to_string(), None),
            RenderedValue::LazyUnresolved { type_name } => {
                ("(lazy, unresolved)".to_string(), Some(type_name.clone()))
            }
        }
    }

    fn legacy_dap_value(&self, name: Option<&str>) -> String {
        match self {
            RenderedValue::Leaf {
                value,
                type_field: Some(type_field),
            } if type_field == "coins"
                && name.is_some_and(|name| identifier_has_word(name, "ton")) =>
            {
                format_coins_for_debug(value).unwrap_or_else(|| value.clone())
            }
            RenderedValue::Leaf { value, .. }
            | RenderedValue::CellLike { value, .. }
            | RenderedValue::EnumValue { value, .. } => value.clone(),
            RenderedValue::CellOf {
                type_name, value, ..
            } => format!("{type_name} {value}"),
            RenderedValue::UnionCase { variant_name, .. } => variant_name.clone(),
            RenderedValue::Struct { type_name, .. } => type_name.clone(),
            RenderedValue::Address { legacy_value, .. } => legacy_value.clone(),
            RenderedValue::Tensor { items, .. } | RenderedValue::ArrayOf { items, .. } => {
                format!("{} items", items.len())
            }
            RenderedValue::LastSeen { inner } => {
                format!("{} (last seen)", inner.legacy_dap_value(name))
            }
            RenderedValue::OptimizedOut => "<optimized out>".to_string(),
            RenderedValue::LazyNotYetLoaded { preview } => {
                format!("{} (not loaded)", preview.legacy_dap_value(name))
            }
            RenderedValue::LazyCantParseSlice => "<not loaded>".to_string(),
            RenderedValue::LazyUnresolved { type_name } => {
                format!("{type_name} (lazy, unresolved)")
            }
        }
    }

    pub fn dap_parts_for_client(&self, name: Option<&str>) -> (String, Option<String>) {
        if dap_legacy_value_enabled() {
            // TODO: remove legacy path
            (self.legacy_dap_value(name), None)
        } else if let Some(name) = name {
            self.dap_parts_for_name(name)
        } else {
            self.dap_parts()
        }
    }

    fn dap_parts_for_name(&self, name: &str) -> (String, Option<String>) {
        match self {
            RenderedValue::Leaf {
                value,
                type_field: Some(type_field),
            } if type_field == "coins" => {
                let value = if identifier_has_word(name, "ton") {
                    format_coins_for_debug(value).unwrap_or_else(|| value.clone())
                } else {
                    value.clone()
                };
                (value, Some(type_field.clone()))
            }
            RenderedValue::LastSeen { inner } => {
                let (value, type_field) = inner.dap_parts_for_name(name);
                let value = if value.is_empty() {
                    "(last seen)".to_string()
                } else {
                    format!("{value} (last seen)")
                };
                (value, type_field)
            }
            RenderedValue::LazyNotYetLoaded { preview } => {
                let (value, type_field) = preview.dap_parts_for_name(name);
                let value = if value.is_empty() {
                    "<not loaded>".to_string()
                } else {
                    format!("{value} (not loaded)")
                };
                (value, type_field)
            }
            _ => self.dap_parts(),
        }
    }

    pub fn dap_value(&self) -> String {
        self.dap_parts().0
    }

    pub fn has_children(&self) -> bool {
        match self {
            RenderedValue::Struct { fields, .. }
            | RenderedValue::Address { fields, .. }
            | RenderedValue::CellLike { fields, .. }
            | RenderedValue::CellOf { fields, .. }
            | RenderedValue::EnumValue { fields, .. }
            | RenderedValue::UnionCase { fields, .. } => !fields.is_empty(),
            RenderedValue::Tensor { items, .. } | RenderedValue::ArrayOf { items, .. } => {
                !items.is_empty()
            }
            RenderedValue::LastSeen { inner } => inner.has_children(),
            _ => false,
        }
    }

    pub(crate) fn raw_cell_like(&self) -> Option<&CellLike> {
        match self {
            RenderedValue::CellLike { raw: Some(raw), .. }
            | RenderedValue::CellOf { raw: Some(raw), .. } => Some(raw),
            RenderedValue::LastSeen { inner } => inner.raw_cell_like(),
            _ => None,
        }
    }
}

const DAP_LEGACY_VALUE_ENV: &str = "ACTON_DEBUG_DAP_USE_LEGACY_VALUE";

fn dap_legacy_value_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();

    *ENABLED.get_or_init(|| {
        std::env::var(DAP_LEGACY_VALUE_ENV)
            .ok()
            .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true"))
    })
}

fn format_coins_for_debug(tokens: &str) -> Option<String> {
    let tokens: u128 = tokens.parse().ok()?;
    let whole = tokens / 1_000_000_000;
    let frac = tokens % 1_000_000_000;
    if frac == 0 {
        return Some(format!("{whole} TON"));
    }

    let frac = format!("{frac:09}");
    let frac = frac.trim_end_matches('0');
    Some(format!("{whole}.{frac} TON"))
}

fn identifier_has_word(name: &str, needle: &str) -> bool {
    let mut start = None;
    let mut prev = None;
    let mut chars = name.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        if !ch.is_ascii_alphanumeric() {
            if let Some(start_idx) = start.take()
                && name[start_idx..idx].eq_ignore_ascii_case(needle)
            {
                return true;
            }
            prev = None;
            continue;
        }

        let next = chars.peek().map(|(_, next)| *next);
        if let Some(prev_ch) = prev
            && let Some(start_idx) = start
            && identifier_word_boundary(prev_ch, ch, next)
        {
            if name[start_idx..idx].eq_ignore_ascii_case(needle) {
                return true;
            }
            start = Some(idx);
        } else if start.is_none() {
            start = Some(idx);
        }

        prev = Some(ch);
    }

    start.is_some_and(|start_idx| name[start_idx..].eq_ignore_ascii_case(needle))
}

fn identifier_word_boundary(prev: char, current: char, next: Option<char>) -> bool {
    if prev.is_ascii_digit() != current.is_ascii_digit() {
        return true;
    }

    if prev.is_ascii_lowercase() && current.is_ascii_uppercase() {
        return true;
    }

    prev.is_ascii_uppercase()
        && current.is_ascii_uppercase()
        && next.is_some_and(|next| next.is_ascii_lowercase())
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
            Self::Address => StdAddr::BITS_WITHOUT_ANYCAST,
            Self::VarInt { .. } | Self::Cell | Self::String => 0,
        }
    }
}

impl fmt::Display for RenderedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderedValue::Leaf { value, .. }
            | RenderedValue::CellLike { value, .. }
            | RenderedValue::EnumValue { value, .. }
            | RenderedValue::Address { value, .. } => write!(f, "{value}"),
            RenderedValue::CellOf {
                type_name, value, ..
            } => write!(f, "{type_name} {value}"),
            RenderedValue::UnionCase {
                variant_name,
                fields,
                ..
            } => match fields.iter().find(|(name, _)| name == "value") {
                Some((_, value)) => write!(f, "{variant_name} {value}"),
                None => write!(f, "{variant_name}"),
            },
            RenderedValue::Struct { type_name, fields } if fields.is_empty() => {
                write!(f, "{type_name} {{}}")
            }
            RenderedValue::Struct { type_name, fields } => {
                write!(f, "{type_name} {{ ")?;
                for (i, (name, val)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{name}: {val}")?;
                }
                write!(f, " }}")
            }
            RenderedValue::Tensor { items, .. } => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            RenderedValue::ArrayOf { items, .. } => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            RenderedValue::LastSeen { inner } => write!(f, "{inner} (last seen)"),
            RenderedValue::OptimizedOut => write!(f, "<optimized out>"),
            RenderedValue::LazyNotYetLoaded { preview } => write!(f, "{preview} (not loaded)"),
            RenderedValue::LazyCantParseSlice => write!(f, "<not loaded>"),
            RenderedValue::LazyUnresolved { type_name } => {
                write!(f, "{type_name} (lazy, unresolved)")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SlotValue — per-slot state (live / last seen / optimized out)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub(crate) enum SlotValue<'a> {
    Live(&'a VmStackValue),
    LastSeen(&'a VmStackValue),
    OptimizedOut,
}

impl SlotValue<'_> {
    pub(crate) const fn is_optimized_out(&self) -> bool {
        matches!(self, SlotValue::OptimizedOut)
    }
}

// ---------------------------------------------------------------------------
// StackReader — cursor over SlotValues, inspired by the TS StackReader
// ---------------------------------------------------------------------------

struct StackReader<'a> {
    slots: &'a [SlotValue<'a>],
    pos: usize,
}

impl<'a> StackReader<'a> {
    pub(crate) const fn new(slots: &'a [SlotValue<'a>]) -> Self {
        Self { slots, pos: 0 }
    }

    pub(crate) fn read_slot(&mut self) -> SlotValue<'a> {
        if self.pos < self.slots.len() {
            let slot = self.slots[self.pos];
            self.pos += 1;
            slot
        } else {
            self.pos += 1;
            SlotValue::OptimizedOut
        }
    }

    pub(crate) fn peek_slot(&self) -> SlotValue<'a> {
        self.peek_at(0)
    }

    fn peek_at(&self, offset: usize) -> SlotValue<'a> {
        self.slots
            .get(self.pos + offset)
            .copied()
            .unwrap_or(SlotValue::OptimizedOut)
    }

    const fn skip(&mut self, n: usize) {
        self.pos += n;
    }

    fn peek_n_all_optimized_out(&self, n: usize) -> bool {
        (0..n).all(|i| self.peek_at(i).is_optimized_out())
    }

    fn peek_n_all_last_seen(&self, n: usize) -> bool {
        n > 0 && (0..n).all(|i| matches!(self.peek_at(i), SlotValue::LastSeen(_)))
    }

    pub(crate) fn read_n_slots(&mut self, n: usize) -> &'a [SlotValue<'a>] {
        let start = self.pos.min(self.slots.len());
        let end = (start + n).min(self.slots.len());
        self.pos = start + n;
        &self.slots[start..end]
    }
}

// ---------------------------------------------------------------------------
// Helpers for rendering specific types from TVM stack values
// ---------------------------------------------------------------------------

/// Extract a single bit from hex-encoded cell data (nibble array — half-bytes).
/// Each nibble is 4 bits, MSB first: bit N is in nibble N/4, position N%4 from MSB.
fn get_bit(nibbles: &[u8], bit_pos: usize) -> u8 {
    let idx = bit_pos / 4;
    if idx >= nibbles.len() {
        return 0;
    }
    (nibbles[idx] >> (3 - bit_pos % 4)) & 1
}

/// Read `count` consecutive bits (up to 8) as a u8 value.
fn get_bits_u8(nibbles: &[u8], start: usize, count: usize) -> u8 {
    let mut v: u8 = 0;
    for i in 0..count {
        v = (v << 1) | get_bit(nibbles, start + i);
    }
    v
}

/// Try to parse `addr_std` from a `CellSlice`.
/// Cell{hex} starts with 2 descriptor bytes (4 hex chars); cell data follows.
/// `bits: start..end` are positions within cell data.
/// `addr_std` = `10` (2b) + `0` (1b anycast) + workchain (8b) + hash (256b) = 267 bits.
fn try_parse_address(cs: &CellSlice) -> Option<String> {
    if cs.bits.is_none() && cs.refs.is_none() {
        return try_parse_full_address_hex(&cs.value);
    }

    let (start_s, end_s) = cs.bits.as_ref()?;
    let start: usize = start_s.parse().ok()?;
    let end: usize = end_s.parse().ok()?;
    if end - start != 267 {
        return None;
    }

    let data_hex = cs.value.get(4..)?; // skip d1, d2
    let nibbles: Vec<u8> = data_hex
        .chars()
        .filter_map(|c| c.to_digit(16).map(|d| d as u8))
        .collect();
    if nibbles.len() * 4 < end {
        return None;
    }

    if get_bits_u8(&nibbles, start, 3) != 0b100 {
        return None;
    } // addr_std prefix no anycast

    let wc = get_bits_u8(&nibbles, start + 3, 8) as i8;
    let mut hash = String::with_capacity(64);
    for i in 0..32 {
        write!(hash, "{:02x}", get_bits_u8(&nibbles, start + 11 + i * 8, 8)).ok()?;
    }
    Some(format!("{wc}:{hash}"))
}

fn try_parse_full_address_hex(hex: &str) -> Option<String> {
    let cell = Boc::decode_hex(hex).ok()?;
    StdAddr::from_item(TupleItem::Slice(cell))
        .ok()
        .map(|addr| addr.to_string())
}

fn render_std_address(type_name: String, legacy_value: String, addr: &StdAddr) -> RenderedValue {
    let raw = addr.to_string();
    let mainnet = DisplayBase64StdAddr {
        addr,
        flags: Base64StdAddrFlags {
            testnet: false,
            base64_url: true,
            bounceable: true,
        },
    }
    .to_string();
    let testnet = DisplayBase64StdAddr {
        addr,
        flags: Base64StdAddrFlags {
            testnet: true,
            base64_url: true,
            bounceable: true,
        },
    }
    .to_string();

    RenderedValue::Address {
        type_name,
        legacy_value,
        value: raw.clone(),
        fields: vec![
            ("raw".to_string(), RenderedValue::leaf(raw)),
            ("mainnet".to_string(), RenderedValue::leaf(mainnet)),
            ("testnet".to_string(), RenderedValue::leaf(testnet)),
        ],
    }
}

fn try_parse_string_hex(hex: &str) -> Option<String> {
    let cell = Boc::decode_hex(hex).ok()?;
    Tuple::parse_snake_string(&cell)
}

fn try_parse_string_cell_like(cell: &CellLike) -> Option<String> {
    match cell {
        CellLike::Cell(hex) | CellLike::Builder(hex) => try_parse_string_hex(hex),
    }
}

fn try_parse_string_slice(cs: &CellSlice) -> Option<String> {
    if cs.bits.is_none() && cs.refs.is_none() {
        return try_parse_string_hex(&cs.value);
    }

    None
}

fn render_cell_like(cell: &CellLike) -> String {
    match cell {
        CellLike::Cell(hex) | CellLike::Builder(hex) => format!("cell{{{hex}}}"),
    }
}

fn decode_cell_like(cell: &CellLike) -> Option<Cell> {
    match cell {
        CellLike::Cell(hex) | CellLike::Builder(hex) => Boc::decode_hex(hex).ok(),
    }
}

fn parse_range_len(range: &Option<(String, String)>) -> Option<usize> {
    let (start, end) = range.as_ref()?;
    Some(end.parse::<usize>().ok()? - start.parse::<usize>().ok()?)
}

fn render_cell_hash(cell: &Cell) -> String {
    format!("0x{}", cell.repr_hash().to_string().to_ascii_lowercase())
}

fn render_cell_hash_prefix(hash: &str) -> String {
    const PREFIX_LEN: usize = 10;
    if hash.len() <= PREFIX_LEN {
        hash.to_owned()
    } else {
        format!("{}...", &hash[..PREFIX_LEN])
    }
}

fn render_cell_summary(bits: Option<usize>, refs: Option<usize>, hash: Option<&str>) -> String {
    let bits = bits.map_or_else(
        || "<unknown> bits".to_owned(),
        |bits| format!("{bits} bits"),
    );
    let refs = refs.map_or_else(
        || "<unknown> refs".to_owned(),
        |refs| format!("{refs} refs"),
    );
    let hash = hash.map_or_else(|| "<unknown hash>".to_owned(), render_cell_hash_prefix);
    format!("{bits}, {refs}, hash: {hash}")
}

fn render_cell_fields(
    bits: Option<usize>,
    refs: Option<usize>,
    hash: Option<String>,
    raw_value: Option<String>,
) -> Vec<(String, RenderedValue)> {
    let mut fields = vec![
        (
            "bits".to_owned(),
            RenderedValue::leaf(
                bits.map_or_else(|| "<unknown>".to_owned(), |bits| bits.to_string()),
            ),
        ),
        (
            "refs".to_owned(),
            RenderedValue::leaf(
                refs.map_or_else(|| "<unknown>".to_owned(), |refs| refs.to_string()),
            ),
        ),
        (
            "hash".to_owned(),
            RenderedValue::leaf(hash.unwrap_or_else(|| "<unknown>".to_owned())),
        ),
    ];
    if let Some(raw_value) = raw_value {
        fields.push(("raw".to_owned(), RenderedValue::leaf(raw_value)));
    }
    fields
}

fn cell_like_meta(cell: &CellLike) -> (Option<usize>, Option<usize>, Option<String>) {
    let Some(cell) = decode_cell_like(cell) else {
        return (None, None, None);
    };
    let slice = cell.as_slice_allow_exotic();
    (
        Some(slice.size_bits() as usize),
        Some(slice.size_refs() as usize),
        Some(render_cell_hash(&cell)),
    )
}

fn exact_slice_cell(cs: &CellSlice) -> Option<Cell> {
    let cell = Boc::decode_hex(&cs.value).ok()?;
    match (&cs.bits, &cs.refs) {
        (Some((start_bits, end_bits)), Some((start_refs, end_refs))) => {
            let start_bits = start_bits.parse::<u16>().ok()?;
            let end_bits = end_bits.parse::<u16>().ok()?;
            let start_refs = start_refs.parse::<u8>().ok()?;
            let end_refs = end_refs.parse::<u8>().ok()?;

            let mut parser = cell.as_slice_allow_exotic();
            parser.skip_first(start_bits, start_refs).ok()?;

            let bit_len = end_bits.saturating_sub(start_bits);
            let mut root_bits = vec![0u8; bit_len.div_ceil(8) as usize];
            parser.load_raw(&mut root_bits, bit_len).ok()?;

            let mut builder = CellBuilder::new();
            builder.store_raw(&root_bits, bit_len).ok()?;
            for _ in start_refs..end_refs {
                let next_ref = parser.load_reference_cloned().ok()?;
                builder.store_reference(next_ref).ok()?;
            }

            builder.build().ok()
        }
        _ => Some(cell),
    }
}

fn slice_meta(cs: &CellSlice) -> (Option<usize>, Option<usize>, Option<String>) {
    let (bits, refs) = match (&cs.bits, &cs.refs) {
        (Some(_), Some(_)) => (parse_range_len(&cs.bits), parse_range_len(&cs.refs)),
        _ => match exact_slice_cell(cs) {
            Some(cell) => {
                let slice = cell.as_slice_allow_exotic();
                (
                    Some(slice.size_bits() as usize),
                    Some(slice.size_refs() as usize),
                )
            }
            None => (None, None),
        },
    };
    let hash = exact_slice_cell(cs).as_ref().map(render_cell_hash);
    (bits, refs, hash)
}

fn slice_as_cell_like(cs: &CellSlice) -> Option<CellLike> {
    exact_slice_cell(cs).map(|cell| CellLike::Cell(Boc::encode_hex(&cell)))
}

fn render_openable_cell_like(
    ty: &Ty,
    value: impl Into<String>,
    bits: Option<usize>,
    refs: Option<usize>,
    hash: Option<String>,
    raw: Option<CellLike>,
) -> RenderedValue {
    let raw_value = value.into();
    let summarize_value = bits.is_some() || refs.is_some() || hash.is_some();

    RenderedValue::CellLike {
        type_name: ty.to_string(),
        value: if summarize_value {
            render_cell_summary(bits, refs, hash.as_deref())
        } else {
            raw_value.clone()
        },
        fields: render_cell_fields(bits, refs, hash, summarize_value.then_some(raw_value)),
        raw,
    }
}

pub(crate) fn render_runtime_vm_value(value: &VmStackValue) -> RenderedValue {
    match value {
        VmStackValue::Null => RenderedValue::leaf("()"),
        VmStackValue::NaN => RenderedValue::leaf("NaN"),
        VmStackValue::Integer(value) => RenderedValue::leaf(value.clone()),
        VmStackValue::Continuation(value) => RenderedValue::leaf(format!("Cont{{{value}}}")),
        VmStackValue::String(value) => RenderedValue::leaf(format!("\"{value}\"")),
        VmStackValue::Unknown => RenderedValue::leaf("???"),
        VmStackValue::Cell(cell) => {
            let (bits, refs, hash) = cell_like_meta(cell);
            render_openable_cell_like(
                &Ty::Cell,
                render_cell_like(cell),
                bits,
                refs,
                hash,
                Some(cell.clone()),
            )
        }
        VmStackValue::Builder(builder_hex) => {
            let cell_like = CellLike::Builder(builder_hex.clone());
            let (bits, refs, hash) = cell_like_meta(&cell_like);
            render_openable_cell_like(
                &Ty::Builder,
                render_builder(builder_hex),
                bits,
                refs,
                hash,
                Some(cell_like),
            )
        }
        VmStackValue::CellSlice(slice) => {
            let (bits, refs, hash) = slice_meta(slice);
            render_openable_cell_like(
                &Ty::Slice,
                render_slice(slice),
                bits,
                refs,
                hash,
                slice_as_cell_like(slice),
            )
        }
        VmStackValue::Tuple(items) => RenderedValue::ArrayOf {
            type_name: "tuple".to_owned(),
            items: items.iter().map(render_runtime_vm_value).collect(),
        },
    }
}

fn render_union_case(
    ty: &Ty,
    variant_name: impl Into<String>,
    value: Option<RenderedValue>,
) -> RenderedValue {
    let mut fields = Vec::new();
    if let Some(value) = value {
        fields.push(("value".to_owned(), value));
    }
    RenderedValue::UnionCase {
        type_name: ty.to_string(),
        variant_name: variant_name.into(),
        fields,
    }
}

fn render_enum_value(ty: &Ty, value: impl Into<String>, raw_value: RenderedValue) -> RenderedValue {
    RenderedValue::EnumValue {
        type_name: ty.to_string(),
        value: value.into(),
        fields: vec![("value".to_owned(), raw_value)],
    }
}

fn map_type_name(k: &Ty, v: &Ty) -> String {
    format!("map<{k}, {v}>")
}

fn render_map_raw(type_name: String, root: Option<&Cell>) -> RenderedValue {
    match root {
        Some(root) => RenderedValue::typed_leaf(
            format!("{type_name} {{raw: cell{{{}}}}}", Boc::encode_hex(root)),
            type_name,
        ),
        None => RenderedValue::Struct {
            type_name,
            fields: vec![],
        },
    }
}

fn parse_map_key_type(ty: &Ty) -> Option<MapScalarType> {
    match ty {
        Ty::Bool => Some(MapScalarType::Bool),
        Ty::Address | Ty::AddressAny => Some(MapScalarType::Address),
        Ty::Int => Some(MapScalarType::Int {
            bits: 257,
            signed: true,
        }),
        Ty::UintN { n: 256 } => Some(MapScalarType::Int {
            bits: 256,
            signed: false,
        }),
        Ty::UintN { n } => u16::try_from(*n).is_ok().then_some(MapScalarType::Int {
            bits: *n as u16,
            signed: false,
        }),
        Ty::IntN { n } => u16::try_from(*n).is_ok().then_some(MapScalarType::Int {
            bits: *n as u16,
            signed: true,
        }),
        _ => None,
    }
}

fn parse_map_value_type(ty: &Ty) -> Option<MapScalarType> {
    match ty {
        Ty::Cell | Ty::CellOf { .. } => Some(MapScalarType::Cell),
        Ty::String => Some(MapScalarType::String),
        Ty::Bool => Some(MapScalarType::Bool),
        Ty::Address | Ty::AddressAny => Some(MapScalarType::Address),
        Ty::Coins | Ty::VaruintN { n: 16 } => Some(MapScalarType::VarInt {
            len_bits: 4,
            signed: false,
        }),
        Ty::Int => Some(MapScalarType::Int {
            bits: 257,
            signed: true,
        }),
        Ty::UintN { n: 256 } => Some(MapScalarType::Int {
            bits: 256,
            signed: false,
        }),
        Ty::UintN { n } => u16::try_from(*n).is_ok().then_some(MapScalarType::Int {
            bits: *n as u16,
            signed: false,
        }),
        Ty::IntN { n } => u16::try_from(*n).is_ok().then_some(MapScalarType::Int {
            bits: *n as u16,
            signed: true,
        }),
        Ty::VarintN { n: 16 } => Some(MapScalarType::VarInt {
            len_bits: 4,
            signed: true,
        }),
        Ty::VarintN { n: 32 } => Some(MapScalarType::VarInt {
            len_bits: 5,
            signed: true,
        }),
        Ty::VaruintN { n: 32 } => Some(MapScalarType::VarInt {
            len_bits: 5,
            signed: false,
        }),
        _ => None,
    }
}

fn format_map_scalar(slice: &mut TyCellSlice<'_>, ty: MapScalarType) -> Result<String, String> {
    match ty {
        MapScalarType::Int { bits, signed } => {
            if !signed && bits == 256 {
                return Ok(format!(
                    "0x{}",
                    slice.load_u256().map_err(|e| e.to_string())?
                ));
            }

            Ok(slice
                .load_bigint(bits, signed)
                .map_err(|e| e.to_string())?
                .to_string())
        }
        MapScalarType::VarInt { len_bits, signed } => Ok(slice
            .load_var_bigint(u16::from(len_bits), signed)
            .map_err(|e| e.to_string())?
            .to_string()),
        MapScalarType::Bool => Ok(slice.load_bit().map_err(|e| e.to_string())?.to_string()),
        MapScalarType::Address => Ok(IntAddr::load_from(slice)
            .map_err(|e| e.to_string())?
            .to_string()),
        MapScalarType::Cell => Ok(render_cell_like(&CellLike::Cell(Boc::encode_hex(
            &slice.load_reference_cloned().map_err(|e| e.to_string())?,
        )))),
        MapScalarType::String => {
            let cell = slice.load_reference_cloned().map_err(|e| e.to_string())?;
            if let Some(string) = Tuple::parse_snake_string(&cell) {
                return Ok(format!("\"{string}\""));
            }
            Ok(render_cell_like(&CellLike::Cell(Boc::encode_hex(&cell))))
        }
    }
}

fn format_map_raw_value(slice: TyCellSlice<'_>) -> Result<String, String> {
    let mut builder = CellBuilder::new();
    builder.store_slice(slice).map_err(|e| e.to_string())?;
    let cell = builder.build().map_err(|e| e.to_string())?;
    Ok(render_cell_like(&CellLike::Cell(Boc::encode_hex(&cell))))
}

fn decode_abi_data(
    symbols: &SourceMap,
    parser: &mut TyCellSlice<'_>,
    ty: &Ty,
) -> Option<ParsedAbiData> {
    let abi = build_compiler_abi(symbols)?;
    decode_abi_data_with_compiler_abi(&abi, parser, ty)
}

fn decode_abi_data_with_compiler_abi(
    abi: &ContractABI,
    parser: &mut TyCellSlice<'_>,
    ty: &Ty,
) -> Option<ParsedAbiData> {
    let data = compiler_abi_serde::decode(parser, abi, ty).ok()?;
    if parser.size_bits() != 0 || parser.size_refs() != 0 {
        // there are remaining data
        return None;
    }
    Some(data)
}

fn render_map_value_with_abi(
    symbols: &SourceMap,
    value_slice: TyCellSlice<'_>,
    value_ty: &Ty,
) -> Option<RenderedValue> {
    let mut parser = value_slice;
    let data = decode_abi_data(symbols, &mut parser, value_ty)?;
    Some(render_abi_data(data, value_ty))
}

fn render_typed_cell(symbols: &SourceMap, ty: &Ty, inner: &Ty, cell: &CellLike) -> RenderedValue {
    let abi = build_compiler_abi(symbols);
    render_typed_cell_with_compiler_abi(abi.as_ref(), ty, inner, cell)
}

fn render_typed_cell_with_compiler_abi(
    abi: Option<&ContractABI>,
    ty: &Ty,
    inner: &Ty,
    cell: &CellLike,
) -> RenderedValue {
    let decoded = if let Some(cell) = decode_cell_like(cell)
        && let Some(abi) = abi
    {
        let mut parser = cell.as_slice_allow_exotic();
        decode_abi_data_with_compiler_abi(abi, &mut parser, inner).map(|data| (inner, data))
    } else {
        None
    };
    render_typed_cell_with_decoded_data(ty, cell, decoded)
}

fn render_typed_cell_with_decoded_data(
    ty: &Ty,
    cell: &CellLike,
    decoded: Option<(&Ty, ParsedAbiData)>,
) -> RenderedValue {
    let value = render_cell_like(cell);
    let (bits, refs, hash) = cell_like_meta(cell);
    let mut fields = render_cell_fields(bits, refs, hash.clone(), Some(value));

    if let Some((inner, data)) = decoded {
        fields.insert(0, ("decoded".to_owned(), render_abi_data(data, inner)));
    }

    RenderedValue::CellOf {
        type_name: ty.to_string(),
        value: render_cell_summary(bits, refs, hash.as_deref()),
        fields,
        raw: Some(cell.clone()),
    }
}

pub(crate) fn render_runtime_storage_with_compiler_abi(
    value: &VmStackValue,
    abi: &ContractABI,
) -> Option<RenderedValue> {
    let VmStackValue::Cell(cell) = value else {
        return None;
    };

    let decoded_cell = decode_cell_like(cell)?;

    // Try deployment storage first and then default one
    for storage_ty in abi
        .storage
        .storage_at_deployment_ty
        .as_ref()
        .into_iter()
        .chain(abi.storage.storage_ty.as_ref())
    {
        let mut parser = decoded_cell.as_slice_allow_exotic();
        if let Some(data) = decode_abi_data_with_compiler_abi(abi, &mut parser, storage_ty) {
            let cell_ty = Ty::CellOf {
                inner: Box::new(storage_ty.clone()),
            };

            return Some(render_typed_cell_with_decoded_data(
                &cell_ty,
                cell,
                Some((storage_ty, data)),
            ));
        }
    }

    None
}

fn decode_abi_data_with_compiler_abi_result(
    abi: &ContractABI,
    parser: &mut TyCellSlice<'_>,
    ty: &Ty,
) -> Result<ParsedAbiData, String> {
    let data = compiler_abi_serde::decode(parser, abi, ty).map_err(|err| err.to_string())?;
    if parser.size_bits() != 0 || parser.size_refs() != 0 {
        return Err(format!(
            "remaining bits/refs after decode: {} bits, {} refs",
            parser.size_bits(),
            parser.size_refs()
        ));
    }
    Ok(data)
}

pub(crate) fn render_cell_like_as_type(
    symbols: &SourceMap,
    value: &RenderedValue,
    ty: &Ty,
) -> Result<RenderedValue, String> {
    let cell_like = value.raw_cell_like().ok_or_else(|| {
        format!(
            "Expression must evaluate to a `cell` or `slice` to decode as `{ty}`, got {}",
            debugger_value_kind(value)
        )
    })?;
    let CellLike::Cell(_) = cell_like else {
        return Err(format!(
            "Expression must evaluate to a `cell` or `slice` to decode as `{ty}`, got {}",
            debugger_value_kind(value)
        ));
    };

    let Ty::CellOf { inner } = ty else {
        return Err(format!(
            "Debugger evaluate only supports casts to Cell<T>, got `{ty}`"
        ));
    };

    let abi = build_compiler_abi(symbols)
        .ok_or_else(|| "Failed to build debug ABI from SourceMap declarations".to_owned())?;
    let decoded_cell = decode_cell_like(cell_like).ok_or_else(|| {
        format!(
            "Failed to materialize {} as a TVM cell while decoding `{ty}`",
            debugger_value_kind(value)
        )
    })?;
    let mut parser = decoded_cell.as_slice_allow_exotic();
    let data = decode_abi_data_with_compiler_abi_result(&abi, &mut parser, inner.as_ref())?;

    Ok(render_typed_cell_with_decoded_data(
        ty,
        cell_like,
        Some((inner.as_ref(), data)),
    ))
}

fn debugger_value_kind(value: &RenderedValue) -> String {
    match value {
        RenderedValue::Leaf {
            value,
            type_field: Some(type_field),
        } => format!("`{type_field}` (`{value}`)"),
        RenderedValue::Leaf { value, .. } => format!("`{value}`"),
        RenderedValue::CellLike { type_name, .. }
        | RenderedValue::CellOf { type_name, .. }
        | RenderedValue::EnumValue { type_name, .. }
        | RenderedValue::UnionCase { type_name, .. }
        | RenderedValue::Struct { type_name, .. }
        | RenderedValue::Address { type_name, .. }
        | RenderedValue::Tensor { type_name, .. }
        | RenderedValue::ArrayOf { type_name, .. }
        | RenderedValue::LazyUnresolved { type_name } => format!("`{type_name}`"),
        RenderedValue::LastSeen { inner } => debugger_value_kind(inner),
        RenderedValue::OptimizedOut => "<optimized out>".to_owned(),
        RenderedValue::LazyNotYetLoaded { preview } => debugger_value_kind(preview),
        RenderedValue::LazyCantParseSlice => "<not loaded>".to_owned(),
    }
}

fn render_abi_data(data: ParsedAbiData, ty: &Ty) -> RenderedValue {
    match data {
        ParsedAbiData::Object(object) if matches!(ty, Ty::EnumRef { .. }) => {
            let mut fields = object.fields.into_iter();
            match fields.next() {
                Some(field) => render_enum_value(
                    ty,
                    object.name,
                    render_abi_data(field.value, &field.field_type),
                ),
                None => typed_leaf_for_ty(ty, object.name),
            }
        }
        ParsedAbiData::Object(object) => RenderedValue::Struct {
            type_name: object.name,
            fields: object
                .fields
                .into_iter()
                .map(|field| (field.name, render_abi_data(field.value, &field.field_type)))
                .collect(),
        },
        ParsedAbiData::Array(items) => RenderedValue::ArrayOf {
            type_name: ty.to_string(),
            items: render_abi_array_items(items, ty),
        },
        ParsedAbiData::Map(entries) => render_abi_map(entries, ty),
        ParsedAbiData::Address(IntAddr::Std(addr)) => {
            render_std_address(ty.to_string(), addr.to_string(), &addr)
        }
        ParsedAbiData::Address(addr) => typed_leaf_for_ty(ty, addr.to_string()),
        ParsedAbiData::ExtAddress(addr) => typed_leaf_for_ty(ty, addr.to_string()),
        ParsedAbiData::Cell(cell) | ParsedAbiData::RemainingBitsAndRefs(cell) => typed_leaf_for_ty(
            ty,
            render_cell_like(&CellLike::Cell(Boc::encode_hex(&cell))),
        ),
        ParsedAbiData::Bits((bytes, bit_len)) => {
            typed_leaf_for_ty(ty, format_abi_bits(&bytes, bit_len))
        }
        ParsedAbiData::Null => typed_leaf_for_ty(ty, "null"),
        ParsedAbiData::Number(value) => typed_leaf_for_ty(ty, value.to_string()),
        ParsedAbiData::Bool(value) => typed_leaf_for_ty(ty, value.to_string()),
        ParsedAbiData::String(value) => typed_leaf_for_ty(ty, format!("\"{value}\"")),
        ParsedAbiData::Symbol(value) => typed_leaf_for_ty(ty, value),
    }
}

fn render_abi_array_items(items: Vec<ParsedAbiData>, ty: &Ty) -> Vec<RenderedValue> {
    match ty {
        Ty::ArrayOf { inner } => items
            .into_iter()
            .map(|item| render_abi_data(item, inner.as_ref()))
            .collect(),
        Ty::Tensor { items: item_types } | Ty::ShapedTuple { items: item_types } => items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let item_ty = item_types.get(index).cloned().unwrap_or(Ty::Unknown);
                render_abi_data(item, &item_ty)
            })
            .collect(),
        _ => items
            .into_iter()
            .map(|item| render_abi_data(item, &Ty::Unknown))
            .collect(),
    }
}

fn render_abi_map(entries: Vec<(ParsedAbiData, ParsedAbiData)>, ty: &Ty) -> RenderedValue {
    let type_name = ty.to_string();
    let (key_ty, value_ty) = match ty {
        Ty::MapKV { k, v } => (k.as_ref().clone(), v.as_ref().clone()),
        _ => (Ty::Unknown, Ty::Unknown),
    };

    RenderedValue::Struct {
        type_name,
        fields: entries
            .into_iter()
            .map(|(key, value)| {
                (
                    format_abi_map_key(&key, &key_ty),
                    render_abi_data(value, &value_ty),
                )
            })
            .collect(),
    }
}

fn format_abi_map_key(data: &ParsedAbiData, key_ty: &Ty) -> String {
    match data {
        ParsedAbiData::Null => "null".to_owned(),
        ParsedAbiData::Number(value) => value.to_string(),
        ParsedAbiData::Bool(value) => value.to_string(),
        ParsedAbiData::String(value) => format!("\"{value}\""),
        ParsedAbiData::Symbol(value) => value.clone(),
        ParsedAbiData::Object(object) if matches!(key_ty, Ty::EnumRef { .. }) => {
            object.name.clone()
        }
        ParsedAbiData::Address(value) => value.to_string(),
        ParsedAbiData::ExtAddress(value) => value.to_string(),
        ParsedAbiData::Cell(value) | ParsedAbiData::RemainingBitsAndRefs(value) => {
            render_cell_like(&CellLike::Cell(Boc::encode_hex(value)))
        }
        ParsedAbiData::Bits((bytes, bit_len)) => format_abi_bits(bytes, *bit_len),
        ParsedAbiData::Object(_) | ParsedAbiData::Array(_) | ParsedAbiData::Map(_) => {
            typed_leaf_for_ty(key_ty, "<key>").dap_value()
        }
    }
}

fn format_abi_bits(bytes: &[u8], bit_len: usize) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(hex, "{byte:02x}").ok();
    }

    if bit_len.is_multiple_of(8) {
        format!("0x{hex}")
    } else {
        format!("0x{hex} ({bit_len} bits)")
    }
}

fn render_map_value(
    symbols: &SourceMap,
    value_slice: TyCellSlice<'_>,
    value_ty: &Ty,
) -> RenderedValue {
    let scalar_type = parse_map_value_type(value_ty);
    let allow_raw_value_fallback =
        scalar_type.is_none() && !matches!(value_ty, Ty::Nullable { .. } | Ty::MapKV { .. });

    let mut value_slice = value_slice;
    if let Some(scalar_type) = scalar_type {
        return match format_map_scalar(&mut value_slice, scalar_type) {
            Ok(value) => typed_leaf_for_ty(value_ty, value),
            Err(err) => typed_leaf_for_ty(value_ty, format!("<value: {err}>")),
        };
    }

    if let Some(value) = render_map_value_with_abi(symbols, value_slice, value_ty) {
        return value;
    }

    if allow_raw_value_fallback {
        return match format_map_raw_value(value_slice) {
            Ok(value) => typed_leaf_for_ty(value_ty, value),
            Err(err) => typed_leaf_for_ty(value_ty, format!("<value: {err}>")),
        };
    }

    typed_leaf_for_ty(value_ty, "<value>")
}

fn render_map_dict(
    symbols: &SourceMap,
    root: Option<Cell>,
    key_ty: &Ty,
    value_ty: &Ty,
) -> RenderedValue {
    let type_name = map_type_name(key_ty, value_ty);

    let Some(key_type) = parse_map_key_type(key_ty) else {
        return render_map_raw(type_name, root.as_ref());
    };

    let mut fields = Vec::new();
    for entry in dict::RawIter::new(&root, key_type.bit_len()) {
        let Ok((key_data, value_slice)) = entry else {
            return RenderedValue::typed_leaf(format!("{type_name} {{...}}"), type_name);
        };

        let key = {
            let mut key_slice = key_data.as_data_slice();
            format_map_scalar(&mut key_slice, key_type).unwrap_or_else(|_| "<key>".to_string())
        };
        let value = render_map_value(symbols, value_slice, value_ty);
        fields.push((key, value));
    }

    RenderedValue::Struct { type_name, fields }
}

/// Convert a range of bits from nibbles to a hex string.
/// Appends a completion tag `_` when the bit count is not a multiple of 4.
fn bits_to_hex(nibbles: &[u8], start: usize, end: usize) -> String {
    let bit_count = end - start;
    let full_nibbles = bit_count / 4;
    let remaining_bits = bit_count % 4;
    let mut hex = String::with_capacity(full_nibbles + 2);

    for i in 0..full_nibbles {
        let n = get_bits_u8(nibbles, start + i * 4, 4);
        write!(hex, "{n:x}").ok();
    }

    if remaining_bits > 0 {
        let mut last: u8 = 0;
        for i in 0..remaining_bits {
            last = (last << 1) | get_bit(nibbles, start + full_nibbles * 4 + i);
        }
        last = (last << 1) | 1;
        last <<= 4 - remaining_bits - 1;
        write!(hex, "{last:x}_").ok();
    }

    hex
}

fn hex_to_nibbles(hex: &str) -> Vec<u8> {
    hex.chars()
        .filter_map(|c| c.to_digit(16).map(|d| d as u8))
        .collect()
}

fn refs_suffix(ref_count: usize) -> String {
    match ref_count {
        0 => String::new(),
        1 => " + 1 ref".to_string(),
        n => format!(" + {n} refs"),
    }
}

/// Render a `CellSlice` as `slice{HEX}`, extracting only the bits in `start..end`.
/// Appends `+ N refs` when the slice carries cell references.
fn render_slice(cs: &CellSlice) -> String {
    let ref_count = cs
        .refs
        .as_ref()
        .and_then(|(s, e)| Some(e.parse::<usize>().ok()? - s.parse::<usize>().ok()?))
        .unwrap_or(0);
    let r_suf = refs_suffix(ref_count);

    if let Some((start_s, end_s)) = &cs.bits
        && let (Ok(start), Ok(end)) = (start_s.parse::<usize>(), end_s.parse::<usize>())
    {
        let data_hex = if cs.value.len() > 4 {
            &cs.value[4..]
        } else {
            &cs.value
        };
        let nibbles = hex_to_nibbles(data_hex);
        if end <= nibbles.len() * 4 {
            let hex = bits_to_hex(&nibbles, start, end);
            return format!("slice{{{hex}}}{r_suf}");
        }
    }
    let data_hex = if cs.value.len() > 4 {
        &cs.value[4..]
    } else {
        &cs.value
    };
    format!("slice{{{data_hex}}}{r_suf}")
}

/// Render a Builder (BC{hex}) as `builder{DATA_HEX}`, stripping descriptor bytes.
/// d1 encodes ref count (`d1 & 7`), d2 encodes data length.
/// When d2 is odd, data has a completion tag; we strip it and render with `_` if needed.
fn render_builder(hex: &str) -> String {
    if hex.len() < 4 {
        return format!("builder{{{hex}}}");
    }
    let d1 = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let d2 = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let ref_count = (d1 & 7) as usize;
    let data_hex = &hex[4..];
    let r_suf = refs_suffix(ref_count);

    if d2 == 0 {
        return format!("builder{{}}{r_suf}");
    }

    let rendered = if d2.is_multiple_of(2) {
        data_hex.to_lowercase()
    } else {
        let nibbles = hex_to_nibbles(data_hex);
        let raw_bits = nibbles.len() * 4;
        let actual_bits = (0..raw_bits)
            .rev()
            .find(|&pos| get_bit(&nibbles, pos) == 1)
            .unwrap_or(0);
        bits_to_hex(&nibbles, 0, actual_bits)
    };

    format!("builder{{{rendered}}}{r_suf}")
}

/// Interpret a TVM tuple as a lisp-list and return the list of element references.
/// On the TVM stack a lisp list is nested: `[a [b [c null]]]`. But in the execution
/// log Fift often (not always!) prints it flattened: one tuple with all elements, e.g. `(a b c)`.
fn flatten_lisp_list(items: &[VmStackValue]) -> Vec<&VmStackValue> {
    if items.len() == 2 {
        match &items[1] {
            VmStackValue::Null => vec![&items[0]],
            VmStackValue::Tuple(tail) if tail.len() == 2 => match &tail[1] {
                VmStackValue::Null => {
                    let mut result = vec![&items[0]];
                    result.push(&tail[0]);
                    result
                }
                VmStackValue::Tuple(_) => {
                    let mut result = vec![&items[0]];
                    result.extend(flatten_lisp_list(tail));
                    result
                }
                _ => items.iter().collect(),
            },
            _ => items.iter().collect(),
        }
    } else {
        items.iter().collect()
    }
}

fn typed_leaf_for_ty(ty: &Ty, value: impl Into<String>) -> RenderedValue {
    RenderedValue::typed_leaf(value, ty.to_string())
}

// ---------------------------------------------------------------------------
// debug_format — recursive type-aware renderer (uses StackReader cursor)
// ---------------------------------------------------------------------------

// Read `ty` from a stack and return formatted representation.
// The returned RenderedValue can be transformed to a plain string, like "Point { x: 10, y: 20 }"
// or to an expandable DAP tree view (for VS Code debugger).
fn debug_format(
    symbols: &SourceMap,
    r: &mut StackReader,
    ty: &Ty,
    un_tuple_if_w: bool,
) -> RenderedValue {
    let width = calc_width_on_stack(symbols, ty);

    if width > 0 && r.peek_n_all_optimized_out(width) {
        r.skip(width);
        return RenderedValue::OptimizedOut;
    }
    if width > 0 && r.peek_n_all_last_seen(width) {
        let as_live: Vec<SlotValue> = (0..width)
            .map(|i| match r.peek_at(i) {
                SlotValue::LastSeen(v) => SlotValue::Live(v),
                other => other,
            })
            .collect();
        r.skip(width);
        let mut sub = StackReader::new(&as_live);
        let inner = debug_format(symbols, &mut sub, ty, un_tuple_if_w);
        return RenderedValue::LastSeen {
            inner: Box::new(inner),
        };
    }

    if un_tuple_if_w
        && width != 1
        && let SlotValue::Live(VmStackValue::Tuple(t)) = r.read_slot()
    {
        let as_live: Vec<SlotValue> = t.iter().map(SlotValue::Live).collect();
        let mut sub = StackReader::new(&as_live);
        return debug_format(symbols, &mut sub, ty, false);
    }

    match ty {
        Ty::Int
        | Ty::IntN { .. }
        | Ty::UintN { .. }
        | Ty::VarintN { .. }
        | Ty::VaruintN { .. }
        | Ty::Coins => match r.read_slot() {
            SlotValue::Live(VmStackValue::Integer(s)) => typed_leaf_for_ty(ty, s.clone()),
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM int"),
        },

        Ty::Bool => match r.read_slot() {
            SlotValue::Live(VmStackValue::Integer(s)) => {
                if s == "0" {
                    typed_leaf_for_ty(ty, "false")
                } else {
                    typed_leaf_for_ty(ty, "true")
                }
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM int"),
        },

        Ty::Cell => match r.read_slot() {
            SlotValue::Live(VmStackValue::Cell(cell)) => {
                let (bits, refs, hash) = cell_like_meta(cell);
                render_openable_cell_like(
                    ty,
                    render_cell_like(cell),
                    bits,
                    refs,
                    hash,
                    Some(cell.clone()),
                )
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM cell"),
        },

        Ty::CellOf { inner } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Cell(cell)) => {
                render_typed_cell(symbols, ty, inner, cell)
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM cell"),
        },

        Ty::String => match r.read_slot() {
            SlotValue::Live(VmStackValue::String(s)) => typed_leaf_for_ty(ty, format!("\"{s}\"")),
            SlotValue::Live(VmStackValue::Cell(cell)) => typed_leaf_for_ty(
                ty,
                try_parse_string_cell_like(cell)
                    .map_or_else(|| render_cell_like(cell), |string| format!("\"{string}\"")),
            ),
            SlotValue::Live(VmStackValue::CellSlice(cs)) => typed_leaf_for_ty(
                ty,
                try_parse_string_slice(cs)
                    .map_or_else(|| render_slice(cs), |string| format!("\"{string}\"")),
            ),
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM cell"),
        },

        Ty::Builder => match r.read_slot() {
            SlotValue::Live(VmStackValue::Builder(b)) => {
                let cell = CellLike::Builder(b.clone());
                let (bits, refs, hash) = cell_like_meta(&cell);
                render_openable_cell_like(ty, render_builder(b), bits, refs, hash, Some(cell))
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM builder"),
        },

        Ty::Slice | Ty::Remaining | Ty::BitsN { .. } => match r.read_slot() {
            SlotValue::Live(VmStackValue::CellSlice(cs)) => {
                let (bits, refs, hash) = slice_meta(cs);
                render_openable_cell_like(
                    ty,
                    render_slice(cs),
                    bits,
                    refs,
                    hash,
                    slice_as_cell_like(cs),
                )
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM slice"),
        },

        Ty::ArrayOf { inner } => {
            // array len N => N sub-items => N calls to inner debug_format
            match r.read_slot() {
                SlotValue::Live(VmStackValue::Tuple(t)) => {
                    let as_live: Vec<SlotValue> = t.iter().map(SlotValue::Live).collect();
                    let mut sub = StackReader::new(&as_live);
                    let items: Vec<RenderedValue> = as_live
                        .iter()
                        .map(|_| debug_format(symbols, &mut sub, inner, true))
                        .collect();
                    RenderedValue::ArrayOf {
                        type_name: ty.to_string(),
                        items,
                    }
                }
                SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
                _ => typed_leaf_for_ty(ty, "not a TVM tuple"),
            }
        }

        Ty::LispListOf { inner } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Tuple(t)) => {
                let elements = flatten_lisp_list(t);
                let as_live: Vec<SlotValue> = elements.iter().map(|v| SlotValue::Live(v)).collect();
                let n = as_live.len();
                let mut sub = StackReader::new(&as_live);
                let items: Vec<RenderedValue> = (0..n)
                    .map(|_| debug_format(symbols, &mut sub, inner, true))
                    .collect();
                RenderedValue::ArrayOf {
                    type_name: ty.to_string(),
                    items,
                }
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::ArrayOf {
                type_name: ty.to_string(),
                items: vec![],
            },
            _ => typed_leaf_for_ty(ty, "not a TVM tuple"),
        },

        Ty::Address | Ty::AddressOpt | Ty::AddressExt | Ty::AddressAny => match r.read_slot() {
            SlotValue::Live(VmStackValue::CellSlice(cs)) => match try_parse_address(cs) {
                Some(raw) => match raw.parse::<StdAddr>() {
                    Ok(addr) => render_std_address(ty.to_string(), addr.to_string(), &addr),
                    Err(_) => typed_leaf_for_ty(ty, raw),
                },
                None => typed_leaf_for_ty(ty, render_slice(cs)),
            },
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM slice"),
        },

        Ty::MapKV { k, v } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Struct {
                type_name: map_type_name(k, v),
                fields: vec![],
            },
            SlotValue::Live(VmStackValue::Cell(cell)) => {
                if let Some(root) = decode_cell_like(cell) {
                    render_map_dict(symbols, Some(root), k, v)
                } else {
                    typed_leaf_for_ty(ty, "not a TVM cell")
                }
            }
            _ => typed_leaf_for_ty(ty, "not a TVM cell"),
        },

        Ty::NullLiteral => match r.read_slot() {
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM null"),
        },

        Ty::Void => typed_leaf_for_ty(ty, "(void)"),

        Ty::Callable => match r.read_slot() {
            SlotValue::Live(VmStackValue::Continuation(_)) => typed_leaf_for_ty(ty, "continuation"),
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM continuation"),
        },
        Ty::Unknown => match r.read_slot() {
            SlotValue::Live(any) => RenderedValue::leaf(any.to_string()),
            _ => RenderedValue::leaf("unreachable"),
        },

        Ty::Nullable {
            inner, stack_width, ..
        } => {
            if let Some(sw) = stack_width {
                // read wide nullable: [null, null, ... 0] or [smth, smth, ... type_id]
                let nullable_slots = r.read_n_slots(*sw);
                let tag_slot = &nullable_slots[sw - 1];
                match tag_slot {
                    SlotValue::Live(VmStackValue::Integer(type_id))
                    | SlotValue::LastSeen(VmStackValue::Integer(type_id)) => {
                        if type_id == "0" {
                            typed_leaf_for_ty(ty, "null")
                        } else {
                            let mut sub = StackReader::new(&nullable_slots[..sw - 1]);
                            debug_format(symbols, &mut sub, inner, false)
                        }
                    }
                    SlotValue::OptimizedOut => {
                        let mut sub = StackReader::new(&nullable_slots[..sw - 1]);
                        debug_format(symbols, &mut sub, inner, false)
                    }
                    _ => typed_leaf_for_ty(ty, "corrupted stack for nullable"),
                }
            } else {
                // read a primitive one-slot nullable: either TVM null or a value of type inner
                match r.peek_slot() {
                    SlotValue::Live(VmStackValue::Null)
                    | SlotValue::LastSeen(VmStackValue::Null) => {
                        r.read_slot();
                        typed_leaf_for_ty(ty, "null")
                    }
                    _ => debug_format(symbols, r, inner, false),
                }
            }
        }

        Ty::StructRef {
            struct_name,
            type_args,
        } => {
            let struct_ref = symbols.get_struct(struct_name);
            let struct_name = match type_args {
                None => struct_name.clone(),
                Some(type_args) => format!(
                    "{}<{}>",
                    struct_name,
                    type_args
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            let mut fields: Vec<(String, RenderedValue)> = Vec::new();
            for f in &struct_ref.fields {
                let field_val = match type_args {
                    Some(type_args) => {
                        let f_ty = instantiate_generics(
                            &f.ty,
                            struct_ref.type_params.as_deref().unwrap_or(&[]),
                            type_args,
                        );
                        debug_format(symbols, r, &f_ty, false)
                    }
                    None => debug_format(symbols, r, &f.ty, false),
                };
                fields.push((f.name.clone(), field_val));
            }
            RenderedValue::Struct {
                type_name: struct_name,
                fields,
            }
        }

        Ty::AliasRef {
            alias_name,
            type_args,
        } => {
            let alias_ref = symbols.get_alias(alias_name);
            match type_args {
                Some(type_args) => {
                    let target_ty = instantiate_generics(
                        &alias_ref.target_ty,
                        alias_ref.type_params.as_deref().unwrap_or(&[]),
                        type_args,
                    );
                    debug_format(symbols, r, &target_ty, false)
                }
                None => debug_format(symbols, r, &alias_ref.target_ty, false),
            }
        }

        Ty::EnumRef { enum_name } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Integer(s)) => {
                let enum_ref = symbols.get_enum(enum_name);
                let text = enum_ref.members.iter().find(|m| &m.value == s).map_or_else(
                    || format!("{}({})", enum_ref.name, s),
                    |m| format!("{}.{}", enum_ref.name, m.name),
                );
                let slot = VmStackValue::Integer(s.clone());
                let slots = [SlotValue::Live(&slot)];
                let mut sub = StackReader::new(&slots);
                let raw_value = debug_format(symbols, &mut sub, &enum_ref.encoded_as, false);
                render_enum_value(ty, text, raw_value)
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM int"),
        },

        Ty::Tensor { items } => {
            let items: Vec<RenderedValue> = items
                .iter()
                .map(|item| debug_format(symbols, r, item, false))
                .collect();
            RenderedValue::Tensor {
                type_name: ty.to_string(),
                items,
            }
        }

        Ty::ShapedTuple { items } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Tuple(t)) => {
                let as_live: Vec<SlotValue> = t.iter().map(SlotValue::Live).collect();
                let mut sub = StackReader::new(&as_live);
                let items: Vec<RenderedValue> = items
                    .iter()
                    .map(|item| debug_format(symbols, &mut sub, item, true))
                    .collect();
                RenderedValue::ArrayOf {
                    type_name: ty.to_string(),
                    items,
                }
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(ty, "null"),
            _ => typed_leaf_for_ty(ty, "not a TVM tuple"),
        },

        Ty::Union {
            variants,
            stack_width: Some(stack_width),
        } => {
            // read tagged union: [smth, smth, ... type_id]
            let stack_width = *stack_width;
            let union_slots = r.read_n_slots(stack_width);
            let tag_slot = &union_slots[stack_width - 1];
            match tag_slot {
                SlotValue::Live(VmStackValue::Integer(type_id))
                | SlotValue::LastSeen(VmStackValue::Integer(type_id)) => {
                    let type_id: usize = type_id.parse().unwrap_or(100500);
                    if let Some(variant) =
                        variants.iter().find(|v| v.stack_type_id == Some(type_id))
                    {
                        let variant_width = variant.stack_width.unwrap_or(0);
                        let Some(variant_start) = stack_width.checked_sub(1 + variant_width) else {
                            return typed_leaf_for_ty(ty, "corrupted stack for union");
                        };
                        let value = if variant_width == 0 {
                            None
                        } else {
                            let mut sub =
                                StackReader::new(&union_slots[variant_start..stack_width - 1]);
                            Some(debug_format(symbols, &mut sub, &variant.variant_ty, false))
                        };
                        render_union_case(ty, variant.variant_ty.to_string(), value)
                    } else {
                        // corrupted stack, type_id on a stack mismatches all variants
                        typed_leaf_for_ty(ty, "union with unknown variant")
                    }
                }
                SlotValue::OptimizedOut => {
                    // this should not happen in practice, because if UTag for a union was erased during compilation,
                    // a union was definitely smart cast, and its type is narrowed, not Ty::Union
                    typed_leaf_for_ty(ty, "union with unknown variant")
                }
                _ => typed_leaf_for_ty(ty, "corrupted stack for union"),
            }
        }

        Ty::Union { .. } => {
            r.read_n_slots(width);
            typed_leaf_for_ty(ty, "union with unresolved layout")
        }

        Ty::GenericT { name_t } => {
            RenderedValue::typed_leaf(format!("unexpected genericT={name_t}"), name_t.clone())
        }
    }
}

pub(crate) fn debug_print_from_stack(
    symbols: &SourceMap,
    slots: &[SlotValue],
    ty: &Ty,
) -> RenderedValue {
    let mut r = StackReader::new(slots);
    debug_format(symbols, &mut r, ty, false)
}

fn render_lazy_struct_fields(
    symbols: &SourceMap,
    struct_ref: &AbiStruct,
    type_args: Option<&[Ty]>,
    slot_values: &[SlotValue],
    ir_slots: &[usize],
    last_seen: &HashMap<usize, VmStackValue>,
    lazy_cell_abi: Option<(&Cell, &ContractABI)>,
) -> Vec<(String, RenderedValue)> {
    let mut lazy_s = lazy_cell_abi.as_ref().map(|(cell, _)| {
        let mut s = cell.as_slice_allow_exotic();
        if let Some(ref prefix) = struct_ref.prefix {
            let _ = s.skip_first(prefix.prefix_len as u16, 0);
        }
        s
    });
    let abi = lazy_cell_abi.as_ref().map(|(_, a)| *a);

    let mut fields = Vec::new();
    let mut offset = 0;
    for f in &struct_ref.fields {
        let f_ty = match type_args {
            Some(type_args) => instantiate_generics(
                &f.ty,
                struct_ref.type_params.as_deref().unwrap_or(&[]),
                type_args,
            ),
            None => f.ty.clone(),
        };
        let f_width = calc_width_on_stack(symbols, &f_ty);
        let field_ir_slots = &ir_slots[offset..offset + f_width];
        let field_ever_seen = field_ir_slots.iter().any(|s| last_seen.contains_key(s));

        let preview = match (lazy_s.as_mut(), abi) {
            (Some(lazy_s), Some(abi)) => {
                if let Ok(parsed) = compiler_abi_serde::decode(lazy_s, abi, &f_ty) {
                    Some(render_abi_data(parsed, &f_ty))
                } else {
                    None
                }
            }
            _ => None,
        };

        let field_val = if field_ever_seen {
            let field_slot_values = &slot_values[offset..offset + f_width];
            let mut r = StackReader::new(field_slot_values);
            debug_format(symbols, &mut r, &f_ty, false)
        } else {
            match preview {
                None => RenderedValue::LazyCantParseSlice,
                Some(preview) => RenderedValue::LazyNotYetLoaded {
                    preview: Box::new(preview),
                },
            }
        };
        fields.push((f.name.clone(), field_val));
        offset += f_width;
    }
    fields
}

// ---------------------------------------------------------------------------
// debug_format_lazy — renders a lazy variable, showing <not loaded> for
// fields whose ir_slots have never been observed on the stack.
// `last_seen` keys serve as the set of IR indices that have appeared in at
// least one MARK_STACK during replay so far.
// ---------------------------------------------------------------------------

pub(crate) fn debug_format_lazy(
    symbols: &SourceMap,
    slot_values: &[SlotValue],
    ir_slots: &[usize],
    ty: &Ty,
    last_seen: &HashMap<usize, VmStackValue>,
    lazy_original_slice: &VmStackValue,
) -> RenderedValue {
    match ty {
        Ty::Union { .. } => {
            // when a lazy var is still Ty::Union, DEBUG_SMART_CAST not appeared, it's still unresolved
            let type_name = format!("{ty}");
            RenderedValue::LazyUnresolved { type_name }
        }

        Ty::AliasRef {
            alias_name,
            type_args,
        } => {
            let alias_ref = symbols.get_alias(alias_name);
            let resolved = match type_args {
                Some(type_args) => instantiate_generics(
                    &alias_ref.target_ty,
                    alias_ref.type_params.as_deref().unwrap_or(&[]),
                    type_args,
                ),
                None => alias_ref.target_ty.clone(),
            };
            if matches!(&resolved, Ty::Union { .. }) {
                return RenderedValue::LazyUnresolved {
                    type_name: alias_name.clone(),
                };
            }
            debug_format_lazy(
                symbols,
                slot_values,
                ir_slots,
                &resolved,
                last_seen,
                lazy_original_slice,
            )
        }

        Ty::StructRef {
            struct_name,
            type_args,
        } => {
            let struct_ref = symbols.get_struct(struct_name);
            let slice_as_cell = match lazy_original_slice {
                VmStackValue::CellSlice(cs) => exact_slice_cell(cs),
                _ => None,
            };

            let ty_args = type_args.as_deref();
            let fields = match &slice_as_cell {
                Some(cell) => {
                    let tmp_abi = build_compiler_abi(symbols).expect("always not None");
                    render_lazy_struct_fields(
                        symbols,
                        struct_ref,
                        ty_args,
                        slot_values,
                        ir_slots,
                        last_seen,
                        Some((cell, &tmp_abi)),
                    )
                }
                None => render_lazy_struct_fields(
                    symbols,
                    struct_ref,
                    ty_args,
                    slot_values,
                    ir_slots,
                    last_seen,
                    None,
                ),
            };

            RenderedValue::Struct {
                type_name: format!("{struct_name} (lazy)"),
                fields,
            }
        }

        _ => {
            let mut r = StackReader::new(slot_values);
            debug_format(symbols, &mut r, ty, false)
        }
    }
}

pub(crate) fn render_runtime_out_actions(
    value: &VmStackValue,
    abi: Option<&ContractABI>,
) -> Option<RenderedValue> {
    let VmStackValue::Cell(cell) = value else {
        return None;
    };
    let root = decode_cell_like(cell)?;
    let actions = decode_out_actions(&root)?;

    let (bits, refs, hash) = cell_like_meta(cell);
    let mut fields = render_cell_fields(bits, refs, hash.clone(), Some(render_cell_like(cell)));
    fields.insert(
        0,
        (
            "decoded".to_owned(),
            RenderedValue::ArrayOf {
                type_name: "array<OutAction>".to_owned(),
                items: actions
                    .iter()
                    .map(|action| render_out_action(action, abi))
                    .collect(),
            },
        ),
    );

    Some(RenderedValue::CellOf {
        type_name: "Cell<array<OutAction>>".to_owned(),
        value: render_cell_summary(bits, refs, hash.as_deref()),
        fields,
        raw: Some(cell.clone()),
    })
}

fn decode_out_actions(root: &Cell) -> Option<Vec<OutAction>> {
    let slice = root.as_slice().ok()?;
    let mut actions: Vec<OutAction> = OutActionsRevIter::new(slice)
        .collect::<Result<_, _>>()
        .ok()?;
    actions.reverse();
    Some(actions)
}

fn render_out_action(action: &OutAction, abi: Option<&ContractABI>) -> RenderedValue {
    match action {
        OutAction::SendMsg { mode, out_msg } => {
            let mode_text = format_send_msg_flags(*mode);
            let mut fields = vec![
                (
                    "mode".to_owned(),
                    RenderedValue::typed_leaf(mode_text.clone(), "int"),
                ),
                ("out_msg_raw".to_owned(), render_cell_value(out_msg.inner())),
            ];
            match out_msg.load() {
                Ok(message) => {
                    let body_meta = resolve_send_message_body_meta(&message, abi);
                    fields.insert(
                        1,
                        (
                            "out_msg".to_owned(),
                            render_owned_relaxed_message(&message, body_meta.as_ref()),
                        ),
                    );
                    RenderedValue::UnionCase {
                        type_name: "OutAction".to_owned(),
                        variant_name: format_send_message_summary(
                            &message.info,
                            mode_text,
                            body_meta.as_ref(),
                        ),
                        fields,
                    }
                }
                Err(_) => RenderedValue::UnionCase {
                    type_name: "OutAction".to_owned(),
                    variant_name: format!("SendMsg {mode_text}"),
                    fields,
                },
            }
        }
        OutAction::SetCode { new_code } => RenderedValue::UnionCase {
            type_name: "OutAction".to_owned(),
            variant_name: "SetCode".to_owned(),
            fields: vec![("new_code".to_owned(), render_cell_value(new_code))],
        },
        OutAction::ReserveCurrency { mode, value } => {
            let mode_text = format_reserve_currency_flags(*mode);
            RenderedValue::UnionCase {
                type_name: "OutAction".to_owned(),
                variant_name: format!(
                    "ReserveCurrency {} with {mode_text}",
                    format_currency_collection(value)
                ),
                fields: vec![
                    (
                        "mode".to_owned(),
                        RenderedValue::typed_leaf(mode_text, "int"),
                    ),
                    (
                        "value".to_owned(),
                        RenderedValue::typed_leaf(
                            format_currency_collection(value),
                            "coins | (coins, ExtraCurrenciesMap)",
                        ),
                    ),
                ],
            }
        }
        OutAction::ChangeLibrary { mode, lib } => {
            let mode_text = format_change_library_mode(*mode);
            RenderedValue::UnionCase {
                type_name: "OutAction".to_owned(),
                variant_name: format!("ChangeLibrary {mode_text}"),
                fields: vec![
                    (
                        "mode".to_owned(),
                        RenderedValue::typed_leaf(mode_text, "int"),
                    ),
                    ("lib".to_owned(), render_lib_ref(lib)),
                ],
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedSendMessageBodyMeta {
    opcode: Option<u32>,
    body_type_name: Option<String>,
    body_decoded: Option<RenderedValue>,
}

fn render_lib_ref(lib: &LibRef) -> RenderedValue {
    match lib {
        LibRef::Hash(hash) => RenderedValue::typed_leaf(
            format!("0x{}", hash.to_string().to_ascii_lowercase()),
            "uint256",
        ),
        LibRef::Cell(cell) => render_cell_value(cell),
    }
}

fn render_owned_relaxed_message(
    message: &OwnedRelaxedMessage,
    body_meta: Option<&ResolvedSendMessageBodyMeta>,
) -> RenderedValue {
    let body = render_message_body(&message.body, body_meta);
    let mut fields = vec![("info".to_owned(), render_relaxed_msg_info(&message.info))];
    if let Some(body_meta) = body_meta
        && let Some(opcode) = body_meta.opcode
    {
        fields.push((
            "opcode".to_owned(),
            RenderedValue::typed_leaf(format!("0x{opcode:08x}"), "uint32"),
        ));
    }
    fields.push((
        "init".to_owned(),
        match &message.init {
            Some(init) => render_state_init(init),
            None => RenderedValue::typed_leaf("null", "StateInit?"),
        },
    ));
    fields.push(("body".to_owned(), body));
    RenderedValue::Struct {
        type_name: "OutMessage".to_owned(),
        fields,
    }
}

fn render_message_body(
    parts: &(tycho_types::cell::CellSliceRange, Cell),
    body_meta: Option<&ResolvedSendMessageBodyMeta>,
) -> RenderedValue {
    let Some(body_meta) = body_meta else {
        return render_cell_slice_parts(parts);
    };
    let Some(body_type_name) = body_meta.body_type_name.as_ref() else {
        return render_cell_slice_parts(parts);
    };
    let Some(body_decoded) = body_meta.body_decoded.as_ref() else {
        return render_cell_slice_parts(parts);
    };
    let Some(cell) = cell_from_slice_parts(parts) else {
        return render_cell_slice_parts(parts);
    };

    let cell_like = CellLike::Cell(Boc::encode_hex(&cell));
    let (bits, refs, hash) = cell_like_meta(&cell_like);
    let mut fields =
        render_cell_fields(bits, refs, hash.clone(), Some(render_cell_like(&cell_like)));
    fields.insert(0, ("decoded".to_owned(), body_decoded.clone()));

    RenderedValue::CellOf {
        type_name: format!("Cell<{body_type_name}>"),
        value: render_cell_summary(bits, refs, hash.as_deref()),
        fields,
        raw: Some(cell_like),
    }
}

fn resolve_send_message_body_meta(
    message: &OwnedRelaxedMessage,
    abi: Option<&ContractABI>,
) -> Option<ResolvedSendMessageBodyMeta> {
    let body = message.body.0.apply(&message.body.1).ok()?;
    let prefix_to_skip = match &message.info {
        RelaxedMsgInfo::Int(info) if info.bounced => 32,
        _ => 0,
    };

    let mut opcode_parser = body;
    if prefix_to_skip > 0 && opcode_parser.skip_first(prefix_to_skip, 0).is_err() {
        return None;
    }
    let opcode = opcode_parser.load_u32().ok();

    let resolved_body = abi.and_then(|abi| match &message.info {
        RelaxedMsgInfo::Int(_) => try_resolve_message_body(
            body,
            abi,
            abi.outgoing_messages.iter().map(|message| &message.body_ty),
            prefix_to_skip,
        ),
        RelaxedMsgInfo::ExtOut(_) => try_resolve_message_body(
            body,
            abi,
            abi.emitted_events.iter().map(|message| &message.body_ty),
            prefix_to_skip,
        ),
    });

    if opcode.is_none() && resolved_body.is_none() {
        return None;
    }

    Some(ResolvedSendMessageBodyMeta {
        opcode,
        body_type_name: resolved_body
            .as_ref()
            .map(|resolved| resolved.type_name.clone()),
        body_decoded: resolved_body.map(|resolved| resolved.decoded),
    })
}

#[derive(Debug, Clone)]
struct ResolvedDecodedMessageBody {
    type_name: String,
    decoded: RenderedValue,
}

fn try_resolve_message_body<'a, I>(
    body: TyCellSlice<'a>,
    abi: &ContractABI,
    candidates: I,
    prefix_to_skip: u16,
) -> Option<ResolvedDecodedMessageBody>
where
    I: IntoIterator<Item = &'a Ty>,
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

        let Some(type_name) = compiler_body_type_name(body_ty) else {
            continue;
        };
        return Some(ResolvedDecodedMessageBody {
            type_name,
            decoded: render_abi_data(data, body_ty),
        });
    }

    None
}

fn compiler_body_type_name(body_ty: &Ty) -> Option<String> {
    match body_ty {
        Ty::StructRef { struct_name, .. } => Some(struct_name.clone()),
        Ty::AliasRef { alias_name, .. } => Some(alias_name.clone()),
        _ => None,
    }
}

fn render_relaxed_msg_info(info: &RelaxedMsgInfo) -> RenderedValue {
    match info {
        RelaxedMsgInfo::Int(info) => RenderedValue::Struct {
            type_name: "message info".to_owned(),
            fields: vec![
                (
                    "ihr_disabled".to_owned(),
                    RenderedValue::typed_leaf(info.ihr_disabled.to_string(), "bool"),
                ),
                (
                    "bounce".to_owned(),
                    RenderedValue::typed_leaf(info.bounce.to_string(), "bool"),
                ),
                (
                    "bounced".to_owned(),
                    RenderedValue::typed_leaf(info.bounced.to_string(), "bool"),
                ),
                (
                    "src".to_owned(),
                    render_optional_int_addr(info.src.as_ref()),
                ),
                ("dst".to_owned(), render_int_addr(&info.dst)),
                (
                    "value".to_owned(),
                    RenderedValue::typed_leaf(
                        format_currency_collection(&info.value),
                        "coins | (coins, ExtraCurrenciesMap)",
                    ),
                ),
                (
                    "fwd_fee".to_owned(),
                    RenderedValue::typed_leaf(info.fwd_fee.into_inner().to_string(), "coins"),
                ),
                (
                    "created_lt".to_owned(),
                    RenderedValue::typed_leaf(info.created_lt.to_string(), "uint64"),
                ),
                (
                    "created_at".to_owned(),
                    RenderedValue::typed_leaf(info.created_at.to_string(), "uint32"),
                ),
            ],
        },
        RelaxedMsgInfo::ExtOut(info) => RenderedValue::Struct {
            type_name: "message info".to_owned(),
            fields: vec![
                (
                    "src".to_owned(),
                    render_optional_int_addr(info.src.as_ref()),
                ),
                (
                    "dst".to_owned(),
                    RenderedValue::typed_leaf(
                        info.dst
                            .as_ref()
                            .map_or_else(|| "null".to_owned(), ToString::to_string),
                        "address?",
                    ),
                ),
                (
                    "created_lt".to_owned(),
                    RenderedValue::typed_leaf(info.created_lt.to_string(), "uint64"),
                ),
                (
                    "created_at".to_owned(),
                    RenderedValue::typed_leaf(info.created_at.to_string(), "uint32"),
                ),
            ],
        },
    }
}

fn render_state_init(state_init: &StateInit) -> RenderedValue {
    let libraries_count = state_init.libraries.iter().flatten().count();
    RenderedValue::Struct {
        type_name: "StateInit".to_owned(),
        fields: vec![
            (
                "fixedPrefixLength".to_owned(),
                match &state_init.split_depth {
                    Some(split_depth) => {
                        RenderedValue::typed_leaf(format!("{split_depth:?}"), "uint5?")
                    }
                    None => RenderedValue::typed_leaf("null", "uint5?"),
                },
            ),
            (
                "special".to_owned(),
                match &state_init.special {
                    Some(special) => RenderedValue::typed_leaf(
                        format!("tick: {}, tock: {}", special.tick, special.tock),
                        "(bool, bool)?",
                    ),
                    None => RenderedValue::typed_leaf("null", "(bool, bool)?"),
                },
            ),
            (
                "code".to_owned(),
                match &state_init.code {
                    Some(code) => render_cell_value(code),
                    None => RenderedValue::typed_leaf("null", "cell?"),
                },
            ),
            (
                "data".to_owned(),
                match &state_init.data {
                    Some(data) => render_cell_value(data),
                    None => RenderedValue::typed_leaf("null", "cell?"),
                },
            ),
            (
                "library".to_owned(),
                RenderedValue::typed_leaf(
                    if libraries_count == 0 {
                        "empty".to_owned()
                    } else {
                        format!("{libraries_count} libraries")
                    },
                    "cell?",
                ),
            ),
        ],
    }
}

fn render_optional_int_addr(addr: Option<&IntAddr>) -> RenderedValue {
    match addr {
        Some(addr) => render_int_addr(addr),
        None => RenderedValue::typed_leaf("null", "address?"),
    }
}

fn render_int_addr(addr: &IntAddr) -> RenderedValue {
    match addr {
        IntAddr::Std(addr) => render_std_address("address".to_owned(), addr.to_string(), addr),
        IntAddr::Var(_) => RenderedValue::typed_leaf(addr.to_string(), "address"),
    }
}

fn render_cell_slice_parts(parts: &(tycho_types::cell::CellSliceRange, Cell)) -> RenderedValue {
    let cell = cell_from_slice_parts(parts);
    cell.as_ref().map_or_else(
        || RenderedValue::typed_leaf("<invalid body>", "cell"),
        render_cell_value,
    )
}

fn cell_from_slice_parts(parts: &(tycho_types::cell::CellSliceRange, Cell)) -> Option<Cell> {
    let slice = parts.0.apply(&parts.1).ok()?;
    let mut builder = CellBuilder::new();
    builder.store_slice(slice).ok()?;
    builder.build().ok()
}

fn render_cell_value(cell: &Cell) -> RenderedValue {
    render_runtime_vm_value(&VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(cell))))
}

fn format_relaxed_msg_summary(info: &RelaxedMsgInfo, mode: String) -> String {
    match info {
        RelaxedMsgInfo::Int(info) => format!(
            "to {} with {} and {}",
            info.dst,
            format_currency_collection(&info.value),
            mode
        ),
        RelaxedMsgInfo::ExtOut(info) => format!(
            "to {} with {}",
            info.dst
                .as_ref()
                .map_or_else(|| "null".to_owned(), ToString::to_string),
            mode
        ),
    }
}

fn format_send_message_summary(
    info: &RelaxedMsgInfo,
    mode: String,
    body_meta: Option<&ResolvedSendMessageBodyMeta>,
) -> String {
    let summary = format_relaxed_msg_summary(info, mode);
    if let Some(body_type_name) = body_meta.and_then(|body_meta| body_meta.body_type_name.as_ref())
    {
        format!("{body_type_name} {summary}")
    } else {
        summary
    }
}

fn format_send_msg_flags(mode: SendMsgFlags) -> String {
    let mut parts = Vec::new();
    if mode.contains(SendMsgFlags::PAY_FEE_SEPARATELY) {
        parts.push("SEND_MODE_PAY_FEES_SEPARATELY");
    }
    if mode.contains(SendMsgFlags::IGNORE_ERROR) {
        parts.push("SEND_MODE_IGNORE_ERRORS");
    }
    if mode.contains(SendMsgFlags::BOUNCE_ON_ERROR) {
        parts.push("SEND_MODE_BOUNCE_ON_ACTION_FAIL");
    }
    if mode.contains(SendMsgFlags::DELETE_IF_EMPTY) {
        parts.push("SEND_MODE_DESTROY");
    }
    if mode.contains(SendMsgFlags::WITH_REMAINING_BALANCE) {
        parts.push("SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE");
    }
    if mode.contains(SendMsgFlags::ALL_BALANCE) {
        parts.push("SEND_MODE_CARRY_ALL_BALANCE");
    }

    let unknown = mode.bits() & !SendMsgFlags::all().bits();
    if unknown != 0 {
        return if parts.is_empty() {
            format!("0x{unknown:02x}")
        } else {
            format!("{} | 0x{unknown:02x}", parts.join(" | "))
        };
    }

    if parts.is_empty() {
        "SEND_MODE_REGULAR".to_owned()
    } else {
        parts.join(" | ")
    }
}

fn format_reserve_currency_flags(mode: ReserveCurrencyFlags) -> String {
    let mut parts = Vec::new();
    if mode.contains(ReserveCurrencyFlags::ALL_BUT) {
        parts.push("ALL_BUT");
    }
    if mode.contains(ReserveCurrencyFlags::IGNORE_ERROR) {
        parts.push("IGNORE_ERROR");
    }
    if mode.contains(ReserveCurrencyFlags::WITH_ORIGINAL_BALANCE) {
        parts.push("WITH_ORIGINAL_BALANCE");
    }
    if mode.contains(ReserveCurrencyFlags::REVERSE) {
        parts.push("REVERSE");
    }
    if mode.contains(ReserveCurrencyFlags::BOUNCE_ON_ERROR) {
        parts.push("BOUNCE_ON_ERROR");
    }

    let unknown = mode.bits() & !ReserveCurrencyFlags::all().bits();
    if unknown != 0 {
        return if parts.is_empty() {
            format!("0x{unknown:02x}")
        } else {
            format!("{} | 0x{unknown:02x}", parts.join(" | "))
        };
    }

    if parts.is_empty() {
        "0".to_owned()
    } else {
        parts.join(" | ")
    }
}

fn format_change_library_mode(mode: ChangeLibraryMode) -> String {
    let mut parts = Vec::new();
    match mode.bits() & 0b11 {
        0 => parts.push("REMOVE"),
        1 => parts.push("ADD_PRIVATE"),
        2 => parts.push("ADD_PUBLIC"),
        3 => {
            parts.push("ADD_PRIVATE");
            parts.push("ADD_PUBLIC");
        }
        _ => {}
    }
    if mode.contains(ChangeLibraryMode::BOUNCE_ON_ERROR) {
        parts.push("BOUNCE_ON_ERROR");
    }

    let unknown = mode.bits() & !(0b11 | ChangeLibraryMode::BOUNCE_ON_ERROR.bits());
    if unknown != 0 {
        return if parts.is_empty() {
            format!("0x{unknown:02x}")
        } else {
            format!("{} | 0x{unknown:02x}", parts.join(" | "))
        };
    }

    if parts.is_empty() {
        "0".to_owned()
    } else {
        parts.join(" | ")
    }
}

fn format_tokens(tokens: u128) -> String {
    let whole = tokens / 1_000_000_000;
    let frac = tokens % 1_000_000_000;
    format!("{whole}.{frac:09} TON")
}

fn format_currency_collection(currency: &CurrencyCollection) -> String {
    let mut result = format_tokens(currency.tokens.into_inner());
    if !currency.other.is_empty() {
        let mut other = Vec::new();
        for entry in currency.other.as_dict().iter().flatten() {
            let (currency_id, amount) = entry;
            other.push(format!("{currency_id}: {amount}"));
        }
        if !other.is_empty() {
            result.push_str(" + [");
            result.push_str(&other.join(", "));
            result.push(']');
        }
    }
    result
}

pub(crate) fn render_runtime_in_message(c7: &VmStackValue) -> Option<RenderedValue> {
    Some(RenderedValue::Struct {
        type_name: "InMessage".to_owned(),
        fields: vec![
            (
                "senderAddress".to_owned(),
                render_runtime_in_msg_sender_address_field(runtime_in_msg_param(c7, 2)?),
            ),
            (
                "valueCoins".to_owned(),
                render_runtime_coins_field(runtime_in_msg_param(c7, 7)?),
            ),
            (
                "valueExtra".to_owned(),
                render_runtime_extra_currencies_field(runtime_in_msg_param(c7, 8)?),
            ),
            (
                "originalForwardFee".to_owned(),
                render_runtime_coins_field(runtime_in_msg_param(c7, 3)?),
            ),
            (
                "createdLt".to_owned(),
                render_runtime_uint_field(runtime_in_msg_param(c7, 4)?, "uint64"),
            ),
            (
                "createdAt".to_owned(),
                render_runtime_uint_field(runtime_in_msg_param(c7, 5)?, "uint32"),
            ),
        ],
    })
}

fn runtime_in_msg_param(c7: &VmStackValue, index: usize) -> Option<&VmStackValue> {
    let env = match c7 {
        VmStackValue::Tuple(items) => items.first()?,
        _ => return None,
    };
    let in_msg_params = match env {
        VmStackValue::Tuple(items) => items.get(17)?,
        _ => return None,
    };
    match in_msg_params {
        VmStackValue::Tuple(items) => items.get(index),
        _ => None,
    }
}

fn render_runtime_in_msg_sender_address_field(value: &VmStackValue) -> RenderedValue {
    match value {
        VmStackValue::CellSlice(cs) => match try_parse_address(cs) {
            Some(raw) => match raw.parse::<StdAddr>() {
                Ok(addr) => render_std_address("address".to_owned(), addr.to_string(), &addr),
                Err(_) => RenderedValue::typed_leaf(raw, "address"),
            },
            None => RenderedValue::typed_leaf(render_slice(cs), "address"),
        },
        _ => RenderedValue::typed_leaf(render_runtime_vm_value(value).dap_value(), "address"),
    }
}

fn render_runtime_coins_field(value: &VmStackValue) -> RenderedValue {
    match value {
        VmStackValue::Integer(value) => RenderedValue::typed_leaf(value.clone(), "coins"),
        _ => RenderedValue::typed_leaf(render_runtime_vm_value(value).dap_value(), "coins"),
    }
}

fn render_runtime_uint_field(value: &VmStackValue, ty: &str) -> RenderedValue {
    match value {
        VmStackValue::Integer(value) => RenderedValue::typed_leaf(value.clone(), ty),
        _ => RenderedValue::typed_leaf(render_runtime_vm_value(value).dap_value(), ty),
    }
}

fn render_runtime_extra_currencies_field(value: &VmStackValue) -> RenderedValue {
    match value {
        VmStackValue::Cell(cell) => {
            let ty = Ty::MapKV {
                k: Box::new(Ty::IntN { n: 32 }),
                v: Box::new(Ty::VaruintN { n: 32 }),
            };
            let (bits, refs, hash) = cell_like_meta(cell);
            render_openable_cell_like(
                &ty,
                render_cell_like(cell),
                bits,
                refs,
                hash,
                Some(cell.clone()),
            )
        }
        VmStackValue::Null => RenderedValue::typed_leaf("()", "map<int32, varuint32>"),
        _ => RenderedValue::typed_leaf(
            render_runtime_vm_value(value).dap_value(),
            "map<int32, varuint32>",
        ),
    }
}

/// Helper to convert data from `SourceMap` to ABI
fn build_compiler_abi(symbols: &SourceMap) -> Option<ContractABI> {
    Some(ContractABI {
        abi_schema_version: "1.0".to_owned(),
        declarations: source_map_declarations_to_abi(symbols)?,
        ..Default::default()
    })
}

// TODO: do we really need two types for declarations?
fn source_map_declarations_to_abi(symbols: &SourceMap) -> Option<Vec<ABIDeclaration>> {
    symbols
        .declarations()
        .iter()
        .map(source_map_declaration_to_abi)
        .collect()
}

fn source_map_declaration_to_abi(decl: &Declaration) -> Option<ABIDeclaration> {
    Some(match decl {
        Declaration::Struct(decl) => {
            let prefix = decl.prefix.as_ref().map(|prefix| ABIOpcode {
                prefix_str: prefix.prefix_str.clone(),
                prefix_len: prefix.prefix_len,
            });
            ABIDeclaration::Struct {
                name: decl.name.clone(),
                type_params: decl.type_params.clone(),
                prefix,
                fields: decl
                    .fields
                    .iter()
                    .map(|field| ABIStructField {
                        name: field.name.clone(),
                        ty: field.ty.clone(),
                        default_value: None,
                        description: String::new(),
                    })
                    .collect(),
                custom_pack_unpack: None,
            }
        }
        Declaration::Alias(decl) => ABIDeclaration::Alias {
            name: decl.name.clone(),
            target_ty: decl.target_ty.clone(),
            type_params: decl.type_params.clone(),
            custom_pack_unpack: None,
        },
        Declaration::Enum(decl) => ABIDeclaration::Enum {
            name: decl.name.clone(),
            encoded_as: decl.encoded_as.clone(),
            members: decl
                .members
                .iter()
                .map(|member| ABIEnumMember {
                    name: member.name.clone(),
                    value: member.value.clone(),
                    description: String::new(),
                })
                .collect(),
            custom_pack_unpack: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tolk_compiler::abi::{
        ABIDeclaration, ABIOpcode, ABIOutgoingMessage, ABIStorage, ABIStructField,
    };
    use tolk_compiler::types_kernel::UnionVariant;
    use ton_abi::abi_serde::{DataField, DataObject};
    use tycho_types::cell::{CellFamily, HashBytes, Lazy, Store};
    use tycho_types::models::{RelaxedIntMsgInfo, SendMsgFlags};
    use tycho_types::models::{ReserveCurrencyFlags, StdAddr};

    #[test]
    fn render_abi_data_uses_field_type_for_object_fields() {
        let rendered = render_abi_data(
            ParsedAbiData::Object(DataObject {
                name: "Payload".to_owned(),
                fields: vec![DataField {
                    name: "flag".to_owned(),
                    field_type: Ty::Bool,
                    value: ParsedAbiData::Bool(true),
                }],
            }),
            &Ty::StructRef {
                struct_name: "Payload".to_owned(),
                type_args: None,
            },
        );

        let RenderedValue::Struct { fields, .. } = rendered else {
            panic!("expected struct");
        };
        let (_, value) = &fields[0];
        assert_eq!(value.dap_parts().1.as_deref(), Some("bool"));
    }

    #[test]
    fn render_abi_data_uses_container_types_for_tensor_items() {
        let rendered = render_abi_data(
            ParsedAbiData::Array(vec![
                ParsedAbiData::Bool(true),
                ParsedAbiData::Number(7.into()),
            ]),
            &Ty::Tensor {
                items: vec![Ty::Bool, Ty::UintN { n: 32 }],
            },
        );

        let RenderedValue::ArrayOf { items, .. } = rendered else {
            panic!("expected array");
        };
        assert_eq!(items[0].dap_parts().1.as_deref(), Some("bool"));
        assert_eq!(items[1].dap_parts().1.as_deref(), Some("uint32"));
    }

    #[test]
    fn render_abi_enum_value_exposes_raw_value_field() {
        let rendered = render_abi_data(
            ParsedAbiData::Object(DataObject {
                name: "Color.Blue".to_owned(),
                fields: vec![DataField {
                    name: "value".to_owned(),
                    field_type: Ty::UintN { n: 8 },
                    value: ParsedAbiData::Number(2.into()),
                }],
            }),
            &Ty::EnumRef {
                enum_name: "Color".to_owned(),
            },
        );

        let RenderedValue::EnumValue {
            type_name,
            value,
            fields,
        } = rendered
        else {
            panic!("expected enum value");
        };
        assert_eq!(type_name, "Color");
        assert_eq!(value, "Color.Blue");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "value");
        assert_eq!(fields[0].1.dap_parts().0, "2");
        assert_eq!(fields[0].1.dap_parts().1.as_deref(), Some("uint8"));
    }

    #[test]
    fn render_openable_cell_like_shows_bits_refs_and_hash() {
        let child = CellBuilder::new().build().unwrap();
        let mut builder = CellBuilder::new();
        builder.store_uint(7, 16).unwrap();
        builder.store_reference(child).unwrap();
        let cell = builder.build().unwrap();

        let (bits, refs, hash) = cell_like_meta(&CellLike::Cell(Boc::encode_hex(&cell)));
        let rendered = render_openable_cell_like(
            &Ty::Cell,
            render_cell_like(&CellLike::Cell(Boc::encode_hex(&cell))),
            bits,
            refs,
            hash,
            Some(CellLike::Cell(Boc::encode_hex(&cell))),
        );

        let RenderedValue::CellLike {
            type_name,
            value,
            fields,
            ..
        } = rendered
        else {
            panic!("expected CellLike");
        };
        assert_eq!(type_name, "cell");
        assert_eq!(
            value,
            format!(
                "16 bits, 1 refs, hash: {}",
                render_cell_hash_prefix(&render_cell_hash(&cell))
            )
        );
        assert_eq!(fields[0].0, "bits");
        assert_eq!(fields[0].1.dap_parts().0, "16");
        assert_eq!(fields[1].0, "refs");
        assert_eq!(fields[1].1.dap_parts().0, "1");
        assert_eq!(fields[2].0, "hash");
        assert_eq!(fields[2].1.dap_parts().0, render_cell_hash(&cell));
        assert_eq!(fields[3].0, "raw");
        assert_eq!(
            fields[3].1.dap_parts().0,
            format!("cell{{{}}}", Boc::encode_hex(&cell))
        );
    }

    #[test]
    fn render_runtime_in_message_reads_fields_from_c7() {
        let addr = IntAddr::Std(StdAddr::new(0, HashBytes([0x11; 32])));
        let mut builder = CellBuilder::new();
        addr.store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let addr_cell = builder.build().unwrap();
        let extra_cell = Cell::empty_cell();

        let mut in_msg_params = vec![VmStackValue::Null; 10];
        in_msg_params[2] = VmStackValue::CellSlice(CellSlice {
            value: Boc::encode_hex(&addr_cell),
            bits: None,
            refs: None,
        });
        in_msg_params[3] = VmStackValue::Integer("123456789".to_owned());
        in_msg_params[4] = VmStackValue::Integer("42".to_owned());
        in_msg_params[5] = VmStackValue::Integer("1710000000".to_owned());
        in_msg_params[7] = VmStackValue::Integer("1000000000".to_owned());
        in_msg_params[8] = VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&extra_cell)));

        let mut env = vec![VmStackValue::Null; 18];
        env[17] = VmStackValue::Tuple(in_msg_params);

        let rendered =
            render_runtime_in_message(&VmStackValue::Tuple(vec![VmStackValue::Tuple(env)]))
                .expect("expected in message");

        let RenderedValue::Struct { type_name, fields } = rendered else {
            panic!("expected InMessage struct");
        };
        assert_eq!(type_name, "InMessage");
        assert_eq!(fields.len(), 6);
        assert_eq!(fields[0].0, "senderAddress");
        assert_eq!(fields[0].1.dap_parts().0, addr.to_string());
        assert_eq!(fields[0].1.dap_parts().1.as_deref(), Some("address"));
        assert_eq!(fields[1].0, "valueCoins");
        assert_eq!(fields[1].1.dap_parts().0, "1000000000");
        assert_eq!(fields[1].1.dap_parts().1.as_deref(), Some("coins"));
        assert_eq!(fields[2].0, "valueExtra");
        assert_eq!(
            fields[2].1.dap_parts().1.as_deref(),
            Some("map<int32, varuint32>")
        );
        assert_eq!(fields[3].0, "originalForwardFee");
        assert_eq!(fields[3].1.dap_parts().0, "123456789");
        assert_eq!(fields[3].1.dap_parts().1.as_deref(), Some("coins"));
        assert_eq!(fields[4].0, "createdLt");
        assert_eq!(fields[4].1.dap_parts().0, "42");
        assert_eq!(fields[4].1.dap_parts().1.as_deref(), Some("uint64"));
        assert_eq!(fields[5].0, "createdAt");
        assert_eq!(fields[5].1.dap_parts().0, "1710000000");
        assert_eq!(fields[5].1.dap_parts().1.as_deref(), Some("uint32"));
    }

    #[test]
    fn render_runtime_storage_chooses_storage_type_that_decodes_cell() {
        let abi = ContractABI {
            declarations: vec![
                ABIDeclaration::Struct {
                    name: "Storage".to_owned(),
                    type_params: None,
                    prefix: None,
                    fields: vec![ABIStructField {
                        name: "count".to_owned(),
                        ty: Ty::UintN { n: 32 },
                        default_value: None,
                        description: String::new(),
                    }],
                    custom_pack_unpack: None,
                },
                ABIDeclaration::Struct {
                    name: "DeploymentStorage".to_owned(),
                    type_params: None,
                    prefix: None,
                    fields: vec![ABIStructField {
                        name: "ready".to_owned(),
                        ty: Ty::Bool,
                        default_value: None,
                        description: String::new(),
                    }],
                    custom_pack_unpack: None,
                },
            ],
            storage: ABIStorage {
                storage_ty: Some(Ty::StructRef {
                    struct_name: "Storage".to_owned(),
                    type_args: None,
                }),
                storage_at_deployment_ty: Some(Ty::StructRef {
                    struct_name: "DeploymentStorage".to_owned(),
                    type_args: None,
                }),
            },
            ..Default::default()
        };
        let mut builder = CellBuilder::new();
        builder.store_uint(7, 32).unwrap();
        let cell = builder.build().unwrap();

        let rendered = render_runtime_storage_with_compiler_abi(
            &VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&cell))),
            &abi,
        )
        .expect("expected decoded c4");

        let RenderedValue::CellOf {
            type_name,
            value,
            fields,
            ..
        } = rendered
        else {
            panic!("expected CellOf");
        };
        assert_eq!(type_name, "Cell<Storage>");
        assert_eq!(
            value,
            format!(
                "32 bits, 0 refs, hash: {}",
                render_cell_hash_prefix(&render_cell_hash(&cell))
            )
        );
        assert_eq!(fields[0].0, "decoded");
        assert_eq!(fields[4].0, "raw");
        assert_eq!(
            fields[4].1.dap_parts().0,
            format!("cell{{{}}}", Boc::encode_hex(&cell))
        );

        let RenderedValue::Struct {
            type_name,
            fields: decoded_fields,
        } = &fields[0].1
        else {
            panic!("expected decoded storage");
        };
        assert_eq!(type_name, "Storage");
        assert_eq!(decoded_fields[0].0, "count");
        assert_eq!(decoded_fields[0].1.dap_parts().0, "7");
        assert_eq!(decoded_fields[0].1.dap_parts().1.as_deref(), Some("uint32"));
    }

    #[test]
    fn named_dap_parts_format_coins_only_for_ton_word() {
        let rendered = RenderedValue::typed_leaf("1022000000", "coins");

        assert_eq!(
            rendered.dap_parts_for_client(Some("tonAmount")).0,
            "1.022 TON"
        );
        assert_eq!(
            rendered.dap_parts_for_client(Some("forward_ton_amount")).0,
            "1.022 TON"
        );
        assert_eq!(
            rendered.dap_parts_for_client(Some("jettonAmount")).0,
            "1022000000"
        );
        assert_eq!(
            rendered.dap_parts_for_client(Some("amount")).0,
            "1022000000"
        );
    }

    #[test]
    fn named_dap_parts_preserve_last_seen_suffix_for_formatted_coins() {
        let rendered = RenderedValue::LastSeen {
            inner: Box::new(RenderedValue::typed_leaf("1022000000", "coins")),
        };

        assert_eq!(
            rendered.dap_parts_for_client(Some("forwardTonAmount")).0,
            "1.022 TON (last seen)"
        );
        assert_eq!(
            rendered.dap_parts_for_client(Some("jettonAmount")).0,
            "1022000000 (last seen)"
        );
    }

    #[test]
    fn legacy_named_dap_value_formats_coins_only_for_ton_word() {
        let rendered = RenderedValue::typed_leaf("1022000000", "coins");

        assert_eq!(rendered.legacy_dap_value(Some("tonAmount")), "1.022 TON");
        assert_eq!(
            rendered.legacy_dap_value(Some("forward_ton_amount")),
            "1.022 TON"
        );
        assert_eq!(
            rendered.legacy_dap_value(Some("jettonAmount")),
            "1022000000"
        );
        assert_eq!(rendered.legacy_dap_value(None), "1022000000");
    }

    #[test]
    fn legacy_named_dap_value_preserves_last_seen_suffix_for_formatted_coins() {
        let rendered = RenderedValue::LastSeen {
            inner: Box::new(RenderedValue::typed_leaf("1022000000", "coins")),
        };

        assert_eq!(
            rendered.legacy_dap_value(Some("forwardTonAmount")),
            "1.022 TON (last seen)"
        );
        assert_eq!(
            rendered.legacy_dap_value(Some("jettonAmount")),
            "1022000000 (last seen)"
        );
    }

    #[test]
    fn render_address_from_slice_keeps_legacy_value_as_address() {
        let addr = StdAddr::new(0, HashBytes([0x22; 32]));
        let mut builder = CellBuilder::new();
        IntAddr::Std(addr.clone())
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let addr_cell = builder.build().unwrap();
        let stack_values = [VmStackValue::CellSlice(CellSlice {
            value: Boc::encode_hex(&addr_cell),
            bits: None,
            refs: None,
        })];
        let slots = [SlotValue::Live(&stack_values[0])];

        let rendered = debug_print_from_stack(&SourceMap::default(), &slots, &Ty::Address);

        let RenderedValue::Address {
            legacy_value,
            value,
            type_name,
            ..
        } = rendered
        else {
            panic!("expected address");
        };
        assert_eq!(type_name, "address");
        assert_eq!(value, addr.to_string());
        assert_eq!(legacy_value, addr.to_string());
    }

    #[test]
    fn render_stack_enum_value_exposes_raw_value_field() {
        let mut symbols_json = serde_json::json!({
            "files": [],
            "declarations": [{
                "kind": "enum",
                "name": "Color",
                "ident_loc": [0, 0, 0, 0, 0],
                "encoded_as": {"kind": "uintN", "n": 8},
                "members": [
                    {"name": "Red", "value": "1"},
                    {"name": "Blue", "value": "2"}
                ]
            }],
            "unique_ty": [],
            "functions": [],
            "debug_marks": []
        });
        let symbols: SourceMap = serde_json::from_value(symbols_json.take()).unwrap();

        let stack_values = [VmStackValue::Integer("2".to_owned())];
        let slots = [SlotValue::Live(&stack_values[0])];
        let rendered = debug_print_from_stack(
            &symbols,
            &slots,
            &Ty::EnumRef {
                enum_name: "Color".to_owned(),
            },
        );

        let RenderedValue::EnumValue {
            type_name,
            value,
            fields,
        } = rendered
        else {
            panic!("expected enum value");
        };
        assert_eq!(type_name, "Color");
        assert_eq!(value, "Color.Blue");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "value");
        assert_eq!(fields[0].1.dap_parts().0, "2");
        assert_eq!(fields[0].1.dap_parts().1.as_deref(), Some("uint8"));
    }

    #[test]
    fn render_union_case_preserves_inner_children() {
        let cell = CellBuilder::new().build().unwrap();
        let union_ty = Ty::Union {
            variants: vec![
                UnionVariant {
                    variant_ty: Ty::Cell,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: Some(1),
                    stack_width: Some(1),
                },
                UnionVariant {
                    variant_ty: Ty::Int,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: Some(2),
                    stack_width: Some(1),
                },
            ],
            stack_width: Some(2),
        };
        let stack_values = [
            VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&cell))),
            VmStackValue::Integer("1".to_owned()),
        ];
        let slots = [
            SlotValue::Live(&stack_values[0]),
            SlotValue::Live(&stack_values[1]),
        ];

        let rendered = debug_print_from_stack(&SourceMap::default(), &slots, &union_ty);
        let (dap_value, dap_type) = rendered.dap_parts();

        let RenderedValue::UnionCase {
            type_name,
            variant_name,
            fields,
        } = rendered
        else {
            panic!("expected UnionCase");
        };
        assert_eq!(type_name, "cell | int");
        assert_eq!(variant_name, "cell");
        assert_eq!(dap_value, "cell");
        assert_eq!(dap_type.as_deref(), Some("cell | int"));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "value");
        let RenderedValue::CellLike { fields, .. } = &fields[0].1 else {
            panic!("expected nested CellLike");
        };
        assert_eq!(fields[0].0, "bits");
        assert_eq!(fields[1].0, "refs");
        assert_eq!(fields[2].0, "hash");
    }

    #[test]
    fn render_null_union_variant_has_no_value_field() {
        let union_ty = Ty::Union {
            variants: vec![
                UnionVariant {
                    variant_ty: Ty::NullLiteral,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: Some(0),
                    stack_width: Some(0),
                },
                UnionVariant {
                    variant_ty: Ty::Bool,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: Some(1),
                    stack_width: Some(1),
                },
            ],
            stack_width: Some(2),
        };
        let stack_values = [VmStackValue::Null, VmStackValue::Integer("0".to_owned())];
        let slots = [
            SlotValue::Live(&stack_values[0]),
            SlotValue::Live(&stack_values[1]),
        ];

        let rendered = debug_print_from_stack(&SourceMap::default(), &slots, &union_ty);

        let RenderedValue::UnionCase {
            variant_name,
            fields,
            ..
        } = rendered
        else {
            panic!("expected UnionCase");
        };
        assert_eq!(variant_name, "null");
        assert!(fields.is_empty());
    }

    #[test]
    fn render_union_without_stack_width_falls_back_without_panic() {
        let union_ty = Ty::Union {
            variants: vec![
                UnionVariant {
                    variant_ty: Ty::Int,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: Some(1),
                    stack_width: Some(1),
                },
                UnionVariant {
                    variant_ty: Ty::Cell,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: Some(2),
                    stack_width: Some(1),
                },
            ],
            stack_width: None,
        };
        let stack_values = [VmStackValue::Integer("7".to_owned())];
        let slots = [SlotValue::Live(&stack_values[0])];

        let rendered = debug_print_from_stack(&SourceMap::default(), &slots, &union_ty);

        assert_eq!(rendered.dap_parts().0, "union with unresolved layout");
        assert_eq!(rendered.dap_parts().1.as_deref(), Some("int | cell"));
    }

    #[test]
    fn render_openable_slice_shows_bits_refs_and_hash() {
        let mut builder = CellBuilder::new();
        builder.store_uint(0xabcd, 16).unwrap();
        let cell = builder.build().unwrap();
        let hash = render_cell_hash(&cell);
        let rendered = render_openable_cell_like(
            &Ty::Slice,
            "slice{abcd}",
            Some(16),
            Some(0),
            Some(hash.clone()),
            None,
        );

        let RenderedValue::CellLike { value, fields, .. } = rendered else {
            panic!("expected CellLike");
        };
        assert_eq!(
            value,
            format!("16 bits, 0 refs, hash: {}", render_cell_hash_prefix(&hash))
        );
        assert_eq!(fields[0].0, "bits");
        assert_eq!(fields[0].1.dap_parts().0, "16");
        assert_eq!(fields[1].0, "refs");
        assert_eq!(fields[1].1.dap_parts().0, "0");
        assert_eq!(fields[2].0, "hash");
        assert_eq!(fields[2].1.dap_parts().0, hash);
        assert_eq!(fields[3].0, "raw");
        assert_eq!(fields[3].1.dap_parts().0, "slice{abcd}");
    }

    #[test]
    fn render_openable_builder_shows_bits_refs_and_hash() {
        let child = CellBuilder::new().build().unwrap();
        let mut builder = CellBuilder::new();
        builder.store_uint(7, 16).unwrap();
        builder.store_reference(child).unwrap();
        let cell = builder.build().unwrap();
        let builder_hex = Boc::encode_hex(&cell);
        let cell_like = CellLike::Builder(builder_hex.clone());
        let (bits, refs, hash) = cell_like_meta(&cell_like);

        let rendered = render_openable_cell_like(
            &Ty::Builder,
            render_builder(&builder_hex),
            bits,
            refs,
            hash,
            Some(cell_like),
        );

        let RenderedValue::CellLike { value, fields, .. } = rendered else {
            panic!("expected CellLike");
        };
        assert_eq!(
            value,
            format!(
                "16 bits, 1 refs, hash: {}",
                render_cell_hash_prefix(&render_cell_hash(&cell))
            )
        );
        assert_eq!(fields[0].0, "bits");
        assert_eq!(fields[0].1.dap_parts().0, "16");
        assert_eq!(fields[1].0, "refs");
        assert_eq!(fields[1].1.dap_parts().0, "1");
        assert_eq!(fields[2].0, "hash");
        assert_eq!(fields[2].1.dap_parts().0, render_cell_hash(&cell));
        assert_eq!(fields[3].0, "raw");
        assert_eq!(fields[3].1.dap_parts().0, render_builder(&builder_hex));
    }

    #[test]
    fn render_runtime_out_actions_decodes_reserve_currency() {
        let action = OutAction::ReserveCurrency {
            mode: ReserveCurrencyFlags::WITH_ORIGINAL_BALANCE,
            value: CurrencyCollection::new(2_500_000_000),
        };
        let cell = build_out_actions_cell(&[action]);

        let rendered = render_runtime_out_actions(
            &VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&cell))),
            None,
        )
        .expect("expected decoded c5");

        let RenderedValue::CellOf {
            type_name, fields, ..
        } = rendered
        else {
            panic!("expected CellOf");
        };
        assert_eq!(type_name, "Cell<array<OutAction>>");
        assert_eq!(fields[0].0, "decoded");

        let RenderedValue::ArrayOf {
            type_name, items, ..
        } = &fields[0].1
        else {
            panic!("expected decoded action array");
        };
        assert_eq!(type_name, "array<OutAction>");
        assert_eq!(items.len(), 1);

        let RenderedValue::UnionCase {
            type_name,
            variant_name,
            fields,
        } = &items[0]
        else {
            panic!("expected out action");
        };
        assert_eq!(type_name, "OutAction");
        assert_eq!(
            variant_name,
            "ReserveCurrency 2.500000000 TON with WITH_ORIGINAL_BALANCE"
        );
        assert_eq!(fields[0].0, "mode");
        assert_eq!(fields[0].1.dap_parts().0, "WITH_ORIGINAL_BALANCE");
        assert_eq!(fields[1].0, "value");
        assert_eq!(fields[1].1.dap_parts().0, "2.500000000 TON");
    }

    #[test]
    fn render_runtime_out_actions_decodes_send_msg() {
        let abi = ContractABI {
            declarations: vec![ABIDeclaration::Struct {
                name: "Transfer".to_owned(),
                type_params: None,
                prefix: Some(ABIOpcode {
                    prefix_str: "0xfeedbeef".to_owned(),
                    prefix_len: 32,
                }),
                fields: vec![],
                custom_pack_unpack: None,
            }],
            outgoing_messages: vec![ABIOutgoingMessage {
                body_ty: Ty::StructRef {
                    struct_name: "Transfer".to_owned(),
                    type_args: None,
                },
                description: String::new(),
            }],
            ..Default::default()
        };
        let mut body_builder = CellBuilder::new();
        body_builder.store_u32(0xfeed_beef).unwrap();
        let body = body_builder.build().unwrap();
        let message = OwnedRelaxedMessage {
            info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
                dst: IntAddr::Std(StdAddr::new(0, HashBytes([0x11; 32]))),
                value: CurrencyCollection::new(1_000_000_000),
                ..Default::default()
            }),
            init: None,
            body: body.into(),
            layout: None,
        };
        let action = OutAction::SendMsg {
            mode: SendMsgFlags::PAY_FEE_SEPARATELY,
            out_msg: Lazy::new(&message).unwrap(),
        };
        let cell = build_out_actions_cell(&[action]);

        let rendered = render_runtime_out_actions(
            &VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&cell))),
            Some(&abi),
        )
        .expect("expected decoded c5");

        let RenderedValue::CellOf { fields, .. } = rendered else {
            panic!("expected CellOf");
        };
        let RenderedValue::ArrayOf { items, .. } = &fields[0].1 else {
            panic!("expected action array");
        };
        let RenderedValue::UnionCase {
            variant_name,
            fields,
            ..
        } = &items[0]
        else {
            panic!("expected out action");
        };
        assert!(variant_name.contains("Transfer"));
        assert_eq!(fields[0].0, "mode");
        assert_eq!(fields[0].1.dap_parts().0, "SEND_MODE_PAY_FEES_SEPARATELY");
        assert_eq!(fields[1].0, "out_msg");
        assert_eq!(fields[2].0, "out_msg_raw");

        let RenderedValue::Struct {
            type_name,
            fields: message_fields,
        } = &fields[1].1
        else {
            panic!("expected rendered message");
        };
        assert_eq!(type_name, "OutMessage");
        assert_eq!(message_fields[0].0, "info");
        assert_eq!(message_fields[1].0, "opcode");
        assert_eq!(message_fields[1].1.dap_parts().0, "0xfeedbeef");
        assert_eq!(message_fields[2].0, "init");
        assert_eq!(message_fields[2].1.dap_parts().0, "null");
        assert_eq!(
            message_fields[2].1.dap_parts().1.as_deref(),
            Some("StateInit?")
        );
        assert_eq!(message_fields[3].0, "body");
        let RenderedValue::CellOf {
            type_name,
            fields: body_fields,
            ..
        } = &message_fields[3].1
        else {
            panic!("expected typed body");
        };
        assert_eq!(type_name, "Cell<Transfer>");
        assert_eq!(body_fields[0].0, "decoded");
        let RenderedValue::Struct {
            type_name,
            fields: decoded_fields,
        } = &body_fields[0].1
        else {
            panic!("expected decoded body");
        };
        assert_eq!(type_name, "Transfer");
        assert!(decoded_fields.is_empty());
    }

    fn build_out_actions_cell(actions: &[OutAction]) -> Cell {
        let mut head = Cell::empty_cell();
        for action in actions {
            let mut builder = CellBuilder::new();
            builder.store_reference(head).unwrap();
            action
                .store_into(&mut builder, Cell::empty_context())
                .unwrap();
            head = builder.build().unwrap();
        }
        head
    }
}
