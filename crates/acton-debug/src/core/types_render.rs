use owo_colors::OwoColorize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::sync::OnceLock;
use tolk_compiler::SourceMap;
use tolk_compiler::abi::ContractABI;
use tolk_compiler::dynamic_unpack::{self, UnpackSchema, UnpackedValue};
use tolk_compiler::source_map::{AbiStruct, Declaration};
use tolk_compiler::types_kernel::{Ty, TyIdx, calc_width_on_stack, render_ty};
use tvm_ffi::from_stack::FromStack;
use tvm_ffi::stack::{Tuple, TupleItem};
use tvm_logs::parser::{CellLike, CellSlice, VmStackValue};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellSlice as TyCellSlice, Load};
use tycho_types::dict;
use tycho_types::models::{
    AnyAddr, Base64StdAddrFlags, ChangeLibraryMode, CurrencyCollection, DisplayBase64StdAddr,
    IntAddr, LibRef, OutAction, OutActionsRevIter, OwnedRelaxedMessage, RelaxedMsgInfo,
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
    MapKV {
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

#[derive(Debug, Clone, Default)]
pub struct PrettyRenderOptions {
    pub address_format: PrettyAddressFormat,
    pub address_labels: HashMap<String, String>,
    pub colorize: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum PrettyAddressFormat {
    #[default]
    Raw,
    Mainnet,
    Testnet,
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
    #[must_use]
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
            RenderedValue::MapKV { type_name, fields } => {
                (map_kv_summary(fields), Some(type_name.clone()))
            }
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
            RenderedValue::MapKV { fields, .. } => map_kv_summary(fields),
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

    #[must_use]
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

    #[must_use]
    pub fn dap_value(&self) -> String {
        self.dap_parts().0
    }

    #[must_use]
    pub fn has_children(&self) -> bool {
        match self {
            RenderedValue::Struct { fields, .. }
            | RenderedValue::MapKV { fields, .. }
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

    #[must_use]
    pub fn to_pretty_string(&self, options: PrettyRenderOptions) -> String {
        let mut out = String::new();
        self.write_pretty(&mut out, 0, &options)
            .expect("writing to String should not fail");
        out
    }

    fn write_pretty(
        &self,
        out: &mut String,
        indent: usize,
        options: &PrettyRenderOptions,
    ) -> fmt::Result {
        match self {
            RenderedValue::Leaf { value, type_field } => {
                write!(
                    out,
                    "{}",
                    pretty_leaf_value(value, type_field.as_deref(), options)
                )
            }
            RenderedValue::CellLike {
                value, fields, raw, ..
            } => {
                write!(
                    out,
                    "{}",
                    pretty_cell_like_value(value, raw.as_ref(), fields, options)
                )
            }
            RenderedValue::EnumValue { value, .. } => {
                write!(out, "{}", pretty_magenta(value, options))
            }
            RenderedValue::Address { value, fields, .. } => {
                let value = pretty_address_value(value, fields, options);
                write!(out, "{}", pretty_cyan(&value, options))
            }
            RenderedValue::CellOf {
                type_name,
                value,
                fields,
                ..
            } if cell_of_has_decoded(fields) => {
                writeln!(
                    out,
                    "{} {} {}",
                    pretty_type_name(type_name, options),
                    pretty_dimmed(value, options),
                    pretty_dimmed("{", options)
                )?;
                for (name, value) in fields {
                    write_indent(out, indent + 4)?;
                    write!(out, "{name}: ")?;
                    value.write_pretty(out, indent + 4, options)?;
                    writeln!(out, ",")?;
                }
                write_indent(out, indent)?;
                write!(out, "{}", pretty_dimmed("}", options))
            }
            RenderedValue::CellOf {
                type_name,
                value,
                raw,
                ..
            } => write!(
                out,
                "{} {}",
                pretty_type_name(type_name, options),
                pretty_cell_like_value(value, raw.as_ref(), &[], options)
            ),
            RenderedValue::UnionCase {
                variant_name,
                fields,
                ..
            } => match fields.iter().find(|(name, _)| name == "value") {
                Some((_, value)) => {
                    write!(out, "{} ", pretty_magenta(variant_name, options))?;
                    value.write_pretty(out, indent, options)
                }
                None => write!(out, "{}", pretty_magenta(variant_name, options)),
            },
            RenderedValue::Struct { type_name, fields }
            | RenderedValue::MapKV { type_name, fields }
                if fields.is_empty() =>
            {
                write!(
                    out,
                    "{} {}{}",
                    pretty_type_name(type_name, options),
                    pretty_dimmed("{", options),
                    pretty_dimmed("}", options)
                )
            }
            RenderedValue::Struct { type_name, fields }
            | RenderedValue::MapKV { type_name, fields } => {
                writeln!(
                    out,
                    "{} {}",
                    pretty_type_name(type_name, options),
                    pretty_dimmed("{", options)
                )?;
                for (name, value) in fields {
                    write_indent(out, indent + 4)?;
                    let name = pretty_field_name(type_name, name, options);
                    let name = pretty_map_key(type_name, &name, options);
                    write!(out, "{name}: ")?;
                    value.write_pretty(out, indent + 4, options)?;
                    writeln!(out, ",")?;
                }
                write_indent(out, indent)?;
                write!(out, "{}", pretty_dimmed("}", options))
            }
            RenderedValue::Tensor { items, .. } => {
                write_collection_pretty(out, indent, items, '(', ')', options)
            }
            RenderedValue::ArrayOf { items, .. } => {
                write_collection_pretty(out, indent, items, '[', ']', options)
            }
            RenderedValue::LastSeen { inner } => {
                inner.write_pretty(out, indent, options)?;
                write!(out, " (last seen)")
            }
            RenderedValue::OptimizedOut => {
                write!(out, "{}", pretty_dimmed("<optimized out>", options))
            }
            RenderedValue::LazyNotYetLoaded { preview } => {
                preview.write_pretty(out, indent, options)?;
                write!(out, " (not loaded)")
            }
            RenderedValue::LazyCantParseSlice => {
                write!(out, "{}", pretty_dimmed("<not loaded>", options))
            }
            RenderedValue::LazyUnresolved { type_name } => {
                write!(
                    out,
                    "{} (lazy, unresolved)",
                    pretty_type_name(type_name, options)
                )
            }
        }
    }

    fn wants_multiline_pretty(&self) -> bool {
        match self {
            RenderedValue::Struct { fields, .. } | RenderedValue::MapKV { fields, .. } => {
                !fields.is_empty()
            }
            RenderedValue::CellOf { fields, .. } => cell_of_has_decoded(fields),
            RenderedValue::Tensor { items, .. } | RenderedValue::ArrayOf { items, .. } => {
                items.iter().any(Self::wants_multiline_pretty)
            }
            RenderedValue::LastSeen { inner }
            | RenderedValue::LazyNotYetLoaded { preview: inner } => inner.wants_multiline_pretty(),
            _ => false,
        }
    }
}

fn cell_of_has_decoded(fields: &[(String, RenderedValue)]) -> bool {
    fields.iter().any(|(name, _)| name == "decoded")
}

fn pretty_field_name<'a>(
    type_name: &str,
    name: &'a str,
    options: &PrettyRenderOptions,
) -> Cow<'a, str> {
    if !map_key_is_address(type_name) {
        return Cow::Borrowed(name);
    }

    name.parse::<StdAddr>()
        .ok()
        .map(|addr| Cow::Owned(format_std_address_for_pretty(&addr, options)))
        .unwrap_or(Cow::Borrowed(name))
}

fn map_key_is_address(type_name: &str) -> bool {
    type_name.starts_with("map<address,") || type_name.starts_with("map<any_address,")
}

fn map_kv_summary(fields: &[(String, RenderedValue)]) -> String {
    if fields.is_empty() {
        "{}".to_owned()
    } else if fields.len() == 1 {
        "1 entry".to_owned()
    } else {
        format!("{} entries", fields.len())
    }
}

fn pretty_leaf_value(
    value: &str,
    type_field: Option<&str>,
    options: &PrettyRenderOptions,
) -> String {
    if value == "null" {
        return pretty_bold(value, options);
    }

    let Some(type_field) = type_field else {
        return value.to_owned();
    };

    if type_is_string_like(type_field) && value.starts_with('"') && value.ends_with('"') {
        return pretty_green(value, options);
    }
    if type_is_number_like(type_field) || type_field == "bool" {
        return pretty_yellow(value, options);
    }
    if type_is_address_like(type_field) {
        return pretty_cyan(value, options);
    }
    if type_is_cell_like(type_field) {
        return pretty_cell_like_value(value, None, &[], options);
    }

    value.to_owned()
}

fn pretty_map_key(type_name: &str, key: &str, options: &PrettyRenderOptions) -> String {
    let Some(key_ty) = map_key_type_name(type_name) else {
        return key.to_owned();
    };

    if type_is_string_like(key_ty) {
        return pretty_green(key, options);
    }
    if type_is_number_like(key_ty) || key_ty == "bool" {
        return pretty_yellow(key, options);
    }
    if type_is_address_like(key_ty) {
        return pretty_cyan(key, options);
    }
    if type_is_cell_like(key_ty) {
        return pretty_cell_like_value(key, None, &[], options);
    }

    key.to_owned()
}

fn map_key_type_name(type_name: &str) -> Option<&str> {
    let inner = type_name.trim().strip_prefix("map<")?.strip_suffix('>')?;
    let split_idx = find_top_level_comma(inner)?;
    Some(inner[..split_idx].trim())
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
            ',' if angle_depth == 0 && paren_depth == 0 && square_depth == 0 => return Some(idx),
            _ => {}
        }
    }

    None
}

fn type_is_number_like(type_name: &str) -> bool {
    matches!(type_name, "int" | "uint" | "coins")
        || type_name.starts_with("int")
        || type_name.starts_with("uint")
        || type_name.starts_with("varint")
        || type_name.starts_with("varuint")
}

fn type_is_string_like(type_name: &str) -> bool {
    type_name == "string"
}

fn type_is_address_like(type_name: &str) -> bool {
    matches!(type_name, "address" | "any_address" | "external_address")
}

fn type_is_cell_like(type_name: &str) -> bool {
    matches!(type_name, "cell" | "slice" | "builder" | "bits")
        || type_name.starts_with("Cell<")
        || type_name.starts_with("bits")
}

fn pretty_yellow(value: &str, options: &PrettyRenderOptions) -> String {
    if options.colorize {
        value.yellow().to_string()
    } else {
        value.to_owned()
    }
}

fn pretty_green(value: &str, options: &PrettyRenderOptions) -> String {
    if options.colorize {
        value.green().to_string()
    } else {
        value.to_owned()
    }
}

fn pretty_magenta(value: &str, options: &PrettyRenderOptions) -> String {
    if options.colorize {
        value.magenta().to_string()
    } else {
        value.to_owned()
    }
}

fn pretty_type_name(value: &str, options: &PrettyRenderOptions) -> String {
    if !options.colorize {
        return value.to_owned();
    }

    // simple lexer to colorize only words in `Cell<int32>`-like types
    let mut out = String::with_capacity(value.len());
    let mut ident_start = None;
    for (idx, ch) in value.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            ident_start.get_or_insert(idx);
            continue;
        }

        if let Some(start) = ident_start.take() {
            out.push_str(&pretty_magenta(&value[start..idx], options));
        }
        out.push(ch);
    }
    if let Some(start) = ident_start {
        out.push_str(&pretty_magenta(&value[start..], options));
    }

    out
}

fn pretty_cyan(value: &str, options: &PrettyRenderOptions) -> String {
    if options.colorize {
        value.cyan().to_string()
    } else {
        value.to_owned()
    }
}

fn pretty_dimmed(value: &str, options: &PrettyRenderOptions) -> String {
    if options.colorize {
        value.dimmed().to_string()
    } else {
        value.to_owned()
    }
}

fn pretty_cell_like_value(
    value: &str,
    raw: Option<&CellLike>,
    fields: &[(String, RenderedValue)],
    options: &PrettyRenderOptions,
) -> String {
    let value = pretty_empty_cell_like_text(value).map_or_else(
        || {
            slice_raw_field(fields).map_or_else(
                || raw.map_or_else(|| compact_cell_like_text(value), raw_cell_like_hex),
                Cow::Borrowed,
            )
        },
        Cow::Borrowed,
    );
    pretty_dimmed(&value, options)
}

const fn pretty_empty_cell_like_text(value: &str) -> Option<&'static str> {
    match value.as_bytes() {
        b"empty slice" => Some("slice{}"),
        b"empty builder" => Some("builder{}"),
        _ => None,
    }
}

