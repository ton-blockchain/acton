use std::collections::HashMap;
use std::fmt::{self, Write};
use tolkc::source_map::SourceMap;
use tolkc::types_kernel::{Ty, calc_width_on_stack, instantiate_generics};
use vmlogs::parser::{CellSlice, VmStackValue};

// ---------------------------------------------------------------------------
// RenderedValue — structured intermediate format for rendered values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum RenderedValue {
    Leaf(String),
    Struct {
        type_name: String,
        fields: Vec<(String, RenderedValue)>,
    },
    Tensor {
        items: Vec<RenderedValue>,
    },
    ArrayOf {
        items: Vec<RenderedValue>,
    },

    LastSeen {
        inner: Box<RenderedValue>,
    },
    OptimizedOut,
    NotYetLoaded,
    LazyUnresolved {
        type_name: String,
    },
}

impl RenderedValue {
    /// Short summary for DAP tree view ("StructName" for structs, "N items" for tensors).
    /// Distinct from Display which produces the full flat string for CLI/tests.
    pub fn dap_value(&self) -> String {
        match self {
            RenderedValue::Leaf(s) => s.clone(),
            RenderedValue::Struct { type_name, .. } => type_name.clone(),
            RenderedValue::Tensor { items } => format!("{} items", items.len()),
            RenderedValue::ArrayOf { items } => format!("{} items", items.len()),
            RenderedValue::LastSeen { inner } => format!("{} (last seen)", inner.dap_value()),
            RenderedValue::OptimizedOut => "<optimized out>".to_string(),
            RenderedValue::NotYetLoaded => "<not loaded>".to_string(),
            RenderedValue::LazyUnresolved { type_name } => {
                format!("{type_name} (lazy, unresolved)")
            }
        }
    }

    pub fn has_children(&self) -> bool {
        match self {
            RenderedValue::Struct { fields, .. } => !fields.is_empty(),
            RenderedValue::Tensor { items } => !items.is_empty(),
            RenderedValue::ArrayOf { items } => !items.is_empty(),
            RenderedValue::LastSeen { inner } => inner.has_children(),
            _ => false,
        }
    }
}

