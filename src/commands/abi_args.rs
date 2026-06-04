use crate::ffi;
use acton_config::color::OwoColorize;
use anyhow::anyhow;
use num_bigint::BigInt;
use tolk_compiler::abi::{ABIFunctionParameter, ContractABI, Ty};
use tolk_compiler::types_kernel::TyIdx;
use tolk_syntax::ast::expressions::parse_tolk_int_literal;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::models::{StdAddr, StdAddrFormat};

pub fn parse_main_stack_args(abi: Option<&ContractABI>, args: &[String]) -> anyhow::Result<Tuple> {
    let Some(abi) = abi else {
        if args.is_empty() {
            return Ok(Tuple::empty());
        }
        anyhow::bail!("Cannot parse script arguments: missing ABI");
    };
    let Some(main) = abi
        .get_methods
        .iter()
        .find(|method| method.tvm_method_id == 0)
    else {
        if args.is_empty() {
            return Ok(Tuple::empty());
        }
        anyhow::bail!("Cannot parse script arguments: main function ABI was not found");
    };

    parse_abi_parameters(abi, &main.parameters, args)
}

pub fn parse_abi_parameters(
    abi: &ContractABI,
    parameters: &[ABIFunctionParameter],
    args: &[String],
) -> anyhow::Result<Tuple> {
    let expected_count = parameters.len();
    if args.len() != expected_count {
        anyhow::bail!(
            "Wrong number of arguments: expected {}, got {}",
            expected_count,
            args.len()
        );
    }

    let mut items = Vec::with_capacity(args.len());
    for (param, arg) in parameters.iter().zip(args) {
        items.push(parse_abi_parameter(abi, param, arg)?);
    }

    Ok(Tuple(items))
}

fn parse_abi_parameter(
    abi: &ContractABI,
    param: &ABIFunctionParameter,
    raw: &str,
) -> anyhow::Result<TupleItem> {
    validate_script_arg_ty(abi, param.ty_idx)
        .and_then(|()| parse_script_value(abi, param.ty_idx, raw))
        .map_err(|err| match err {
            ScriptArgParseError::Invalid => {
                anyhow!(
                    "Cannot parse argument {} as {}: {}",
                    param.name.yellow(),
                    abi.render_type(param.ty_idx).yellow(),
                    format_arg_value(raw).yellow()
                )
            }
            ScriptArgParseError::Unsupported { kind, ty }
                if unsupported_type_message_needs_name(&ty) =>
            {
                anyhow!(
                    "Argument {} has unsupported {} type {}",
                    param.name.yellow(),
                    kind.yellow(),
                    ty.yellow()
                )
            }
            ScriptArgParseError::Unsupported { kind, .. } => {
                anyhow!(
                    "Argument {} has unsupported {} type",
                    param.name.yellow(),
                    kind.yellow()
                )
            }
        })
}

#[derive(Debug)]
enum ScriptArgParseError {
    Invalid,
    Unsupported { kind: &'static str, ty: String },
}

fn validate_script_arg_ty(abi: &ContractABI, ty_idx: TyIdx) -> Result<(), ScriptArgParseError> {
    let Some(ty) = abi.ty_by_idx(ty_idx) else {
        return Err(ScriptArgParseError::Unsupported {
            kind: "argument",
            ty: format!("ty#{ty_idx}"),
        });
    };
    match ty {
        Ty::Nullable { inner_ty_idx, .. } | Ty::ArrayOf { inner_ty_idx } => {
            validate_script_arg_ty(abi, *inner_ty_idx)
        }
        Ty::Int
        | Ty::IntN { .. }
        | Ty::UintN { .. }
        | Ty::VarintN { .. }
        | Ty::VaruintN { .. }
        | Ty::Coins
        | Ty::BitsN { .. }
        | Ty::Bool
        | Ty::Cell
        | Ty::CellOf { .. }
        | Ty::Slice
        | Ty::String
        | Ty::Address
        | Ty::AddressExt
        | Ty::AddressOpt
        | Ty::NullLiteral => Ok(()),
        _ => Err(unsupported_ty(abi, ty_idx)),
    }
}

fn parse_script_value(
    abi: &ContractABI,
    ty_idx: TyIdx,
    raw: &str,
) -> Result<TupleItem, ScriptArgParseError> {
    let Some(ty) = abi.ty_by_idx(ty_idx) else {
        return Err(ScriptArgParseError::Unsupported {
            kind: "argument",
            ty: format!("ty#{ty_idx}"),
        });
    };
    let trimmed = raw.trim();
    match ty {
        Ty::String if !trimmed.starts_with('"') => string_tuple_item(raw),
        Ty::Nullable { .. } if trimmed == "null" => Ok(TupleItem::Null),
        Ty::Nullable { inner_ty_idx, .. } => parse_script_value(abi, *inner_ty_idx, raw),
        _ => {
            let mut parser = ScriptArgParser::new(abi, raw);
            let item = parser.parse_value(ty_idx)?;
            parser.skip_ws();
            if parser.is_eof() {
                Ok(item)
            } else {
                Err(ScriptArgParseError::Invalid)
            }
        }
    }
}

struct ScriptArgParser<'a> {
    abi: &'a ContractABI,
    input: &'a str,
    pos: usize,
}