fn raw_cell_like_hex(raw: &CellLike) -> Cow<'_, str> {
    match raw {
        CellLike::Cell(hex) | CellLike::Builder(hex) => Cow::Borrowed(hex),
    }
}

fn slice_raw_field(fields: &[(String, RenderedValue)]) -> Option<&str> {
    fields.iter().find_map(|(name, value)| {
        if name != "raw" {
            return None;
        }
        let RenderedValue::Leaf { value, .. } = value else {
            return None;
        };
        value.starts_with("slice{").then_some(value.as_str())
    })
}

fn compact_cell_like_text(value: &str) -> Cow<'_, str> {
    for prefix in ["cell{", "builder{", "slice{"] {
        if let Some(inner) = value.strip_prefix(prefix)
            && let Some(end) = inner.find('}')
        {
            let (hex, suffix) = inner.split_at(end);
            let suffix = &suffix[1..];
            if suffix.is_empty() {
                return Cow::Borrowed(hex);
            }
            if suffix.starts_with(" + ") {
                return Cow::Owned(format!("{hex}{suffix}"));
            }
        }
    }

    Cow::Borrowed(value)
}

fn pretty_bold(value: &str, options: &PrettyRenderOptions) -> String {
    if options.colorize {
        value.bold().to_string()
    } else {
        value.to_owned()
    }
}

fn pretty_address_value<'a>(
    raw: &'a str,
    fields: &'a [(String, RenderedValue)],
    options: &PrettyRenderOptions,
) -> Cow<'a, str> {
    let field_name = match options.address_format {
        PrettyAddressFormat::Raw => return labeled_address_value(raw, raw, options),
        PrettyAddressFormat::Mainnet => "mainnet",
        PrettyAddressFormat::Testnet => "testnet",
    };

    let rendered = fields
        .iter()
        .find_map(|(name, value)| {
            if name == field_name
                && let RenderedValue::Leaf { value, .. } = value
            {
                Some(value.as_str())
            } else {
                None
            }
        })
        .unwrap_or(raw);

    labeled_address_value(rendered, raw, options)
}

fn labeled_address_value<'a>(
    rendered: &'a str,
    raw: &str,
    options: &PrettyRenderOptions,
) -> Cow<'a, str> {
    match options.address_labels.get(raw) {
        Some(label) => Cow::Owned(format!("{rendered} ({label})")),
        None => Cow::Borrowed(rendered),
    }
}

fn format_std_address_for_pretty(addr: &StdAddr, options: &PrettyRenderOptions) -> String {
    match options.address_format {
        PrettyAddressFormat::Raw => addr.to_string(),
        PrettyAddressFormat::Mainnet | PrettyAddressFormat::Testnet => DisplayBase64StdAddr {
            addr,
            flags: Base64StdAddrFlags {
                testnet: matches!(options.address_format, PrettyAddressFormat::Testnet),
                base64_url: true,
                bounceable: true,
            },
        }
        .to_string(),
    }
}

fn render_int_address(type_name: String, addr: &IntAddr) -> RenderedValue {
    match addr {
        IntAddr::Std(addr) => render_std_address(type_name, addr.to_string(), addr),
        IntAddr::Var(_) => RenderedValue::typed_leaf(addr.to_string(), type_name),
    }
}

fn render_cell_address(
    symbols: &dyn UnpackSchema,
    type_name: String,
    ty_idx: TyIdx,
    cell: &CellLike,
) -> RenderedValue {
    // TonCenter returns address stack values as cells, so ABI address fields
    // need this decode path before falling back to generic cell rendering.
    if let Some(address) = decode_cell_like(cell).and_then(|cell| cell.parse::<IntAddr>().ok()) {
        render_int_address(type_name, &address)
    } else {
        typed_leaf_for_ty(symbols, ty_idx, render_cell_like(cell))
    }
}

fn write_indent(out: &mut String, indent: usize) -> fmt::Result {
    for _ in 0..indent {
        out.write_char(' ')?;
    }
    Ok(())
}

fn write_collection_pretty(
    out: &mut String,
    indent: usize,
    items: &[RenderedValue],
    open: char,
    close: char,
    options: &PrettyRenderOptions,
) -> fmt::Result {
    if items.is_empty() {
        return write!(out, "{open}{close}");
    }

    if !items.iter().any(RenderedValue::wants_multiline_pretty) {
        write!(out, "{open}")?;
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                write!(out, ", ")?;
            }
            item.write_pretty(out, indent, options)?;
        }
        return write!(out, "{close}");
    }

    writeln!(out, "{open}")?;
    for item in items {
        write_indent(out, indent + 4)?;
        item.write_pretty(out, indent + 4, options)?;
        writeln!(out, ",")?;
    }
    write_indent(out, indent)?;
    write!(out, "{close}")
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
    Bits { bits: u16 },
    VarInt { len_bits: u8, signed: bool },
    Bool,
    Address,
    Cell,
    String,
}