impl fmt::Display for RenderedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderedValue::Leaf(s) => write!(f, "{s}"),
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
            RenderedValue::Tensor { items } => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            RenderedValue::ArrayOf { items } => {
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
            RenderedValue::NotYetLoaded => write!(f, "<not loaded>"),
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
pub enum SlotValue<'a> {
    Live(&'a VmStackValue),
    LastSeen(&'a VmStackValue),
    OptimizedOut,
}

impl<'a> SlotValue<'a> {
    pub const fn is_optimized_out(&self) -> bool {
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

/// Try to parse addr_std from a CellSlice.
/// Cell{hex} starts with 2 descriptor bytes (4 hex chars); cell data follows.
/// `bits: start..end` are positions within cell data.
/// addr_std = `10` (2b) + `0` (1b anycast) + workchain (8b) + hash (256b) = 267 bits.
fn try_parse_address(cs: &CellSlice) -> Option<String> {
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
    Some(format!("{}:{}", wc, hash))
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
        write!(hex, "{:x}", n).ok();
    }

    if remaining_bits > 0 {
        let mut last: u8 = 0;
        for i in 0..remaining_bits {
            last = (last << 1) | get_bit(nibbles, start + full_nibbles * 4 + i);
        }
        last = (last << 1) | 1;
        last <<= 4 - remaining_bits - 1;
        write!(hex, "{:x}_", last).ok();
    }

    hex
}

fn resolve_alias_target(symbols: &SourceMap, alias_name: &str, type_args: Option<&[Ty]>) -> Ty {
    let alias_ref = symbols.get_alias(alias_name);
    match type_args {
        Some(type_args) => instantiate_generics(
            &alias_ref.target_ty,
            alias_ref.type_params.as_deref().unwrap_or(&[]),
            type_args,
        ),
        None => alias_ref.target_ty.clone(),
    }
}

fn render_named_type(name: &str, type_args: Option<&[Ty]>) -> String {
    match type_args {
        None => name.to_string(),
        Some(type_args) => format!(
            "{}<{}>",
            name,
            type_args
                .iter()
                .map(|ty| ty.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
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

/// Render a CellSlice as `slice{HEX}`, extracting only the bits in `start..end`.
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
            SlotValue::Live(VmStackValue::Integer(s)) => RenderedValue::Leaf(s.to_string()),
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM int".to_string()),
        },

        Ty::Bool => match r.read_slot() {
            SlotValue::Live(VmStackValue::Integer(s)) => {
                if s == "0" {
                    RenderedValue::Leaf("false".to_string())
                } else {
                    RenderedValue::Leaf("true".to_string())
                }
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM int".to_string()),
        },

        Ty::Cell => match r.read_slot() {
            SlotValue::Live(VmStackValue::Cell(_)) => RenderedValue::Leaf("cell".to_string()),
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM cell".to_string()),
        },

        Ty::CellOf { inner } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Cell(_)) => RenderedValue::Leaf(format!("Cell<{inner}>")),
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM cell".to_string()),
        },

        Ty::String => match r.read_slot() {
            SlotValue::Live(VmStackValue::Cell(_)) => {
                RenderedValue::Leaf("string (contents unavailable)".to_string())
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM cell".to_string()),
        },

        Ty::Builder => match r.read_slot() {
            SlotValue::Live(VmStackValue::Builder(b)) => RenderedValue::Leaf(render_builder(b)),
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM builder".to_string()),
        },

        Ty::Slice | Ty::Remaining | Ty::BitsN { .. } => match r.read_slot() {
            SlotValue::Live(VmStackValue::CellSlice(cs)) => RenderedValue::Leaf(render_slice(cs)),
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM slice".to_string()),
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
                    RenderedValue::ArrayOf { items }
                }
                SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
                _ => RenderedValue::Leaf("not a TVM tuple".to_string()),
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
                RenderedValue::ArrayOf { items }
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::ArrayOf { items: vec![] },
            _ => RenderedValue::Leaf("not a TVM tuple".to_string()),
        },

        Ty::Address | Ty::AddressOpt | Ty::AddressExt | Ty::AddressAny => match r.read_slot() {
            SlotValue::Live(VmStackValue::CellSlice(cs)) => {
                RenderedValue::Leaf(try_parse_address(cs).unwrap_or_else(|| render_slice(cs)))
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM slice".to_string()),
        },

        Ty::MapKV { .. } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("empty map".to_string()),
            SlotValue::Live(VmStackValue::Cell(_)) => {
                RenderedValue::Leaf("non-empty map".to_string())
            }
            _ => RenderedValue::Leaf("not a TVM cell".to_string()),
        },

        Ty::NullLiteral => match r.read_slot() {
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM null".to_string()),
        },

        Ty::Void => RenderedValue::Leaf("(void)".to_string()),

        Ty::Callable => match r.read_slot() {
            SlotValue::Live(VmStackValue::Continuation(_)) => {
                RenderedValue::Leaf("continuation".to_string())
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM continuation".to_string()),
        },
        Ty::Unknown => match r.read_slot() {
            SlotValue::Live(any) => RenderedValue::Leaf(any.to_string()),
            _ => RenderedValue::Leaf("unreachable".to_string()),
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
                            RenderedValue::Leaf("null".to_string())
                        } else {
                            let mut sub = StackReader::new(&nullable_slots[..sw - 1]);
                            debug_format(symbols, &mut sub, inner, false)
                        }
                    }
                    SlotValue::OptimizedOut => {
                        let mut sub = StackReader::new(&nullable_slots[..sw - 1]);
                        debug_format(symbols, &mut sub, inner, false)
                    }
                    _ => RenderedValue::Leaf("corrupted stack for nullable".to_string()),
                }
            } else {
                // read a primitive one-slot nullable: either TVM null or a value of type inner
                match r.peek_slot() {
                    SlotValue::Live(VmStackValue::Null)
                    | SlotValue::LastSeen(VmStackValue::Null) => {
                        r.read_slot();
                        RenderedValue::Leaf("null".to_string())
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
            let struct_name = render_named_type(struct_name, type_args.as_deref());
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
        } => debug_format(
            symbols,
            r,
            &resolve_alias_target(symbols, alias_name, type_args.as_deref()),
            false,
        ),

        Ty::EnumRef { enum_name } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Integer(s)) => {
                let enum_ref = symbols.get_enum(enum_name);
                let text = enum_ref
                    .members
                    .iter()
                    .find(|m| &m.value == s)
                    .map(|m| format!("{}.{}", enum_ref.name, m.name))
                    .unwrap_or_else(|| format!("{}({})", enum_ref.name, s));
                RenderedValue::Leaf(text)
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM int".to_string()),
        },

        Ty::Tensor { items } => {
            let items: Vec<RenderedValue> = items
                .iter()
                .map(|item| debug_format(symbols, r, item, false))
                .collect();
            RenderedValue::Tensor { items }
        }

        Ty::ShapedTuple { items } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Tuple(t)) => {
                let as_live: Vec<SlotValue> = t.iter().map(SlotValue::Live).collect();
                let mut sub = StackReader::new(&as_live);
                let items: Vec<RenderedValue> = items
                    .iter()
                    .map(|item| debug_format(symbols, &mut sub, item, true))
                    .collect();
                RenderedValue::ArrayOf { items }
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::Leaf("null".to_string()),
            _ => RenderedValue::Leaf("not a TVM tuple".to_string()),
        },

        Ty::Union {
            variants,
            stack_width: Some(stack_width),
        } => {
            // read tagged union: [smth, smth, ... type_id]
            let union_slots = r.read_n_slots(*stack_width);
            let tag_slot = &union_slots[stack_width - 1];
            match tag_slot {
                SlotValue::Live(VmStackValue::Integer(type_id))
                | SlotValue::LastSeen(VmStackValue::Integer(type_id)) => {
                    let type_id: usize = type_id.parse().unwrap_or(100500);
                    if let Some(variant) =
                        variants.iter().find(|v| v.stack_type_id == Some(type_id))
                    {
                        let mut sub = StackReader::new(
                            &union_slots[stack_width - 1 - variant.stack_width.unwrap_or(0)
                                ..stack_width - 1],
                        );
                        let inner = debug_format(symbols, &mut sub, &variant.variant_ty, false);
                        if matches!(&variant.variant_ty, Ty::StructRef { .. }) {
                            inner
                        } else {
                            RenderedValue::Leaf(format!("#{} {inner}", variant.variant_ty))
                        }
                    } else {
                        // corrupted stack, type_id on a stack mismatches all variants
                        RenderedValue::Leaf("union with unknown variant".to_string())
                    }
                }
                SlotValue::OptimizedOut => {
                    // this should not happen in practice, because if UTag for a union was erased during compilation,
                    // a union was definitely smart cast, and its type is narrowed, not Ty::Union
                    RenderedValue::Leaf("union with unknown variant".to_string())
                }
                _ => RenderedValue::Leaf("corrupted stack for union".to_string()),
            }
        }

        Ty::GenericT { name_t } => RenderedValue::Leaf(format!("unexpected genericT={name_t}")),

        _ => {
            panic!("unexpected TVM type");
        }
    }
}

pub fn debug_print_from_stack(symbols: &SourceMap, slots: &[SlotValue], ty: &Ty) -> RenderedValue {
    let mut r = StackReader::new(slots);
    debug_format(symbols, &mut r, ty, false)
}

// ---------------------------------------------------------------------------
// debug_format_lazy — renders a lazy variable, showing <not loaded> for
// fields whose ir_slots have never been observed on the stack.
// `last_seen` keys serve as the set of IR indices that have appeared in at
// least one MARK_STACK during replay so far.
// ---------------------------------------------------------------------------

pub fn debug_format_lazy(
    symbols: &SourceMap,
    slot_values: &[SlotValue],
    ir_slots: &[usize],
    ty: &Ty,
    last_seen: &HashMap<usize, VmStackValue>,
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
            let resolved = resolve_alias_target(symbols, alias_name, type_args.as_deref());
            if matches!(&resolved, Ty::Union { .. }) {
                return RenderedValue::LazyUnresolved {
                    type_name: alias_name.clone(),
                };
            }
            debug_format_lazy(symbols, slot_values, ir_slots, &resolved, last_seen)
        }

        Ty::StructRef {
            struct_name,
            type_args,
        } => {
            let struct_ref = symbols.get_struct(struct_name);
            let mut fields: Vec<(String, RenderedValue)> = Vec::new();
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

                let field_val = if field_ever_seen {
                    let field_slot_values = &slot_values[offset..offset + f_width];
                    let mut r = StackReader::new(field_slot_values);
                    debug_format(symbols, &mut r, &f_ty, false)
                } else {
                    RenderedValue::NotYetLoaded
                };
                fields.push((f.name.clone(), field_val));
                offset += f_width;
            }
            RenderedValue::Struct {
                type_name: format!(
                    "{} (lazy)",
                    render_named_type(struct_name, type_args.as_deref())
                ),
                fields,
            }
        }

        _ => {
            let mut r = StackReader::new(slot_values);
            debug_format(symbols, &mut r, ty, false)
        }
    }
}