impl<'a> ScriptArgParser<'a> {
    const fn new(abi: &'a ContractABI, input: &'a str) -> Self {
        Self { abi, input, pos: 0 }
    }

    const fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.rest().chars().next()
            && ch.is_whitespace()
        {
            self.pos += ch.len_utf8();
        }
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.input.as_bytes().get(self.pos).copied() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_value(&mut self, ty_idx: TyIdx) -> Result<TupleItem, ScriptArgParseError> {
        self.skip_ws();
        let Some(ty) = self.abi.ty_by_idx(ty_idx) else {
            return Err(ScriptArgParseError::Unsupported {
                kind: "argument",
                ty: format!("ty#{ty_idx}"),
            });
        };
        match ty {
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins => self.parse_int(),
            Ty::Bool => self.parse_bool(),
            Ty::Cell | Ty::CellOf { .. } => self.parse_cell().map(TupleItem::Cell),
            Ty::Slice | Ty::BitsN { .. } => self.parse_cell().map(TupleItem::Slice),
            Ty::String => self.parse_string(),
            Ty::Address | Ty::AddressExt => self.parse_address(),
            Ty::AddressOpt => {
                if self.consume_exact_token("null") {
                    Ok(TupleItem::Null)
                } else {
                    self.parse_address()
                }
            }
            Ty::NullLiteral => {
                if self.consume_exact_token("null") {
                    Ok(TupleItem::Null)
                } else {
                    Err(ScriptArgParseError::Invalid)
                }
            }
            Ty::ArrayOf { inner_ty_idx } => self.parse_array(*inner_ty_idx),
            _ => Err(unsupported_ty(self.abi, ty_idx)),
        }
    }

    fn parse_int(&mut self) -> Result<TupleItem, ScriptArgParseError> {
        let token = self.parse_token()?;
        if token == "NaN" {
            return Ok(TupleItem::Nan);
        }
        let value = parse_number(token).ok_or(ScriptArgParseError::Invalid)?;
        Ok(TupleItem::Int(value))
    }

    fn parse_bool(&mut self) -> Result<TupleItem, ScriptArgParseError> {
        let token = self.parse_token()?;
        match token {
            "true" => Ok(TupleItem::Int(BigInt::from(-1))),
            "false" => Ok(TupleItem::Int(BigInt::from(0))),
            _ => Err(ScriptArgParseError::Invalid),
        }
    }

    fn parse_cell(&mut self) -> Result<Cell, ScriptArgParseError> {
        let token = self.parse_token()?;
        Boc::decode_hex(token).map_err(|_| ScriptArgParseError::Invalid)
    }

    fn parse_string(&mut self) -> Result<TupleItem, ScriptArgParseError> {
        if self.rest().starts_with('"') {
            let value = self.parse_quoted_string()?;
            string_tuple_item(&value)
        } else {
            let value = self.parse_token()?;
            string_tuple_item(value)
        }
    }

    fn parse_address(&mut self) -> Result<TupleItem, ScriptArgParseError> {
        let token = self.parse_token()?;
        address_tuple_item(token)
    }

    fn parse_array(&mut self, inner_ty_idx: TyIdx) -> Result<TupleItem, ScriptArgParseError> {
        if !self.consume_byte(b'[') {
            return Err(ScriptArgParseError::Invalid);
        }

        let mut items = Vec::new();
        loop {
            self.skip_ws();
            if self.consume_byte(b']') {
                return Ok(TupleItem::Tuple(Tuple(items)));
            }
            if self.is_eof() {
                return Err(ScriptArgParseError::Invalid);
            }

            items.push(self.parse_value(inner_ty_idx)?);

            self.skip_ws();
            self.consume_byte(b',');
        }
    }