impl MapScalarType {
    const fn bit_len(self) -> u16 {
        match self {
            Self::Int { bits, .. } | Self::Bits { bits } => bits,
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
            RenderedValue::Struct { type_name, fields }
            | RenderedValue::MapKV { type_name, fields }
                if fields.is_empty() =>
            {
                write!(f, "{type_name} {{}}")
            }
            RenderedValue::Struct { type_name, fields }
            | RenderedValue::MapKV { type_name, fields } => {
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

/// Try to parse `addr_none` or `addr_std` from a `CellSlice`.
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
    let bit_len = end.checked_sub(start)?;

    let data_hex = cs.value.get(4..)?; // skip d1, d2
    let nibbles: Vec<u8> = data_hex
        .chars()
        .filter_map(|c| c.to_digit(16).map(|d| d as u8))
        .collect();
    if nibbles.len() * 4 < end {
        return None;
    }

    match bit_len {
        2 if get_bits_u8(&nibbles, start, 2) == 0b00 => Some("addr_none".to_owned()),
        267 => {
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
        _ => None,
    }
}

fn try_parse_full_address_hex(hex: &str) -> Option<String> {
    let cell = Boc::decode_hex(hex).ok()?;
    match cell.bit_len() {
        2 if matches!(cell.parse::<AnyAddr>().ok()?, AnyAddr::None) => Some("addr_none".to_owned()),
        _ => StdAddr::from_item(TupleItem::Slice(cell))
            .ok()
            .map(|addr| addr.to_string()),
    }
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

fn parse_range_len(range: Option<&(String, String)>) -> Option<usize> {
    let (start, end) = range?;
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

const fn is_empty_cell_like(bits: Option<usize>, refs: Option<usize>) -> bool {
    matches!((bits, refs), (Some(0), Some(0)))
}

fn empty_cell_like_value(
    type_name: &str,
    bits: Option<usize>,
    refs: Option<usize>,
) -> Option<&'static str> {
    if !is_empty_cell_like(bits, refs) {
        return None;
    }

    match type_name {
        "cell" => Some("empty cell"),
        "slice" => Some("empty slice"),
        "builder" => Some("empty builder"),
        _ => None,
    }
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
        (Some(_), Some(_)) => (
            parse_range_len(cs.bits.as_ref()),
            parse_range_len(cs.refs.as_ref()),
        ),
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
    type_name: impl Into<String>,
    value: impl Into<String>,
    bits: Option<usize>,
    refs: Option<usize>,
    hash: Option<String>,
    raw: Option<CellLike>,
) -> RenderedValue {
    let type_name = type_name.into();
    let raw_value = value.into();
    if let Some(empty_value) = empty_cell_like_value(&type_name, bits, refs) {
        return RenderedValue::CellLike {
            type_name,
            value: empty_value.to_owned(),
            fields: Vec::new(),
            raw,
        };
    }

    let summarize_value = bits.is_some() || refs.is_some() || hash.is_some();

    RenderedValue::CellLike {
        type_name,
        value: if summarize_value {
            render_cell_summary(bits, refs, hash.as_deref())
        } else {
            raw_value.clone()
        },
        fields: render_cell_fields(bits, refs, hash, summarize_value.then_some(raw_value)),
        raw,
    }
}

fn render_openable_cell_like_name(
    type_name: impl Into<String>,
    value: impl Into<String>,
    bits: Option<usize>,
    refs: Option<usize>,
    hash: Option<String>,
    raw: Option<CellLike>,
) -> RenderedValue {
    let type_name = type_name.into();
    let raw_value = value.into();
    if let Some(empty_value) = empty_cell_like_value(&type_name, bits, refs) {
        return RenderedValue::CellLike {
            type_name,
            value: empty_value.to_owned(),
            fields: Vec::new(),
            raw,
        };
    }

    let summarize_value = bits.is_some() || refs.is_some() || hash.is_some();

    RenderedValue::CellLike {
        type_name,
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
                "cell",
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
                "builder",
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
                "slice",
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
    type_name: impl Into<String>,
    variant_name: impl Into<String>,
    value: Option<RenderedValue>,
) -> RenderedValue {
    let mut fields = Vec::new();
    if let Some(value) = value {
        fields.push(("value".to_owned(), value));
    }
    RenderedValue::UnionCase {
        type_name: type_name.into(),
        variant_name: variant_name.into(),
        fields,
    }
}

fn render_enum_value(
    type_name: impl Into<String>,
    value: impl Into<String>,
    raw_value: RenderedValue,
) -> RenderedValue {
    RenderedValue::EnumValue {
        type_name: type_name.into(),
        value: value.into(),
        fields: vec![("value".to_owned(), raw_value)],
    }
}

fn render_enum_value_name(
    type_name: String,
    value: impl Into<String>,
    raw_value: RenderedValue,
) -> RenderedValue {
    RenderedValue::EnumValue {
        type_name,
        value: value.into(),
        fields: vec![("value".to_owned(), raw_value)],
    }
}

fn render_map_raw(type_name: String, root: Option<&Cell>) -> RenderedValue {
    match root {
        Some(root) => RenderedValue::typed_leaf(
            format!("{type_name} {{raw: {}}}", Boc::encode_hex(root)),
            type_name,
        ),
        None => RenderedValue::MapKV {
            type_name,
            fields: vec![],
        },
    }
}

fn parse_map_key_type(ty: &Ty) -> Option<MapScalarType> {
    match ty {
        Ty::Bool => Some(MapScalarType::Bool),
        Ty::BitsN { n } => u16::try_from(*n)
            .is_ok()
            .then_some(MapScalarType::Bits { bits: *n as u16 }),
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
        MapScalarType::Bits { bits } => {
            let mut bytes = vec![0u8; usize::from(bits).div_ceil(8)];
            slice
                .load_raw(&mut bytes, bits)
                .map_err(|e| e.to_string())?;
            Ok(format_abi_bits(&bytes, usize::from(bits)))
        }
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
        MapScalarType::Cell => Ok(Boc::encode_hex(
            &slice.load_reference_cloned().map_err(|e| e.to_string())?,
        )),
        MapScalarType::String => {
            let cell = slice.load_reference_cloned().map_err(|e| e.to_string())?;
            if let Some(string) = Tuple::parse_snake_string(&cell) {
                return Ok(format!("\"{string}\""));
            }
            Ok(Boc::encode_hex(&cell))
        }
    }
}

fn format_map_raw_value(slice: TyCellSlice<'_>) -> Result<String, String> {
    let mut builder = CellBuilder::new();
    builder.store_slice(slice).map_err(|e| e.to_string())?;
    let cell = builder.build().map_err(|e| e.to_string())?;
    Ok(Boc::encode_hex(&cell))
}

fn decode_abi_data(
    symbols: &dyn UnpackSchema,
    parser: &mut TyCellSlice<'_>,
    ty_idx: TyIdx,
) -> Option<UnpackedValue> {
    let data = dynamic_unpack::unpack_from_slice(parser, symbols, ty_idx).ok()?;
    if parser.size_bits() != 0 || parser.size_refs() != 0 {
        // there are remaining data
        return None;
    }
    Some(data)
}

fn render_map_value_with_symbols(
    symbols: &dyn UnpackSchema,
    value_slice: TyCellSlice<'_>,
    value_ty_idx: TyIdx,
) -> Option<RenderedValue> {
    let mut parser = value_slice;
    let data = decode_abi_data(symbols, &mut parser, value_ty_idx)?;
    Some(render_abi_data(symbols, data, value_ty_idx))
}

fn render_typed_cell(
    symbols: &dyn UnpackSchema,
    type_name: String,
    inner_ty_idx: TyIdx,
    cell: &CellLike,
) -> RenderedValue {
    let decoded = if let Some(cell) = decode_cell_like(cell) {
        let mut parser = cell.as_slice_allow_exotic();
        decode_abi_data(symbols, &mut parser, inner_ty_idx).map(|data| (inner_ty_idx, data))
    } else {
        None
    };
    render_typed_cell_with_decoded_data(symbols, type_name, cell, decoded)
}

fn render_typed_cell_with_decoded_data(
    symbols: &dyn UnpackSchema,
    type_name: String,
    cell: &CellLike,
    decoded: Option<(TyIdx, UnpackedValue)>,
) -> RenderedValue {
    let value = render_cell_like(cell);
    let (bits, refs, hash) = cell_like_meta(cell);
    if decoded.is_none() && is_empty_cell_like(bits, refs) {
        return RenderedValue::CellOf {
            type_name,
            value: "empty cell".to_owned(),
            fields: Vec::new(),
            raw: Some(cell.clone()),
        };
    }

    let mut fields = render_cell_fields(bits, refs, hash.clone(), Some(value));

    if let Some((inner_ty_idx, data)) = decoded {
        fields.insert(
            0,
            (
                "decoded".to_owned(),
                render_abi_data(symbols, data, inner_ty_idx),
            ),
        );
    }

    RenderedValue::CellOf {
        type_name,
        value: render_cell_summary(bits, refs, hash.as_deref()),
        fields,
        raw: Some(cell.clone()),
    }
}

pub(crate) fn render_runtime_storage_with_abi(
    value: &VmStackValue,
    symbols: &SourceMap,
    abi: &ContractABI,
) -> Option<RenderedValue> {
    let VmStackValue::Cell(cell) = value else {
        return None;
    };

    let decoded_cell = decode_cell_like(cell)?;

    // Try deployment storage first and then default one
    for storage_ty_idx in abi
        .storage
        .storage_at_deployment_ty_idx
        .into_iter()
        .chain(abi.storage.storage_ty_idx)
    {
        let storage_ty_idx =
            source_map_ty_idx_for_abi_ty(symbols, abi, storage_ty_idx).unwrap_or(storage_ty_idx);
        let mut parser = decoded_cell.as_slice_allow_exotic();
        if let Some(data) = decode_abi_data(symbols, &mut parser, storage_ty_idx) {
            return Some(render_typed_cell_with_decoded_data(
                symbols,
                format!("Cell<{}>", render_ty(symbols, storage_ty_idx)),
                cell,
                Some((storage_ty_idx, data)),
            ));
        }
    }

    None
}

fn source_map_ty_idx_for_abi_ty(
    symbols: &SourceMap,
    abi: &ContractABI,
    abi_ty_idx: TyIdx,
) -> Option<TyIdx> {
    match abi.ty_by_idx(abi_ty_idx)? {
        Ty::StructRef { struct_name, .. } => symbols.declarations().iter().find_map(|decl| {
            let Declaration::Struct(struct_decl) = decl else {
                return None;
            };
            (struct_decl.name == *struct_name).then_some(struct_decl.ty_idx)
        }),
        Ty::AliasRef { alias_name, .. } => symbols.declarations().iter().find_map(|decl| {
            let Declaration::Alias(alias_decl) = decl else {
                return None;
            };
            (alias_decl.name == *alias_name).then_some(alias_decl.ty_idx)
        }),
        _ => symbols.ty_by_idx(abi_ty_idx).map(|_| abi_ty_idx),
    }
}

fn render_abi_data(
    symbols: &dyn UnpackSchema,
    data: UnpackedValue,
    ty_idx: TyIdx,
) -> RenderedValue {
    let type_name = render_ty(symbols, ty_idx);
    let shape_ty_idx = resolve_alias_shape_ty(symbols, ty_idx).unwrap_or(ty_idx);
    if abi_object_is_enum(symbols, shape_ty_idx) {
        return render_abi_enum_data(symbols, data, type_name, shape_ty_idx);
    }

    match data {
        UnpackedValue::Object { name, fields } => {
            let object_ty_idx = abi_object_context_ty(symbols, &name, ty_idx).unwrap_or(ty_idx);
            RenderedValue::Struct {
                type_name: name.clone(),
                fields: fields
                    .into_iter()
                    .map(|(field_name, value)| {
                        let field_ty_idx =
                            abi_object_field_ty(symbols, object_ty_idx, &name, &field_name)
                                .unwrap_or(object_ty_idx);
                        (field_name, render_abi_data(symbols, value, field_ty_idx))
                    })
                    .collect(),
            }
        }
        UnpackedValue::Array(items) => RenderedValue::ArrayOf {
            type_name,
            items: render_abi_array_items(symbols, items, ty_idx),
        },
        UnpackedValue::Map(entries) => render_abi_map(symbols, entries, ty_idx),
        UnpackedValue::Address(IntAddr::Std(addr)) => {
            render_std_address(type_name, addr.to_string(), &addr)
        }
        UnpackedValue::Address(addr) => render_int_address(type_name, &addr),
        UnpackedValue::ExtAddress(addr) => typed_leaf(type_name, addr.to_string()),
        UnpackedValue::Cell(cell) | UnpackedValue::RemainingBitsAndRefs(cell) => {
            typed_leaf(type_name, Boc::encode_hex(&cell))
        }
        UnpackedValue::Bits((bytes, bit_len)) => {
            typed_leaf(type_name, format_abi_bits(&bytes, bit_len))
        }
        UnpackedValue::AddressNone => typed_leaf(type_name, "addr_none"),
        UnpackedValue::Null => typed_leaf(type_name, "null"),
        UnpackedValue::Void => typed_leaf(type_name, "(void)"),
        UnpackedValue::Number(value) => typed_leaf(type_name, value.to_string()),
        UnpackedValue::Bool(value) => typed_leaf(type_name, value.to_string()),
        UnpackedValue::String(value) => typed_leaf(type_name, format!("\"{value}\"")),
    }
}

#[must_use]
pub fn render_unpacked_value_as_tolk_type(
    symbols: &dyn UnpackSchema,
    data: UnpackedValue,
    ty_idx: TyIdx,
) -> RenderedValue {
    render_abi_data(symbols, data, ty_idx)
}

fn render_abi_enum_data(
    symbols: &dyn UnpackSchema,
    data: UnpackedValue,
    type_name: String,
    enum_ty_idx: TyIdx,
) -> RenderedValue {
    let raw_ty_idx = enum_raw_value_ty(symbols, enum_ty_idx).unwrap_or(enum_ty_idx);
    let value_name = abi_enum_value_name(symbols, enum_ty_idx, &data)
        .unwrap_or_else(|| abi_enum_fallback_value(symbols, enum_ty_idx, &data));

    render_enum_value_name(
        type_name,
        value_name,
        render_abi_data(symbols, data, raw_ty_idx),
    )
}

fn render_abi_array_items(
    symbols: &dyn UnpackSchema,
    items: Vec<UnpackedValue>,
    ty_idx: TyIdx,
) -> Vec<RenderedValue> {
    let shape_ty_idx = resolve_alias_shape_ty(symbols, ty_idx).unwrap_or(ty_idx);
    match symbols.ty_by_idx(shape_ty_idx).unwrap_or(&Ty::Unknown) {
        Ty::ArrayOf { inner_ty_idx } => items
            .into_iter()
            .map(|item| render_abi_data(symbols, item, *inner_ty_idx))
            .collect(),
        Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let item_ty_idx = items_ty_idx.get(index).copied().unwrap_or(shape_ty_idx);
                render_abi_data(symbols, item, item_ty_idx)
            })
            .collect(),
        _ => items
            .into_iter()
            .map(|item| render_abi_data(symbols, item, ty_idx))
            .collect(),
    }
}

fn render_abi_map(
    symbols: &dyn UnpackSchema,
    entries: Vec<(UnpackedValue, UnpackedValue)>,
    ty_idx: TyIdx,
) -> RenderedValue {
    let type_name = render_ty(symbols, ty_idx);
    let shape_ty_idx = resolve_alias_shape_ty(symbols, ty_idx).unwrap_or(ty_idx);
    let (key_ty, value_ty) = match symbols.ty_by_idx(shape_ty_idx).unwrap_or(&Ty::Unknown) {
        Ty::MapKV {
            key_ty_idx,
            value_ty_idx,
        } => (*key_ty_idx, *value_ty_idx),
        _ => (ty_idx, ty_idx),
    };

    RenderedValue::MapKV {
        type_name,
        fields: entries
            .into_iter()
            .map(|(key, value)| {
                (
                    format_abi_map_key(symbols, &key, key_ty),
                    render_abi_data(symbols, value, value_ty),
                )
            })
            .collect(),
    }
}

fn format_abi_map_key(
    symbols: &dyn UnpackSchema,
    data: &UnpackedValue,
    key_ty_idx: TyIdx,
) -> String {
    let shape_ty_idx = resolve_alias_shape_ty(symbols, key_ty_idx).unwrap_or(key_ty_idx);
    if abi_object_is_enum(symbols, shape_ty_idx) {
        return abi_enum_value_name(symbols, shape_ty_idx, data)
            .unwrap_or_else(|| abi_enum_fallback_value(symbols, shape_ty_idx, data));
    }

    match data {
        UnpackedValue::Null => "null".to_owned(),
        UnpackedValue::Void => "(void)".to_owned(),
        UnpackedValue::Number(value) => value.to_string(),
        UnpackedValue::Bool(value) => value.to_string(),
        UnpackedValue::String(value) => format!("\"{value}\""),
        UnpackedValue::Object { name, .. } if abi_object_is_enum(symbols, key_ty_idx) => {
            name.clone()
        }
        UnpackedValue::AddressNone => "addr_none".to_owned(),
        UnpackedValue::Address(value) => value.to_string(),
        UnpackedValue::ExtAddress(value) => value.to_string(),
        UnpackedValue::Cell(value) | UnpackedValue::RemainingBitsAndRefs(value) => {
            Boc::encode_hex(value)
        }
        UnpackedValue::Bits((bytes, bit_len)) => format_abi_bits(bytes, *bit_len),
        UnpackedValue::Object { .. } | UnpackedValue::Array(_) | UnpackedValue::Map(_) => {
            typed_leaf(render_ty(symbols, key_ty_idx), "<key>").dap_value()
        }
    }
}

fn abi_object_is_enum(symbols: &dyn UnpackSchema, ty_idx: TyIdx) -> bool {
    matches!(
        symbols
            .ty_by_idx(resolve_alias_shape_ty(symbols, ty_idx).unwrap_or(ty_idx))
            .unwrap_or(&Ty::Unknown),
        Ty::EnumRef { .. }
    )
}

fn enum_raw_value_ty(symbols: &dyn UnpackSchema, ty_idx: TyIdx) -> Option<TyIdx> {
    match symbols.ty_by_idx(ty_idx)? {
        Ty::EnumRef { enum_name } => Some(symbols.enum_decl_info(enum_name)?.encoded_as_ty_idx),
        Ty::AliasRef { .. } => enum_raw_value_ty(symbols, symbols.alias_target_for(ty_idx)?.ty_idx),
        _ => None,
    }
}

fn abi_enum_value_name(
    symbols: &dyn UnpackSchema,
    ty_idx: TyIdx,
    data: &UnpackedValue,
) -> Option<String> {
    let Ty::EnumRef { enum_name } = symbols.ty_by_idx(ty_idx)? else {
        return None;
    };
    let enum_ref = symbols.enum_decl_info(enum_name)?;
    enum_ref
        .members
        .iter()
        .find(|member| match data {
            UnpackedValue::Number(value) => member.value == value.to_string(),
            UnpackedValue::Bool(value) => member.value == value.to_string(),
            _ => false,
        })
        .map(|member| format!("{}.{}", enum_ref.name, member.name))
}

fn abi_enum_fallback_value(
    symbols: &dyn UnpackSchema,
    ty_idx: TyIdx,
    data: &UnpackedValue,
) -> String {
    let enum_name = match symbols.ty_by_idx(ty_idx) {
        Some(Ty::EnumRef { enum_name }) => enum_name.as_str(),
        _ => return format!("{data:?}"),
    };
    match data {
        UnpackedValue::Number(value) => format!("{enum_name}({value})"),
        UnpackedValue::Bool(value) => format!("{enum_name}({value})"),
        other => format!("{enum_name}({other:?})"),
    }
}

fn abi_object_context_ty(
    symbols: &dyn UnpackSchema,
    object_name: &str,
    ty_idx: TyIdx,
) -> Option<TyIdx> {
    match symbols.ty_by_idx(ty_idx)? {
        Ty::AliasRef { .. } => {
            let target = symbols.alias_target_for(ty_idx)?.ty_idx;
            abi_object_context_ty(symbols, object_name, target).or(Some(target))
        }
        Ty::Union { variants, .. } => resolve_union_object_ty(symbols, variants, object_name),
        _ => None,
    }
}

fn abi_object_field_ty(
    symbols: &dyn UnpackSchema,
    ty_idx: TyIdx,
    object_name: &str,
    field_name: &str,
) -> Option<TyIdx> {
    match symbols.ty_by_idx(ty_idx)? {
        Ty::StructRef { struct_name: _, .. } => symbols
            .struct_fields_for(ty_idx)?
            .into_iter()
            .find(|field| field.name == field_name)
            .map(|field| field.ty_idx),
        Ty::EnumRef { enum_name } if field_name == "value" => {
            Some(symbols.enum_decl_info(enum_name)?.encoded_as_ty_idx)
        }
        Ty::CellOf { inner_ty_idx } if object_name == "Cell" && field_name == "ref" => {
            Some(*inner_ty_idx)
        }
        Ty::AliasRef { .. } => {
            let target = symbols.alias_target_for(ty_idx)?.ty_idx;
            abi_object_field_ty(symbols, target, object_name, field_name)
        }
        Ty::Union { variants, .. } => {
            for (variant, label) in variants.iter().zip(union_variant_labels(symbols, variants)) {
                if label.as_deref() == Some(object_name) {
                    return if field_name == "value" {
                        Some(variant.variant_ty_idx)
                    } else {
                        abi_object_field_ty(
                            symbols,
                            variant.variant_ty_idx,
                            object_name,
                            field_name,
                        )
                    };
                }
                if union_label_simple(symbols, variant.variant_ty_idx).as_deref()
                    == Some(object_name)
                {
                    return abi_object_field_ty(
                        symbols,
                        variant.variant_ty_idx,
                        object_name,
                        field_name,
                    );
                }
            }
            None
        }
        _ => None,
    }
}

fn resolve_alias_shape_ty(symbols: &dyn UnpackSchema, ty_idx: TyIdx) -> Option<TyIdx> {
    match symbols.ty_by_idx(ty_idx)? {
        Ty::AliasRef { .. } => symbols.alias_target_for(ty_idx).map(|target| target.ty_idx),
        _ => None,
    }
}

fn resolve_union_object_ty(
    symbols: &dyn UnpackSchema,
    variants: &[tolk_compiler::types_kernel::UnionVariant],
    object_name: &str,
) -> Option<TyIdx> {
    let labels = union_variant_labels(symbols, variants);
    variants.iter().zip(labels).find_map(|(variant, label)| {
        if label.as_deref() == Some(object_name)
            || union_label_simple(symbols, variant.variant_ty_idx).as_deref() == Some(object_name)
        {
            Some(variant.variant_ty_idx)
        } else {
            None
        }
    })
}

fn union_variant_labels(
    symbols: &dyn UnpackSchema,
    variants: &[tolk_compiler::types_kernel::UnionVariant],
) -> Vec<Option<String>> {
    let simple_labels = variants
        .iter()
        .map(|variant| union_label_simple(symbols, variant.variant_ty_idx))
        .collect::<Vec<_>>();
    let has_duplicates = simple_labels.iter().enumerate().any(|(idx, label)| {
        label.is_some() && simple_labels[..idx].iter().any(|prev| prev == label)
    });

    variants
        .iter()
        .zip(simple_labels)
        .map(|(variant, simple_label)| {
            if matches!(
                symbols.ty_by_idx(variant.variant_ty_idx),
                Some(Ty::NullLiteral)
            ) {
                Some(String::new())
            } else if has_duplicates {
                Some(render_ty(symbols, variant.variant_ty_idx))
            } else {
                simple_label
            }
        })
        .collect()
}

fn union_label_simple(symbols: &dyn UnpackSchema, ty_idx: TyIdx) -> Option<String> {
    let ty = symbols.ty_by_idx(ty_idx)?;
    Some(match ty {
        Ty::Int => "int".to_owned(),
        Ty::IntN { n } => format!("int{n}"),
        Ty::UintN { n } => format!("uint{n}"),
        Ty::VarintN { n } => format!("varint{n}"),
        Ty::VaruintN { n } => format!("varuint{n}"),
        Ty::Coins => "coins".to_owned(),
        Ty::Bool => "bool".to_owned(),
        Ty::Cell => "cell".to_owned(),
        Ty::Builder => "builder".to_owned(),
        Ty::Slice => "slice".to_owned(),
        Ty::Remaining => "RemainingBitsAndRefs".to_owned(),
        Ty::Address => "address".to_owned(),
        Ty::AddressOpt => "address?".to_owned(),
        Ty::AddressExt => "ext_address".to_owned(),
        Ty::AddressAny => "any_address".to_owned(),
        Ty::BitsN { n } => format!("bits{n}"),
        Ty::NullLiteral => "null".to_owned(),
        Ty::Callable => "callable".to_owned(),
        Ty::Void => "void".to_owned(),
        Ty::Nullable { inner_ty_idx, .. } => {
            format!("{}?", union_label_simple(symbols, *inner_ty_idx)?)
        }
        Ty::CellOf { .. } => "Cell".to_owned(),
        Ty::Tensor { .. } | Ty::ShapedTuple { .. } => "tensor".to_owned(),
        Ty::MapKV { .. } => "map".to_owned(),
        Ty::EnumRef { enum_name } => enum_name.clone(),
        Ty::StructRef { struct_name, .. } => struct_name.clone(),
        Ty::AliasRef { .. } => {
            union_label_simple(symbols, symbols.alias_target_for(ty_idx)?.ty_idx)?
        }
        Ty::GenericT { name_t } => name_t.clone(),
        Ty::Union { variants, .. } => variants
            .iter()
            .filter_map(|variant| union_label_simple(symbols, variant.variant_ty_idx))
            .collect::<Vec<_>>()
            .join("|"),
        Ty::ArrayOf { .. } => "array".to_owned(),
        Ty::LispListOf { .. } => "lisp_list".to_owned(),
        Ty::Unknown => "unknown".to_owned(),
        Ty::String => "string".to_owned(),
    })
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
    symbols: &dyn UnpackSchema,
    value_slice: TyCellSlice<'_>,
    value_ty_idx: TyIdx,
) -> RenderedValue {
    let value_ty = symbols.ty_by_idx(value_ty_idx).unwrap_or(&Ty::Unknown);
    let scalar_type = parse_map_value_type(value_ty);
    let allow_raw_value_fallback =
        scalar_type.is_none() && !matches!(value_ty, Ty::Nullable { .. } | Ty::MapKV { .. });

    let mut value_slice = value_slice;
    if matches!(scalar_type, Some(MapScalarType::Address)) {
        return match IntAddr::load_from(&mut value_slice) {
            Ok(addr) => render_int_address(render_ty(symbols, value_ty_idx), &addr),
            Err(err) => typed_leaf_for_ty(symbols, value_ty_idx, format!("<value: {err}>")),
        };
    }

    if let Some(scalar_type) = scalar_type {
        return match format_map_scalar(&mut value_slice, scalar_type) {
            Ok(value) => typed_leaf_for_ty(symbols, value_ty_idx, value),
            Err(err) => typed_leaf_for_ty(symbols, value_ty_idx, format!("<value: {err}>")),
        };
    }

    if let Some(value) = render_map_value_with_symbols(symbols, value_slice, value_ty_idx) {
        return value;
    }

    if allow_raw_value_fallback {
        return match format_map_raw_value(value_slice) {
            Ok(value) => typed_leaf_for_ty(symbols, value_ty_idx, value),
            Err(err) => typed_leaf_for_ty(symbols, value_ty_idx, format!("<value: {err}>")),
        };
    }

    typed_leaf_for_ty(symbols, value_ty_idx, "<value>")
}

fn render_map_dict(
    symbols: &dyn UnpackSchema,
    root: Option<Cell>,
    key_ty_idx: TyIdx,
    value_ty_idx: TyIdx,
) -> RenderedValue {
    let type_name = format!(
        "map<{}, {}>",
        render_ty(symbols, key_ty_idx),
        render_ty(symbols, value_ty_idx)
    );
    let key_ty = symbols.ty_by_idx(key_ty_idx).unwrap_or(&Ty::Unknown);

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
        let value = render_map_value(symbols, value_slice, value_ty_idx);
        fields.push((key, value));
    }

    RenderedValue::MapKV { type_name, fields }
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
    if let Some(cell) = exact_slice_cell(cs)
        && let Some(rendered) = render_exact_slice_cell(&cell)
    {
        return rendered;
    }

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

fn render_exact_slice_cell(cell: &Cell) -> Option<String> {
    let mut slice = cell.as_slice_allow_exotic();
    let bit_len = slice.size_bits();
    let bit_count = bit_len as usize;
    let ref_count = slice.size_refs() as usize;
    let mut raw_bits = vec![0u8; bit_count.div_ceil(8)];
    slice.load_raw(&mut raw_bits, bit_len).ok()?;

    let mut data_hex = String::with_capacity(raw_bits.len() * 2);
    for byte in raw_bits {
        write!(data_hex, "{byte:02x}").ok()?;
    }

    let nibbles = hex_to_nibbles(&data_hex);
    let rendered = bits_to_hex(&nibbles, 0, bit_count);
    Some(format!("slice{{{rendered}}}{}", refs_suffix(ref_count)))
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

fn typed_leaf_for_ty(
    symbols: &dyn UnpackSchema,
    ty_idx: TyIdx,
    value: impl Into<String>,
) -> RenderedValue {
    RenderedValue::typed_leaf(value, render_ty(symbols, ty_idx))
}

fn typed_leaf(type_name: impl Into<String>, value: impl Into<String>) -> RenderedValue {
    RenderedValue::typed_leaf(value, type_name)
}

/// Toncenter v3 serializes empty dict/null stack values as `list: []`,
/// which the legacy stack parser represents as an empty tuple.
/// TODO: remove if fixed
const fn slot_is_empty_tuple(slot: SlotValue<'_>) -> bool {
    matches!(slot, SlotValue::Live(VmStackValue::Tuple(items)) if items.is_empty())
}

fn type_accepts_empty_toncenter_list_as_null(symbols: &dyn UnpackSchema, ty_idx: TyIdx) -> bool {
    match symbols.ty_by_idx(ty_idx) {
        Some(Ty::Cell | Ty::CellOf { .. } | Ty::MapKV { .. }) => true,
        Some(Ty::AliasRef { .. }) => symbols.alias_target_for(ty_idx).is_some_and(|target| {
            type_accepts_empty_toncenter_list_as_null(symbols, target.ty_idx)
        }),
        _ => false,
    }
}

fn render_empty_map(
    symbols: &dyn UnpackSchema,
    key_ty_idx: TyIdx,
    value_ty_idx: TyIdx,
) -> RenderedValue {
    RenderedValue::MapKV {
        type_name: format!(
            "map<{}, {}>",
            render_ty(symbols, key_ty_idx),
            render_ty(symbols, value_ty_idx)
        ),
        fields: vec![],
    }
}

// ---------------------------------------------------------------------------
// debug_format — recursive type-aware renderer (uses StackReader cursor)
// ---------------------------------------------------------------------------

// Read `ty` from a stack and return formatted representation.
// The returned RenderedValue can be transformed to a plain string, like "Point { x: 10, y: 20 }"
// or to an expandable DAP tree view (for VS Code debugger).
fn debug_format(
    symbols: &dyn UnpackSchema,
    r: &mut StackReader,
    ty_idx: TyIdx,
    un_tuple_if_w: bool,
) -> RenderedValue {
    let ty = symbols.ty_by_idx(ty_idx).unwrap_or(&Ty::Unknown);
    let ty_name = render_ty(symbols, ty_idx);
    let width = calc_width_on_stack(symbols, ty_idx);

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
        let inner = debug_format(symbols, &mut sub, ty_idx, un_tuple_if_w);
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
        return debug_format(symbols, &mut sub, ty_idx, false);
    }

    match ty {
        Ty::Int
        | Ty::IntN { .. }
        | Ty::UintN { .. }
        | Ty::VarintN { .. }
        | Ty::VaruintN { .. }
        | Ty::Coins => match r.read_slot() {
            SlotValue::Live(VmStackValue::Integer(s)) => {
                typed_leaf_for_ty(symbols, ty_idx, s.clone())
            }
            SlotValue::Live(VmStackValue::NaN) => typed_leaf_for_ty(symbols, ty_idx, "NaN"),
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM int"),
        },

        Ty::Bool => match r.read_slot() {
            SlotValue::Live(VmStackValue::Integer(s)) => {
                if s == "0" {
                    typed_leaf_for_ty(symbols, ty_idx, "false")
                } else {
                    typed_leaf_for_ty(symbols, ty_idx, "true")
                }
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM int"),
        },

        Ty::Cell => match r.read_slot() {
            SlotValue::Live(VmStackValue::Cell(cell)) => {
                let (bits, refs, hash) = cell_like_meta(cell);
                render_openable_cell_like(
                    ty_name,
                    render_cell_like(cell),
                    bits,
                    refs,
                    hash,
                    Some(cell.clone()),
                )
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM cell"),
        },

        Ty::CellOf { inner_ty_idx } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Cell(cell)) => {
                render_typed_cell(symbols, ty_name, *inner_ty_idx, cell)
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM cell"),
        },

        Ty::String => match r.read_slot() {
            SlotValue::Live(VmStackValue::String(s)) => {
                typed_leaf_for_ty(symbols, ty_idx, format!("\"{s}\""))
            }
            SlotValue::Live(VmStackValue::Cell(cell)) => typed_leaf_for_ty(
                symbols,
                ty_idx,
                try_parse_string_cell_like(cell)
                    .map_or_else(|| render_cell_like(cell), |string| format!("\"{string}\"")),
            ),
            SlotValue::Live(VmStackValue::CellSlice(cs)) => typed_leaf_for_ty(
                symbols,
                ty_idx,
                try_parse_string_slice(cs)
                    .map_or_else(|| render_slice(cs), |string| format!("\"{string}\"")),
            ),
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM cell"),
        },

        Ty::Builder => match r.read_slot() {
            SlotValue::Live(VmStackValue::Builder(b)) => {
                let cell = CellLike::Builder(b.clone());
                let (bits, refs, hash) = cell_like_meta(&cell);
                render_openable_cell_like(ty_name, render_builder(b), bits, refs, hash, Some(cell))
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM builder"),
        },

        Ty::Slice | Ty::Remaining | Ty::BitsN { .. } => match r.read_slot() {
            // TonCenter can encode get-method slice values as cells in legacy stack JSON.
            SlotValue::Live(VmStackValue::Cell(cell)) => {
                let (bits, refs, hash) = cell_like_meta(cell);
                render_openable_cell_like(
                    ty_name,
                    render_cell_like(cell),
                    bits,
                    refs,
                    hash,
                    Some(cell.clone()),
                )
            }
            SlotValue::Live(VmStackValue::CellSlice(cs)) => {
                let (bits, refs, hash) = slice_meta(cs);
                render_openable_cell_like(
                    ty_name,
                    render_slice(cs),
                    bits,
                    refs,
                    hash,
                    slice_as_cell_like(cs),
                )
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM slice"),
        },

        Ty::ArrayOf { inner_ty_idx } => {
            // array len N => N sub-items => N calls to inner debug_format
            match r.read_slot() {
                SlotValue::Live(VmStackValue::Tuple(t)) => {
                    let as_live: Vec<SlotValue> = t.iter().map(SlotValue::Live).collect();
                    let mut sub = StackReader::new(&as_live);
                    let items: Vec<RenderedValue> = as_live
                        .iter()
                        .map(|_| debug_format(symbols, &mut sub, *inner_ty_idx, true))
                        .collect();
                    RenderedValue::ArrayOf {
                        type_name: ty_name,
                        items,
                    }
                }
                SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
                _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM tuple"),
            }
        }

        Ty::LispListOf { inner_ty_idx } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Tuple(t)) => {
                let elements = flatten_lisp_list(t);
                let as_live: Vec<SlotValue> = elements.iter().map(|v| SlotValue::Live(v)).collect();
                let n = as_live.len();
                let mut sub = StackReader::new(&as_live);
                let items: Vec<RenderedValue> = (0..n)
                    .map(|_| debug_format(symbols, &mut sub, *inner_ty_idx, true))
                    .collect();
                RenderedValue::ArrayOf {
                    type_name: ty_name,
                    items,
                }
            }
            SlotValue::Live(VmStackValue::Null) => RenderedValue::ArrayOf {
                type_name: ty_name,
                items: vec![],
            },
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM tuple"),
        },

        Ty::Address | Ty::AddressOpt | Ty::AddressExt | Ty::AddressAny => match r.read_slot() {
            // TonCenter encodes get-method address values as cells in legacy stack JSON.
            SlotValue::Live(VmStackValue::Cell(cell)) => {
                render_cell_address(symbols, ty_name, ty_idx, cell)
            }
            SlotValue::Live(VmStackValue::CellSlice(cs)) => match try_parse_address(cs) {
                Some(raw) => match raw.parse::<StdAddr>() {
                    Ok(addr) => render_std_address(ty_name, addr.to_string(), &addr),
                    Err(_) => typed_leaf_for_ty(symbols, ty_idx, raw),
                },
                None => typed_leaf_for_ty(symbols, ty_idx, render_slice(cs)),
            },
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM slice"),
        },

        Ty::MapKV {
            key_ty_idx,
            value_ty_idx,
        } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Null) => {
                render_empty_map(symbols, *key_ty_idx, *value_ty_idx)
            }
            SlotValue::Live(VmStackValue::Cell(cell)) => {
                if let Some(root) = decode_cell_like(cell) {
                    render_map_dict(symbols, Some(root), *key_ty_idx, *value_ty_idx)
                } else {
                    typed_leaf_for_ty(symbols, ty_idx, "not a TVM cell")
                }
            }
            slot if slot_is_empty_tuple(slot) => {
                render_empty_map(symbols, *key_ty_idx, *value_ty_idx)
            }
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM cell"),
        },

        Ty::NullLiteral => match r.read_slot() {
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM null"),
        },

        Ty::Void => typed_leaf_for_ty(symbols, ty_idx, "(void)"),

        Ty::Callable => match r.read_slot() {
            SlotValue::Live(VmStackValue::Continuation(_)) => {
                typed_leaf_for_ty(symbols, ty_idx, "continuation")
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM continuation"),
        },
        Ty::Unknown => match r.read_slot() {
            SlotValue::Live(any) => RenderedValue::leaf(any.to_string()),
            _ => RenderedValue::leaf("unreachable"),
        },

        Ty::Nullable {
            inner_ty_idx,
            stack_width,
            ..
        } => {
            if let Some(sw) = stack_width {
                // read wide nullable: [null, null, ... 0] or [smth, smth, ... type_id]
                let nullable_slots = r.read_n_slots(*sw);
                let tag_slot = &nullable_slots[sw - 1];
                match tag_slot {
                    SlotValue::Live(VmStackValue::Integer(type_id))
                    | SlotValue::LastSeen(VmStackValue::Integer(type_id)) => {
                        if type_id == "0" {
                            typed_leaf(ty_name, "null")
                        } else {
                            let mut sub = StackReader::new(&nullable_slots[..sw - 1]);
                            debug_format(symbols, &mut sub, *inner_ty_idx, false)
                        }
                    }
                    SlotValue::OptimizedOut => {
                        let mut sub = StackReader::new(&nullable_slots[..sw - 1]);
                        debug_format(symbols, &mut sub, *inner_ty_idx, false)
                    }
                    _ => typed_leaf_for_ty(symbols, ty_idx, "corrupted stack for nullable"),
                }
            } else {
                // read a primitive one-slot nullable: either TVM null or a value of type inner
                match r.peek_slot() {
                    SlotValue::Live(VmStackValue::Null)
                    | SlotValue::LastSeen(VmStackValue::Null) => {
                        r.read_slot();
                        typed_leaf(ty_name, "null")
                    }
                    slot if slot_is_empty_tuple(slot)
                        && type_accepts_empty_toncenter_list_as_null(symbols, *inner_ty_idx) =>
                    {
                        r.read_slot();
                        typed_leaf(ty_name, "null")
                    }
                    _ => debug_format(symbols, r, *inner_ty_idx, false),
                }
            }
        }

        Ty::StructRef { .. } => {
            let mut fields: Vec<(String, RenderedValue)> = Vec::new();
            for f in symbols.struct_fields_for(ty_idx).unwrap_or_default() {
                let field_val = debug_format(symbols, r, f.ty_idx, false);
                fields.push((f.name, field_val));
            }
            RenderedValue::Struct {
                type_name: ty_name,
                fields,
            }
        }

        Ty::AliasRef { .. } => {
            let Some(target_ty_idx) = symbols.alias_target_for(ty_idx).map(|target| target.ty_idx)
            else {
                return typed_leaf_for_ty(symbols, ty_idx, "unresolved alias");
            };
            debug_format(symbols, r, target_ty_idx, false)
        }

        Ty::EnumRef { enum_name } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Integer(s)) => {
                let Some(enum_ref) = symbols.enum_decl_info(enum_name) else {
                    return typed_leaf_for_ty(symbols, ty_idx, s.clone());
                };
                let text = enum_ref.members.iter().find(|m| &m.value == s).map_or_else(
                    || format!("{}({})", enum_ref.name, s),
                    |m| format!("{}.{}", enum_ref.name, m.name),
                );
                let slot = VmStackValue::Integer(s.clone());
                let slots = [SlotValue::Live(&slot)];
                let mut sub = StackReader::new(&slots);
                let raw_value = debug_format(symbols, &mut sub, enum_ref.encoded_as_ty_idx, false);
                render_enum_value(ty_name, text, raw_value)
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM int"),
        },

        Ty::Tensor { items_ty_idx } => {
            let items: Vec<RenderedValue> = items_ty_idx
                .iter()
                .map(|&item| debug_format(symbols, r, item, false))
                .collect();
            RenderedValue::Tensor {
                type_name: ty_name,
                items,
            }
        }

        Ty::ShapedTuple { items_ty_idx } => match r.read_slot() {
            SlotValue::Live(VmStackValue::Tuple(t)) => {
                let as_live: Vec<SlotValue> = t.iter().map(SlotValue::Live).collect();
                let mut sub = StackReader::new(&as_live);
                let items: Vec<RenderedValue> = items_ty_idx
                    .iter()
                    .map(|&item| debug_format(symbols, &mut sub, item, true))
                    .collect();
                RenderedValue::ArrayOf {
                    type_name: ty_name,
                    items,
                }
            }
            SlotValue::Live(VmStackValue::Null) => typed_leaf_for_ty(symbols, ty_idx, "null"),
            _ => typed_leaf_for_ty(symbols, ty_idx, "not a TVM tuple"),
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
                            return typed_leaf_for_ty(symbols, ty_idx, "corrupted stack for union");
                        };
                        let value = if variant_width == 0 {
                            None
                        } else {
                            let mut sub =
                                StackReader::new(&union_slots[variant_start..stack_width - 1]);
                            Some(debug_format(
                                symbols,
                                &mut sub,
                                variant.variant_ty_idx,
                                false,
                            ))
                        };
                        render_union_case(
                            ty_name,
                            render_ty(symbols, variant.variant_ty_idx),
                            value,
                        )
                    } else {
                        // corrupted stack, type_id on a stack mismatches all variants
                        typed_leaf_for_ty(symbols, ty_idx, "union with unknown variant")
                    }
                }
                SlotValue::OptimizedOut => {
                    // this should not happen in practice, because if UTag for a union was erased during compilation,
                    // a union was definitely smart cast, and its type is narrowed, not Ty::Union
                    typed_leaf_for_ty(symbols, ty_idx, "union with unknown variant")
                }
                _ => typed_leaf_for_ty(symbols, ty_idx, "corrupted stack for union"),
            }
        }

        Ty::Union { .. } => {
            r.read_n_slots(width);
            typed_leaf(ty_name, "union with unresolved layout")
        }

        Ty::GenericT { name_t } => {
            RenderedValue::typed_leaf(format!("unexpected genericT={name_t}"), name_t.clone())
        }
    }
}

pub(crate) fn debug_print_from_stack(
    symbols: &dyn UnpackSchema,
    slots: &[SlotValue],
    ty_idx: TyIdx,
) -> RenderedValue {
    let mut r = StackReader::new(slots);
    debug_format(symbols, &mut r, ty_idx, false)
}

fn tuple_item_to_vm_stack_value(item: &TupleItem) -> VmStackValue {
    match item {
        TupleItem::Null => VmStackValue::Null,
        TupleItem::Int(value) => VmStackValue::Integer(value.to_string()),
        TupleItem::Nan => VmStackValue::NaN,
        TupleItem::Cont(cont) => VmStackValue::Continuation(Boc::encode_hex(&cont.code)),
        TupleItem::Cell(cell) => VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(cell))),
        TupleItem::Slice(cell) => VmStackValue::CellSlice(CellSlice {
            value: Boc::encode_hex(cell),
            bits: None,
            refs: None,
        }),
        TupleItem::Builder(cell) => VmStackValue::Builder(Boc::encode_hex(cell)),
        TupleItem::Tuple(tuple) => VmStackValue::Tuple(tuple_items_to_vm_stack_values(tuple)),
    }
}