    fn parse_quoted_string(&mut self) -> Result<String, ScriptArgParseError> {
        if !self.consume_byte(b'"') {
            return Err(ScriptArgParseError::Invalid);
        }

        let mut value = String::new();
        while !self.is_eof() {
            let ch = self.next_char().ok_or(ScriptArgParseError::Invalid)?;
            match ch {
                '"' => return Ok(value),
                '\n' | '\r' => return Err(ScriptArgParseError::Invalid),
                '\\' => {
                    let escaped = self.next_char().ok_or(ScriptArgParseError::Invalid)?;
                    match escaped {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '0' => value.push('\0'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        other => value.push(other),
                    }
                }
                other => value.push(other),
            }
        }

        Err(ScriptArgParseError::Invalid)
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.rest().chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn parse_token(&mut self) -> Result<&'a str, ScriptArgParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.rest().chars().next() {
            if ch.is_whitespace() || matches!(ch, ',' | ']') {
                break;
            }
            self.pos += ch.len_utf8();
        }
        if self.pos == start {
            Err(ScriptArgParseError::Invalid)
        } else {
            Ok(&self.input[start..self.pos])
        }
    }

    fn consume_exact_token(&mut self, expected: &str) -> bool {
        let start = self.pos;
        match self.parse_token() {
            Ok(token) if token == expected => true,
            _ => {
                self.pos = start;
                false
            }
        }
    }
}

fn parse_number(raw: &str) -> Option<BigInt> {
    let (negative, raw) = raw
        .strip_prefix('-')
        .map_or((false, raw), |raw| (true, raw));
    let literal = parse_tolk_int_literal(raw)?;
    let mut value = BigInt::parse_bytes(literal.digits().as_bytes(), literal.radix())?;
    if negative {
        value = -value;
    }
    Some(value)
}

fn string_tuple_item(value: &str) -> Result<TupleItem, ScriptArgParseError> {
    let mut tuple = Tuple::empty();
    tuple.push_string_slice(value);
    tuple.0.pop().ok_or(ScriptArgParseError::Invalid)
}

fn address_tuple_item(value: &str) -> Result<TupleItem, ScriptArgParseError> {
    let (addr, _) = StdAddr::from_str_ext(
        ffi::emulation::normalize_address_input(value),
        StdAddrFormat::any(),
    )
    .map_err(|_| ScriptArgParseError::Invalid)?;
    let mut builder = CellBuilder::new();
    addr.store_into(&mut builder, Cell::empty_context())
        .map_err(|_| ScriptArgParseError::Invalid)?;
    builder
        .build()
        .map(TupleItem::Slice)
        .map_err(|_| ScriptArgParseError::Invalid)
}

fn unsupported_ty(abi: &ContractABI, ty_idx: TyIdx) -> ScriptArgParseError {
    ScriptArgParseError::Unsupported {
        kind: abi
            .ty_by_idx(ty_idx)
            .map_or("argument", unsupported_ty_kind),
        ty: abi.render_type(ty_idx),
    }
}

fn unsupported_ty_kind(ty: &Ty) -> &'static str {
    match ty {
        Ty::AliasRef { alias_name, .. } if alias_name == "tuple" => "tuple",
        Ty::AliasRef { alias_name, .. } if alias_name == "dict" => "dict",
        Ty::AliasRef { .. } => "alias",
        Ty::StructRef { .. } => "struct",
        Ty::Union { .. } => "union",
        Ty::MapKV { .. } => "map",
        Ty::LispListOf { .. } => "lisp list",
        Ty::Tensor { .. } | Ty::ShapedTuple { .. } => "tuple",
        Ty::GenericT { .. } => "generic",
        Ty::Callable => "continuation",
        Ty::Builder => "builder",
        Ty::AddressAny => "any_address",
        Ty::Remaining => "remaining",
        Ty::Void => "void",
        Ty::EnumRef { .. } => "enum",
        Ty::Unknown => "unknown",
        _ => "argument",
    }
}

fn unsupported_type_message_needs_name(ty: &str) -> bool {
    !matches!(
        ty,
        "builder" | "dict" | "tuple" | "continuation" | "unknown" | "void"
    )
}

fn format_arg_value(raw: &str) -> String {
    const MAX_LEN: usize = 120;

    let value = format!("{raw:?}");
    if value.chars().count() <= MAX_LEN {
        value
    } else {
        let shortened = value.chars().take(MAX_LEN).collect::<String>();
        format!("{shortened}...")
    }
}