fn tuple_items_to_vm_stack_values(tuple: &Tuple) -> Vec<VmStackValue> {
    tuple.iter().map(tuple_item_to_vm_stack_value).collect()
}

#[must_use]
pub fn render_tuple_as_tolk_type(
    symbols: &dyn UnpackSchema,
    tuple: &Tuple,
    ty_idx: TyIdx,
) -> RenderedValue {
    let stack_values = tuple_items_to_vm_stack_values(tuple);
    let slots: Vec<SlotValue<'_>> = stack_values.iter().map(SlotValue::Live).collect();
    debug_print_from_stack(symbols, &slots, ty_idx)
}

pub fn render_tuple_item_as_tolk_type(
    symbols: &SourceMap,
    item: &TupleItem,
    ty_idx: TyIdx,
) -> RenderedValue {
    match item {
        TupleItem::Tuple(tuple) if top_level_tuple_is_stack_frame(symbols, ty_idx) => {
            render_tuple_as_tolk_type(symbols, tuple, ty_idx)
        }
        _ => {
            let stack_value = tuple_item_to_vm_stack_value(item);
            let slots = [SlotValue::Live(&stack_value)];
            debug_print_from_stack(symbols, &slots, ty_idx)
        }
    }
}

fn top_level_tuple_is_stack_frame(symbols: &dyn UnpackSchema, ty_idx: TyIdx) -> bool {
    let Some(ty) = symbols.ty_by_idx(ty_idx) else {
        return false;
    };
    match ty {
        Ty::Tensor { .. } | Ty::StructRef { .. } => true,
        Ty::Nullable {
            inner_ty_idx,
            stack_width,
            ..
        } => {
            stack_width.is_some_and(|w| w != 1)
                || top_level_tuple_is_stack_frame(symbols, *inner_ty_idx)
        }
        Ty::Union {
            stack_width: Some(stack_width),
            ..
        } => *stack_width != 1,
        Ty::AliasRef { .. } => symbols
            .alias_target_for(ty_idx)
            .is_some_and(|target| top_level_tuple_is_stack_frame(symbols, target.ty_idx)),
        _ => false,
    }
}

fn render_lazy_struct_fields(
    symbols: &SourceMap,
    struct_ref: &AbiStruct,
    struct_ty_idx: TyIdx,
    slot_values: &[SlotValue],
    ir_slots: &[usize],
    last_seen: &HashMap<usize, VmStackValue>,
    lazy_cell: Option<&Cell>,
) -> Vec<(String, RenderedValue)> {
    let mut lazy_s = lazy_cell.map(|cell| {
        let mut s = cell.as_slice_allow_exotic();
        if let Some(ref prefix) = struct_ref.prefix {
            let _ = s.skip_first(prefix.prefix_len as u16, 0);
        }
        s
    });

    let mut fields = Vec::new();
    let mut offset = 0;
    let fields_to_render = symbols
        .struct_fields_of(struct_ty_idx)
        .unwrap_or_else(|| struct_ref.fields.clone());
    for f in &fields_to_render {
        let f_width = calc_width_on_stack(symbols, f.ty_idx);
        let field_ir_slots = &ir_slots[offset..offset + f_width];
        let field_ever_seen = field_ir_slots.iter().any(|s| last_seen.contains_key(s));

        let preview = match lazy_s.as_mut() {
            Some(lazy_s) => {
                if let Ok(parsed) = dynamic_unpack::unpack_from_slice(lazy_s, symbols, f.ty_idx) {
                    Some(render_abi_data(symbols, parsed, f.ty_idx))
                } else {
                    None
                }
            }
            None => None,
        };

        let field_val = if field_ever_seen {
            let field_slot_values = &slot_values[offset..offset + f_width];
            let mut r = StackReader::new(field_slot_values);
            debug_format(symbols, &mut r, f.ty_idx, false)
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
    ty_idx: TyIdx,
    last_seen: &HashMap<usize, VmStackValue>,
    lazy_original_slice: &VmStackValue,
) -> RenderedValue {
    let ty = symbols.ty_by_idx(ty_idx).unwrap_or(&Ty::Unknown);
    match ty {
        Ty::Union { .. } => {
            // when a lazy var is still Ty::Union, DEBUG_SMART_CAST not appeared, it's still unresolved
            let type_name = render_ty(symbols, ty_idx);
            RenderedValue::LazyUnresolved { type_name }
        }

        Ty::AliasRef { alias_name, .. } => {
            let Some(resolved_ty_idx) = symbols.alias_target_of(ty_idx) else {
                return RenderedValue::LazyUnresolved {
                    type_name: alias_name.clone(),
                };
            };
            if matches!(symbols.ty_by_idx(resolved_ty_idx), Some(Ty::Union { .. })) {
                return RenderedValue::LazyUnresolved {
                    type_name: alias_name.clone(),
                };
            }
            debug_format_lazy(
                symbols,
                slot_values,
                ir_slots,
                resolved_ty_idx,
                last_seen,
                lazy_original_slice,
            )
        }

        Ty::StructRef { struct_name, .. } => {
            let struct_ref = symbols.get_struct(struct_name);
            let slice_as_cell = match lazy_original_slice {
                VmStackValue::CellSlice(cs) => exact_slice_cell(cs),
                _ => None,
            };

            let fields = match &slice_as_cell {
                Some(cell) => render_lazy_struct_fields(
                    symbols,
                    struct_ref,
                    ty_idx,
                    slot_values,
                    ir_slots,
                    last_seen,
                    Some(cell),
                ),
                None => render_lazy_struct_fields(
                    symbols,
                    struct_ref,
                    ty_idx,
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
            debug_format(symbols, &mut r, ty_idx, false)
        }
    }
}

pub(crate) fn render_runtime_out_actions(
    value: &VmStackValue,
    symbols: &SourceMap,
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
                    .map(|action| render_out_action(action, symbols, abi))
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

fn render_out_action(
    action: &OutAction,
    symbols: &SourceMap,
    abi: Option<&ContractABI>,
) -> RenderedValue {
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
                    let body_meta = resolve_send_message_body_meta(&message, symbols, abi);
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
    symbols: &SourceMap,
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
            symbols,
            abi.outgoing_messages
                .iter()
                .map(|message| message.body_ty_idx),
            prefix_to_skip,
        ),
        RelaxedMsgInfo::ExtOut(_) => try_resolve_message_body(
            body,
            symbols,
            abi.emitted_events.iter().map(|message| message.body_ty_idx),
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

fn try_resolve_message_body<I>(
    body: TyCellSlice,
    symbols: &SourceMap,
    candidates: I,
    prefix_to_skip: u16,
) -> Option<ResolvedDecodedMessageBody>
where
    I: IntoIterator<Item = TyIdx>,
{
    for body_ty_idx in candidates {
        let mut parser = body;
        if prefix_to_skip > 0 && parser.skip_first(prefix_to_skip, 0).is_err() {
            continue;
        }

        let Ok(data) = dynamic_unpack::unpack_from_slice(&mut parser, symbols, body_ty_idx) else {
            continue;
        };
        if parser.size_bits() != 0 || parser.size_refs() != 0 {
            continue;
        }

        let Some(type_name) = compiler_body_type_name(symbols, body_ty_idx) else {
            continue;
        };
        return Some(ResolvedDecodedMessageBody {
            type_name,
            decoded: render_abi_data(symbols, data, body_ty_idx),
        });
    }

    None
}

fn compiler_body_type_name(symbols: &SourceMap, body_ty_idx: TyIdx) -> Option<String> {
    match symbols.ty_by_idx(body_ty_idx)? {
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
            let (bits, refs, hash) = cell_like_meta(cell);
            if bits == Some(0) && refs == Some(0) {
                return RenderedValue::MapKV {
                    type_name: "map<int32, varuint32>".to_owned(),
                    fields: vec![],
                };
            }
            render_openable_cell_like_name(
                "map<int32, varuint32>",
                render_cell_like(cell),
                bits,
                refs,
                hash,
                Some(cell.clone()),
            )
        }
        VmStackValue::Null => RenderedValue::MapKV {
            type_name: "map<int32, varuint32>".to_owned(),
            fields: vec![],
        },
        _ => RenderedValue::typed_leaf(
            render_runtime_vm_value(value).dap_value(),
            "map<int32, varuint32>",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tolk_compiler::abi::{
        ABICustomPackUnpack, ABIDeclaration, ABIOpcode, ABIOutgoingMessage, ABIStorage,
        ABIStructField,
    };
    use tolk_compiler::source_map::{
        AbiAlias, AbiEnum, AbiStruct, Declaration, EnumMemberInfo, FieldInfo, PrefixInfo, SrcRange,
    };
    use tolk_compiler::types_kernel::UnionVariant;
    use tycho_types::cell::{CellDataBuilder, CellFamily, HashBytes, Lazy, Store};
    use tycho_types::dict::{Dict, DictKey, StoreDictKey};
    use tycho_types::error::Error;
    use tycho_types::models::{RelaxedIntMsgInfo, SendMsgFlags};
    use tycho_types::models::{ReserveCurrencyFlags, StdAddr};

    fn source_map_with_declarations_and_types(
        declarations: Vec<Declaration>,
        unique_types: Vec<Ty>,
    ) -> SourceMap {
        serde_json::from_value(serde_json::json!({
            "files": [],
            "declarations": declarations,
            "unique_types": unique_types,
            "struct_instantiations": [],
            "alias_instantiations": [],
            "functions": [],
        }))
        .unwrap()
    }

    fn source_map_with_declarations(declarations: Vec<Declaration>) -> SourceMap {
        source_map_with_declarations_and_types(declarations, Vec::new())
    }

    fn source_map_with_types(unique_types: Vec<Ty>) -> SourceMap {
        source_map_with_declarations_and_types(Vec::new(), unique_types)
    }

    fn empty_symbols() -> SourceMap {
        source_map_with_declarations(Vec::new())
    }

    fn add_ty(unique_types: &mut Vec<Ty>, ty: Ty) -> TyIdx {
        if let Some(idx) = unique_types.iter().position(|existing| existing == &ty) {
            return idx;
        }
        let idx = unique_types.len();
        unique_types.push(ty);
        idx
    }

    fn loc() -> SrcRange {
        SrcRange(vec![0, 0, 0, 0, 0])
    }

    #[test]
    fn pretty_type_name_keeps_generic_punctuation_uncolored() {
        let options = PrettyRenderOptions {
            colorize: true,
            ..Default::default()
        };
        let magenta = |value: &str| value.magenta().to_string();

        assert_eq!(
            pretty_type_name("Cell<DedustSwapParams>", &options),
            format!("{}<{}>", magenta("Cell"), magenta("DedustSwapParams"))
        );
        assert_eq!(
            pretty_type_name("map<uint8, Cell<array<OutAction>>>", &options),
            format!(
                "{}<{}, {}<{}<{}>>>",
                magenta("map"),
                magenta("uint8"),
                magenta("Cell"),
                magenta("array"),
                magenta("OutAction")
            )
        );
        assert_eq!(
            pretty_type_name("Cell<DedustSwapParams>", &PrettyRenderOptions::default()),
            "Cell<DedustSwapParams>"
        );
    }

    #[test]
    fn render_abi_data_uses_field_type_for_object_fields() {
        let mut unique_types = Vec::new();
        let bool_ty_idx = add_ty(&mut unique_types, Ty::Bool);
        let payload_ty_idx = add_ty(
            &mut unique_types,
            Ty::StructRef {
                struct_name: "Payload".to_owned(),
                type_args_ty_idx: None,
            },
        );
        let symbols = source_map_with_declarations_and_types(
            vec![Declaration::Struct(AbiStruct {
                name: "Payload".to_owned(),
                ty_idx: payload_ty_idx,
                ident_loc: loc(),
                type_params: None,
                prefix: None,
                fields: vec![FieldInfo {
                    name: "flag".to_owned(),
                    ty_idx: bool_ty_idx,
                }],
                custom_pack_unpack: None,
            })],
            unique_types,
        );
        let rendered = render_abi_data(
            &symbols,
            UnpackedValue::Object {
                name: "Payload".to_owned(),
                fields: vec![("flag".to_owned(), UnpackedValue::Bool(true))],
            },
            payload_ty_idx,
        );

        let RenderedValue::Struct { fields, .. } = rendered else {
            panic!("expected struct");
        };
        let (_, value) = &fields[0];
        assert_eq!(value.dap_parts().1.as_deref(), Some("bool"));
    }

    #[test]
    fn render_abi_data_uses_container_types_for_tensor_items() {
        let mut unique_types = Vec::new();
        let bool_ty_idx = add_ty(&mut unique_types, Ty::Bool);
        let uint32_ty_idx = add_ty(&mut unique_types, Ty::UintN { n: 32 });
        let tensor_ty_idx = add_ty(
            &mut unique_types,
            Ty::Tensor {
                items_ty_idx: vec![bool_ty_idx, uint32_ty_idx],
            },
        );
        let symbols = source_map_with_types(unique_types);
        let rendered = render_abi_data(
            &symbols,
            UnpackedValue::Array(vec![
                UnpackedValue::Bool(true),
                UnpackedValue::Number(7.into()),
            ]),
            tensor_ty_idx,
        );

        let RenderedValue::ArrayOf { items, .. } = rendered else {
            panic!("expected array");
        };
        assert_eq!(items[0].dap_parts().1.as_deref(), Some("bool"));
        assert_eq!(items[1].dap_parts().1.as_deref(), Some("uint32"));
    }

    #[test]
    fn render_abi_data_renders_maps_as_map_kv() {
        let mut unique_types = Vec::new();
        let key_ty_idx = add_ty(&mut unique_types, Ty::IntN { n: 32 });
        let value_ty_idx = add_ty(&mut unique_types, Ty::Bool);
        let map_ty_idx = add_ty(
            &mut unique_types,
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            },
        );
        let symbols = source_map_with_types(unique_types);
        let rendered = render_abi_data(
            &symbols,
            UnpackedValue::Map(vec![(
                UnpackedValue::Number(1.into()),
                UnpackedValue::Bool(true),
            )]),
            map_ty_idx,
        );

        assert_eq!(rendered.dap_parts().0, "1 entry");
        let RenderedValue::MapKV { type_name, fields } = rendered else {
            panic!("expected MapKV");
        };
        assert_eq!(type_name, "map<int32, bool>");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "1");
        assert_eq!(fields[0].1.dap_parts().0, "true");
        assert_eq!(fields[0].1.dap_parts().1.as_deref(), Some("bool"));
    }

    #[test]
    fn render_map_accepts_toncenter_empty_list_as_empty_map() {
        let mut unique_types = Vec::new();
        let key_ty_idx = add_ty(&mut unique_types, Ty::IntN { n: 32 });
        let value_ty_idx = add_ty(&mut unique_types, Ty::Bool);
        let map_ty_idx = add_ty(
            &mut unique_types,
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            },
        );
        let rendered = render_tuple_item_as_tolk_type(
            &source_map_with_types(unique_types),
            &TupleItem::Tuple(Tuple::empty()),
            map_ty_idx,
        );

        let RenderedValue::MapKV { type_name, fields } = rendered else {
            panic!("expected empty map");
        };
        assert_eq!(type_name, "map<int32, bool>");
        assert!(fields.is_empty());
    }

    #[test]
    fn render_map_with_bits264_key_and_void_value() {
        #[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
        struct Bits264([u8; 33]);

        impl DictKey for Bits264 {
            const BITS: u16 = 264;
        }

        impl StoreDictKey for Bits264 {
            fn store_into_data(&self, data: &mut CellDataBuilder) -> Result<(), Error> {
                data.store_raw(&self.0, Self::BITS)
            }
        }

        let mut unique_types = Vec::new();
        let key_ty_idx = add_ty(&mut unique_types, Ty::BitsN { n: 264 });
        let value_ty_idx = add_ty(&mut unique_types, Ty::Void);
        let map_ty_idx = add_ty(
            &mut unique_types,
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            },
        );
        let mut map = Dict::<Bits264, ()>::new();
        map.set(Bits264([0x11; 33]), ()).unwrap();

        let root = map.into_root().unwrap();
        let stack_value = VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&root)));
        let slots = [SlotValue::Live(&stack_value)];
        let rendered =
            debug_print_from_stack(&source_map_with_types(unique_types), &slots, map_ty_idx);

        let RenderedValue::MapKV { type_name, fields } = rendered else {
            panic!("expected map");
        };
        assert_eq!(type_name, "map<bits264, void>");
        assert_eq!(fields.len(), 1);
        assert_eq!(
            fields[0].0,
            "0x111111111111111111111111111111111111111111111111111111111111111111"
        );
        assert_eq!(fields[0].1.dap_parts().0, "(void)");
        assert_eq!(fields[0].1.dap_parts().1.as_deref(), Some("void"));
    }

    #[test]
    fn render_nullable_cell_accepts_toncenter_empty_list_as_null() {
        let unique_types = vec![
            Ty::Cell,
            Ty::Nullable {
                inner_ty_idx: 0,
                stack_type_id: None,
                stack_width: None,
            },
        ];
        let rendered = render_tuple_item_as_tolk_type(
            &source_map_with_types(unique_types),
            &TupleItem::Tuple(Tuple::empty()),
            1,
        );

        assert_eq!(rendered.dap_parts().0, "null");
        assert_eq!(rendered.dap_parts().1.as_deref(), Some("cell?"));
    }

    #[test]
    fn render_abi_enum_value_exposes_raw_value_field() {
        let mut unique_types = Vec::new();
        let uint8_ty_idx = add_ty(&mut unique_types, Ty::UintN { n: 8 });
        let color_ty_idx = add_ty(
            &mut unique_types,
            Ty::EnumRef {
                enum_name: "Color".to_owned(),
            },
        );
        let symbols = source_map_with_declarations_and_types(
            vec![Declaration::Enum(AbiEnum {
                name: "Color".to_owned(),
                ty_idx: color_ty_idx,
                ident_loc: loc(),
                encoded_as_ty_idx: uint8_ty_idx,
                members: vec![EnumMemberInfo {
                    name: "Blue".to_owned(),
                    value: "2".to_owned(),
                }],
                custom_pack_unpack: None,
            })],
            unique_types,
        );
        let rendered = render_abi_data(&symbols, UnpackedValue::Number(2.into()), color_ty_idx);

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
            "cell",
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
    fn render_empty_cell_like_values_are_compact() {
        let cell = CellBuilder::new().build().unwrap();
        let cell_like = CellLike::Cell(Boc::encode_hex(&cell));
        let (bits, refs, hash) = cell_like_meta(&cell_like);
        let rendered = render_openable_cell_like(
            "cell",
            render_cell_like(&cell_like),
            bits,
            refs,
            hash,
            Some(cell_like),
        );

        assert_eq!(rendered.dap_parts().0, "empty cell");
        assert_eq!(rendered.dap_parts().1.as_deref(), Some("cell"));
        assert!(!rendered.has_children());

        let rendered = render_openable_cell_like("slice", "slice{}", Some(0), Some(0), None, None);
        assert_eq!(rendered.dap_parts().0, "empty slice");
        assert_eq!(rendered.dap_parts().1.as_deref(), Some("slice"));
        assert!(!rendered.has_children());
        assert_eq!(
            rendered.to_pretty_string(PrettyRenderOptions::default()),
            "slice{}"
        );

        let builder_hex = Boc::encode_hex(&cell);
        let cell_like = CellLike::Builder(builder_hex.clone());
        let (bits, refs, hash) = cell_like_meta(&cell_like);
        let rendered = render_openable_cell_like(
            "builder",
            render_builder(&builder_hex),
            bits,
            refs,
            hash,
            Some(cell_like),
        );
        assert_eq!(rendered.dap_parts().0, "empty builder");
        assert_eq!(rendered.dap_parts().1.as_deref(), Some("builder"));
        assert!(!rendered.has_children());
        assert_eq!(
            rendered.to_pretty_string(PrettyRenderOptions::default()),
            "builder{}"
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
        assert_eq!(fields[2].1.dap_parts().0, "{}");
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
        let mut unique_types = Vec::new();
        let storage_ty_idx = add_ty(
            &mut unique_types,
            Ty::StructRef {
                struct_name: "Storage".to_owned(),
                type_args_ty_idx: None,
            },
        );
        let deployment_ty_idx = add_ty(
            &mut unique_types,
            Ty::StructRef {
                struct_name: "DeploymentStorage".to_owned(),
                type_args_ty_idx: None,
            },
        );
        let count_ty_idx = add_ty(&mut unique_types, Ty::UintN { n: 32 });
        let ready_ty_idx = add_ty(&mut unique_types, Ty::Bool);
        let symbols = source_map_with_declarations_and_types(
            vec![
                Declaration::Struct(AbiStruct {
                    name: "Storage".to_owned(),
                    ty_idx: storage_ty_idx,
                    ident_loc: loc(),
                    type_params: None,
                    prefix: None,
                    fields: vec![FieldInfo {
                        name: "count".to_owned(),
                        ty_idx: count_ty_idx,
                    }],
                    custom_pack_unpack: None,
                }),
                Declaration::Struct(AbiStruct {
                    name: "DeploymentStorage".to_owned(),
                    ty_idx: deployment_ty_idx,
                    ident_loc: loc(),
                    type_params: None,
                    prefix: None,
                    fields: vec![FieldInfo {
                        name: "ready".to_owned(),
                        ty_idx: ready_ty_idx,
                    }],
                    custom_pack_unpack: None,
                }),
            ],
            unique_types.clone(),
        );
        let abi = ContractABI {
            unique_types,
            declarations: vec![
                ABIDeclaration::Struct {
                    name: "Storage".to_owned(),
                    ty_idx: storage_ty_idx,
                    type_params: None,
                    prefix: None,
                    fields: vec![ABIStructField {
                        name: "count".to_owned(),
                        ty_idx: count_ty_idx,
                        client_ty_idx: None,
                        default_value: None,
                        description: String::new(),
                    }],
                    custom_pack_unpack: None,
                    description: String::new(),
                },
                ABIDeclaration::Struct {
                    name: "DeploymentStorage".to_owned(),
                    ty_idx: deployment_ty_idx,
                    type_params: None,
                    prefix: None,
                    fields: vec![ABIStructField {
                        name: "ready".to_owned(),
                        ty_idx: ready_ty_idx,
                        client_ty_idx: None,
                        default_value: None,
                        description: String::new(),
                    }],
                    custom_pack_unpack: None,
                    description: String::new(),
                },
            ],
            storage: ABIStorage {
                storage_ty_idx: Some(storage_ty_idx),
                storage_at_deployment_ty_idx: Some(deployment_ty_idx),
            },
            ..Default::default()
        };
        let mut builder = CellBuilder::new();
        builder.store_uint(7, 32).unwrap();
        let cell = builder.build().unwrap();

        let rendered = render_runtime_storage_with_abi(
            &VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&cell))),
            &symbols,
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
    fn render_runtime_storage_maps_abi_type_index_to_source_map_type() {
        let source_storage_ty_idx = 1;
        let source_count_ty_idx = 2;
        let source_types = vec![
            Ty::Void,
            Ty::StructRef {
                struct_name: "Storage".to_owned(),
                type_args_ty_idx: None,
            },
            Ty::UintN { n: 32 },
        ];
        let symbols = source_map_with_declarations_and_types(
            vec![Declaration::Struct(AbiStruct {
                name: "Storage".to_owned(),
                ty_idx: source_storage_ty_idx,
                ident_loc: loc(),
                type_params: None,
                prefix: None,
                fields: vec![FieldInfo {
                    name: "count".to_owned(),
                    ty_idx: source_count_ty_idx,
                }],
                custom_pack_unpack: None,
            })],
            source_types,
        );

        let abi_count_ty_idx = 1;
        let abi_storage_ty_idx = 2;
        let abi = ContractABI {
            unique_types: vec![
                Ty::Void,
                Ty::UintN { n: 32 },
                Ty::StructRef {
                    struct_name: "Storage".to_owned(),
                    type_args_ty_idx: None,
                },
            ],
            declarations: vec![ABIDeclaration::Struct {
                name: "Storage".to_owned(),
                ty_idx: abi_storage_ty_idx,
                type_params: None,
                prefix: None,
                fields: vec![ABIStructField {
                    name: "count".to_owned(),
                    ty_idx: abi_count_ty_idx,
                    client_ty_idx: None,
                    default_value: None,
                    description: String::new(),
                }],
                custom_pack_unpack: None,
                description: String::new(),
            }],
            storage: ABIStorage {
                storage_ty_idx: Some(abi_storage_ty_idx),
                storage_at_deployment_ty_idx: None,
            },
            ..Default::default()
        };
        let mut builder = CellBuilder::new();
        builder.store_uint(7, 32).unwrap();
        let cell = builder.build().unwrap();

        let rendered = render_runtime_storage_with_abi(
            &VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&cell))),
            &symbols,
            &abi,
        )
        .expect("expected decoded c4");

        let RenderedValue::CellOf {
            type_name, fields, ..
        } = rendered
        else {
            panic!("expected CellOf");
        };
        assert_eq!(type_name, "Cell<Storage>");
        assert_eq!(fields[0].0, "decoded");

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
    fn render_typed_cell_decodes_builtin_tlb_varuint_custom_aliases() {
        let mut unique_types = Vec::new();
        let int_ty_idx = add_ty(&mut unique_types, Ty::Int);
        let varuint7_ty_idx = add_ty(
            &mut unique_types,
            Ty::AliasRef {
                alias_name: "TlbVarUint7".to_owned(),
                type_args_ty_idx: None,
            },
        );
        let varuint3_ty_idx = add_ty(
            &mut unique_types,
            Ty::AliasRef {
                alias_name: "TlbVarUint3".to_owned(),
                type_args_ty_idx: None,
            },
        );
        let gas_record_ty_idx = add_ty(
            &mut unique_types,
            Ty::StructRef {
                struct_name: "GasRecord".to_owned(),
                type_args_ty_idx: None,
            },
        );
        let custom_pack_unpack = Some(ABICustomPackUnpack {
            pack_to_builder: Some(true),
            unpack_from_slice: Some(true),
        });
        let symbols = source_map_with_declarations_and_types(
            vec![
                Declaration::Alias(AbiAlias {
                    name: "TlbVarUint7".to_owned(),
                    ty_idx: varuint7_ty_idx,
                    ident_loc: loc(),
                    target_ty_idx: int_ty_idx,
                    type_params: None,
                    custom_pack_unpack: custom_pack_unpack.clone(),
                }),
                Declaration::Alias(AbiAlias {
                    name: "TlbVarUint3".to_owned(),
                    ty_idx: varuint3_ty_idx,
                    ident_loc: loc(),
                    target_ty_idx: int_ty_idx,
                    type_params: None,
                    custom_pack_unpack,
                }),
                Declaration::Struct(AbiStruct {
                    name: "GasRecord".to_owned(),
                    ty_idx: gas_record_ty_idx,
                    ident_loc: loc(),
                    type_params: None,
                    prefix: None,
                    fields: vec![
                        FieldInfo {
                            name: "gasUsed".to_owned(),
                            ty_idx: varuint7_ty_idx,
                        },
                        FieldInfo {
                            name: "gasCredit".to_owned(),
                            ty_idx: varuint3_ty_idx,
                        },
                    ],
                    custom_pack_unpack: None,
                }),
            ],
            unique_types,
        );
        let mut builder = CellBuilder::new();
        builder.store_uint(1, 3).unwrap();
        builder.store_uint(118, 8).unwrap();
        builder.store_uint(2, 2).unwrap();
        builder.store_uint(1024, 16).unwrap();
        let cell = builder.build().unwrap();
        let rendered = render_typed_cell(
            &symbols,
            "Cell<GasRecord>".to_owned(),
            gas_record_ty_idx,
            &CellLike::Cell(Boc::encode_hex(&cell)),
        );

        let RenderedValue::CellOf { fields, .. } = rendered else {
            panic!("expected CellOf");
        };
        assert_eq!(fields[0].0, "decoded");
        let RenderedValue::Struct {
            type_name,
            fields: decoded_fields,
        } = &fields[0].1
        else {
            panic!("expected decoded cell");
        };
        assert_eq!(type_name, "GasRecord");
        assert_eq!(decoded_fields[0].0, "gasUsed");
        assert_eq!(decoded_fields[0].1.dap_parts().0, "118");
        assert_eq!(
            decoded_fields[0].1.dap_parts().1.as_deref(),
            Some("TlbVarUint7")
        );
        assert_eq!(decoded_fields[1].0, "gasCredit");
        assert_eq!(decoded_fields[1].1.dap_parts().0, "1024");
        assert_eq!(
            decoded_fields[1].1.dap_parts().1.as_deref(),
            Some("TlbVarUint3")
        );
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

        let rendered = debug_print_from_stack(&source_map_with_types(vec![Ty::Address]), &slots, 0);

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
    fn render_address_any_none_from_slice_uses_addr_none() {
        let mut builder = CellBuilder::new();
        AnyAddr::None
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let none_cell = builder.build().unwrap();
        let stack_values = [VmStackValue::CellSlice(CellSlice {
            value: Boc::encode_hex(&none_cell),
            bits: None,
            refs: None,
        })];
        let slots = [SlotValue::Live(&stack_values[0])];

        let rendered =
            debug_print_from_stack(&source_map_with_types(vec![Ty::AddressAny]), &slots, 0);

        assert_eq!(
            rendered.dap_parts(),
            ("addr_none".to_owned(), Some("any_address".to_owned()))
        );
    }

    #[test]
    fn render_stack_enum_value_exposes_raw_value_field() {
        let mut symbols_json = serde_json::json!({
            "files": [],
            "declarations": [{
                "kind": "enum",
                "name": "Color",
                "ident_loc": [0, 0, 0, 0, 0],
                "ty_idx": 1,
                "encoded_as_ty_idx": 0,
                "members": [
                    {"name": "Red", "value": "1"},
                    {"name": "Blue", "value": "2"}
                ]
            }],
            "unique_types": [
                {"kind": "uintN", "n": 8},
                {"kind": "EnumRef", "enum_name": "Color"}
            ],
            "struct_instantiations": [],
            "alias_instantiations": [],
            "functions": [],
            "debug_marks": []
        });
        let symbols: SourceMap = serde_json::from_value(symbols_json.take()).unwrap();

        let stack_values = [VmStackValue::Integer("2".to_owned())];
        let slots = [SlotValue::Live(&stack_values[0])];
        let rendered = debug_print_from_stack(&symbols, &slots, 1);

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
        let unique_types = vec![
            Ty::Cell,
            Ty::Int,
            Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty_idx: 0,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: Some(1),
                        stack_width: Some(1),
                    },
                    UnionVariant {
                        variant_ty_idx: 1,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: Some(2),
                        stack_width: Some(1),
                    },
                ],
                stack_width: Some(2),
            },
        ];
        let stack_values = [
            VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&cell))),
            VmStackValue::Integer("1".to_owned()),
        ];
        let slots = [
            SlotValue::Live(&stack_values[0]),
            SlotValue::Live(&stack_values[1]),
        ];

        let rendered = debug_print_from_stack(&source_map_with_types(unique_types), &slots, 2);
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
        let RenderedValue::CellLike { value, fields, .. } = &fields[0].1 else {
            panic!("expected nested CellLike");
        };
        assert_eq!(value, "empty cell");
        assert!(fields.is_empty());
    }

    #[test]
    fn render_null_union_variant_has_no_value_field() {
        let unique_types = vec![
            Ty::NullLiteral,
            Ty::Bool,
            Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty_idx: 0,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: Some(0),
                        stack_width: Some(0),
                    },
                    UnionVariant {
                        variant_ty_idx: 1,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: Some(1),
                        stack_width: Some(1),
                    },
                ],
                stack_width: Some(2),
            },
        ];
        let stack_values = [VmStackValue::Null, VmStackValue::Integer("0".to_owned())];
        let slots = [
            SlotValue::Live(&stack_values[0]),
            SlotValue::Live(&stack_values[1]),
        ];

        let rendered = debug_print_from_stack(&source_map_with_types(unique_types), &slots, 2);

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
    fn render_top_level_nullable_map_unpacks_tuple_as_stack_frame() {
        let unique_types = vec![
            Ty::IntN { n: 32 },
            Ty::MapKV {
                key_ty_idx: 0,
                value_ty_idx: 0,
            },
            Ty::Nullable {
                inner_ty_idx: 1,
                stack_type_id: Some(1),
                stack_width: Some(2),
            },
        ];
        let present_empty_map =
            TupleItem::Tuple(Tuple(vec![TupleItem::Null, TupleItem::Int(1.into())]));
        let rendered = render_tuple_item_as_tolk_type(
            &source_map_with_types(unique_types),
            &present_empty_map,
            2,
        );

        let RenderedValue::MapKV { type_name, fields } = rendered else {
            panic!("expected present empty map to render as MapKV");
        };
        assert_eq!(type_name, "map<int32, int32>");
        assert!(fields.is_empty());
    }

    #[test]
    fn render_empty_map_dap_parts_show_empty_value_and_type() {
        let rendered = RenderedValue::MapKV {
            type_name: "map<int32, int32>".to_owned(),
            fields: vec![],
        };

        let (value, type_field) = rendered.dap_parts();
        assert_eq!(value, "{}");
        assert_eq!(type_field.as_deref(), Some("map<int32, int32>"));
        assert_eq!(rendered.legacy_dap_value(Some("balances")), "{}");
    }

    #[test]
    fn render_top_level_nullable_map_uses_zero_tag_for_null() {
        let unique_types = vec![
            Ty::IntN { n: 32 },
            Ty::MapKV {
                key_ty_idx: 0,
                value_ty_idx: 0,
            },
            Ty::Nullable {
                inner_ty_idx: 1,
                stack_type_id: Some(1),
                stack_width: Some(2),
            },
        ];
        let null_map = TupleItem::Tuple(Tuple(vec![TupleItem::Null, TupleItem::Int(0.into())]));
        let rendered =
            render_tuple_item_as_tolk_type(&source_map_with_types(unique_types), &null_map, 2);

        assert_eq!(rendered.dap_parts().0, "null");
        assert_eq!(
            rendered.dap_parts().1.as_deref(),
            Some("map<int32, int32>?")
        );
    }

    #[test]
    fn render_nan_as_int_like_value() {
        let rendered = render_tuple_item_as_tolk_type(
            &source_map_with_types(vec![Ty::Int]),
            &TupleItem::Nan,
            0,
        );

        assert_eq!(rendered.dap_parts().0, "NaN");
        assert_eq!(rendered.dap_parts().1.as_deref(), Some("int"));
    }

    #[test]
    fn render_union_without_stack_width_falls_back_without_panic() {
        let unique_types = vec![
            Ty::Int,
            Ty::Cell,
            Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty_idx: 0,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: Some(1),
                        stack_width: Some(1),
                    },
                    UnionVariant {
                        variant_ty_idx: 1,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: Some(2),
                        stack_width: Some(1),
                    },
                ],
                stack_width: None,
            },
        ];
        let stack_values = [VmStackValue::Integer("7".to_owned())];
        let slots = [SlotValue::Live(&stack_values[0])];

        let rendered = debug_print_from_stack(&source_map_with_types(unique_types), &slots, 2);

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
            "slice",
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
            "builder",
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
            &empty_symbols(),
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
        let transfer_ty_idx = 0;
        let unique_types = vec![Ty::StructRef {
            struct_name: "Transfer".to_owned(),
            type_args_ty_idx: None,
        }];
        let symbols = source_map_with_declarations_and_types(
            vec![Declaration::Struct(AbiStruct {
                name: "Transfer".to_owned(),
                ty_idx: transfer_ty_idx,
                ident_loc: loc(),
                type_params: None,
                prefix: Some(PrefixInfo {
                    prefix_num: 0xfeedbeef,
                    prefix_len: 32,
                }),
                fields: vec![],
                custom_pack_unpack: None,
            })],
            unique_types.clone(),
        );
        let abi = ContractABI {
            unique_types,
            declarations: vec![ABIDeclaration::Struct {
                name: "Transfer".to_owned(),
                ty_idx: transfer_ty_idx,
                type_params: None,
                prefix: Some(ABIOpcode {
                    prefix_num: 0xfeedbeef,
                    prefix_len: 32,
                }),
                fields: vec![],
                custom_pack_unpack: None,
                description: String::new(),
            }],
            outgoing_messages: vec![ABIOutgoingMessage {
                body_ty_idx: transfer_ty_idx,
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
            &symbols,
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
