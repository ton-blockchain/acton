use crate::hashes::{crc16, crc32};
use num_bigint::{BigInt, Sign};
use num_traits::{ToPrimitive, Zero};
use rustc_hash::{FxHashMap, FxHashSet};
use sha2::{Digest, Sha256};
use tolk_resolver::{
    AstNodeSpanExt, FileDb, FileId, ProjectIndex, Resolved, Span, SymbolId, SymbolKind,
};
use tolk_syntax::ast::expressions::{Expr, parse_tolk_int_literal};
use tolk_syntax::{AstNode, TopLevel};

pub trait ConstantEvaluationContext {
    fn file_db(&self) -> &FileDb;

    fn project_index(&self) -> &ProjectIndex;

    fn resolve_at(&self, file_id: FileId, span: Span) -> Option<Resolved>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstantValue {
    Int(BigInt),
    Bool(bool),
    String(String),
    Overflow,
    Unknown,
}

impl ConstantValue {
    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    #[must_use]
    pub fn format(&self) -> String {
        match self {
            Self::Int(value) if value.is_zero() => "0".to_owned(),
            Self::Int(value) if value.sign() != Sign::Minus && value.bits() <= 32 => {
                format!("{value} (0x{})", value.to_str_radix(16).to_uppercase())
            }
            Self::Int(value) => format!("0x{}", value.to_str_radix(16).to_uppercase()),
            Self::Bool(value) => value.to_string(),
            Self::String(value) => format!("{value:?}"),
            Self::Overflow => "overflow".to_owned(),
            Self::Unknown => "unknown".to_owned(),
        }
    }
}

pub struct ConstantEvaluator<'a> {
    context: &'a dyn ConstantEvaluationContext,
    constant_stack: FxHashSet<SymbolId>,
    constant_cache: FxHashMap<SymbolId, ConstantValue>,
    enum_stack: FxHashSet<SymbolId>,
    enum_cache: FxHashMap<SymbolId, FxHashMap<SymbolId, ConstantValue>>,
}

impl<'a> ConstantEvaluator<'a> {
    #[must_use]
    pub fn new(context: &'a dyn ConstantEvaluationContext) -> Self {
        Self {
            context,
            constant_stack: FxHashSet::default(),
            constant_cache: FxHashMap::default(),
            enum_stack: FxHashSet::default(),
            enum_cache: FxHashMap::default(),
        }
    }

    pub fn evaluate_constant(&mut self, symbol_id: SymbolId) -> ConstantValue {
        if let Some(value) = self.constant_cache.get(&symbol_id) {
            return value.clone();
        }
        if !self.constant_stack.insert(symbol_id) {
            return ConstantValue::Unknown;
        }

        let value = self.evaluate_constant_inner(symbol_id);
        self.constant_stack.remove(&symbol_id);
        self.constant_cache.insert(symbol_id, value.clone());
        value
    }

    pub fn evaluate_enum_member(&mut self, symbol_id: SymbolId) -> ConstantValue {
        let owner_id = {
            let Some(file_index) = self
                .context
                .project_index()
                .get_file_index(symbol_id.file_id)
            else {
                return ConstantValue::Unknown;
            };
            let Some(owner) = file_index.decls.iter().find(|symbol| {
                matches!(&symbol.kind, SymbolKind::Enum { members } if members.iter().any(|member| member.id == symbol_id))
            }) else {
                return ConstantValue::Unknown;
            };
            owner.id
        };
        self.populate_enum_cache(owner_id);
        self.enum_cache
            .get(&owner_id)
            .and_then(|values| values.get(&symbol_id))
            .cloned()
            .unwrap_or(ConstantValue::Unknown)
    }

    pub fn evaluate_enum_values(
        &mut self,
        symbol_id: SymbolId,
    ) -> Option<&FxHashMap<SymbolId, ConstantValue>> {
        self.populate_enum_cache(symbol_id);
        self.enum_cache.get(&symbol_id)
    }

    fn populate_enum_cache(&mut self, symbol_id: SymbolId) {
        if self.enum_cache.contains_key(&symbol_id) || !self.enum_stack.insert(symbol_id) {
            return;
        }
        let values = self.evaluate_enum_inner(symbol_id);
        self.enum_stack.remove(&symbol_id);
        self.enum_cache.insert(symbol_id, values);
    }

    fn evaluate_constant_inner(&mut self, symbol_id: SymbolId) -> ConstantValue {
        let Some(file) = self.context.file_db().get_by_id(symbol_id.file_id) else {
            return ConstantValue::Unknown;
        };
        let Some(TopLevel::Constant(constant)) = file.find_syntax_declaration(symbol_id) else {
            return ConstantValue::Unknown;
        };
        let Some(expression) = constant.value() else {
            return ConstantValue::Unknown;
        };
        self.evaluate_expression(symbol_id.file_id, expression, file.source().source.as_ref())
    }

    fn evaluate_enum_inner(&mut self, symbol_id: SymbolId) -> FxHashMap<SymbolId, ConstantValue> {
        let Some(symbol) = self
            .context
            .project_index()
            .resolve_symbol(symbol_id)
            .cloned()
        else {
            return FxHashMap::default();
        };
        let SymbolKind::Enum { members } = symbol.kind else {
            return FxHashMap::default();
        };
        let Some(file) = self.context.file_db().get_by_id(symbol_id.file_id) else {
            return FxHashMap::default();
        };
        let Some(TopLevel::Enum(enum_decl)) = file.find_syntax_declaration(symbol_id) else {
            return FxHashMap::default();
        };
        let Some(body) = enum_decl.body() else {
            return FxHashMap::default();
        };
        let source = file.source().source.as_ref();
        let mut values = FxHashMap::default();
        let mut previous = Some(BigInt::from(-1));

        for (member, member_symbol) in body.members().zip(members) {
            let value = member.default().map_or_else(
                || {
                    previous
                        .as_ref()
                        .map_or(ConstantValue::Unknown, |value| checked_integer(value + 1))
                },
                |expression| {
                    normalize_enum_value(self.evaluate_expression(
                        symbol_id.file_id,
                        expression,
                        source,
                    ))
                },
            );
            previous = match &value {
                ConstantValue::Int(value) => Some(value.clone()),
                ConstantValue::Bool(_)
                | ConstantValue::String(_)
                | ConstantValue::Overflow
                | ConstantValue::Unknown => None,
            };
            values.insert(member_symbol.id, value);
        }

        values
    }

    fn evaluate_expression(
        &mut self,
        file_id: FileId,
        expression: Expr<'_>,
        source: &str,
    ) -> ConstantValue {
        match expression {
            Expr::NumberLit(literal) => parse_integer(literal.text(source)),
            Expr::BoolLit(literal) => ConstantValue::Bool(literal.value()),
            Expr::StringLit(literal) => ConstantValue::String(literal.content(source).to_owned()),
            Expr::Ident(identifier) => self.evaluate_reference(file_id, identifier.span()),
            Expr::DotAccess(access) => access.field().map_or(ConstantValue::Unknown, |field| {
                self.evaluate_reference(file_id, field.span())
            }),
            Expr::Bin(binary) => {
                let Some(left) = binary.left() else {
                    return ConstantValue::Unknown;
                };
                let Some(right) = binary.right() else {
                    return ConstantValue::Unknown;
                };
                let left = self.evaluate_expression(file_id, left, source);
                let right = self.evaluate_expression(file_id, right, source);
                apply_binary(binary.operator_name(source), left, right)
            }
            Expr::Unary(unary) => {
                let Some(argument) = unary.argument() else {
                    return ConstantValue::Unknown;
                };
                let argument = self.evaluate_expression(file_id, argument, source);
                apply_unary(unary.operator_name(source), argument)
            }
            Expr::Paren(paren) => paren.inner().map_or(ConstantValue::Unknown, |inner| {
                self.evaluate_expression(file_id, inner, source)
            }),
            Expr::AsCast(cast) => cast.expr().map_or(ConstantValue::Unknown, |inner| {
                self.evaluate_expression(file_id, inner, source)
            }),
            Expr::Call(call) => self.evaluate_compile_time_call(file_id, call, source),
            _ => ConstantValue::Unknown,
        }
    }

    fn evaluate_reference(&mut self, file_id: FileId, span: Span) -> ConstantValue {
        let Some(Resolved::Global(symbol_id)) = self.context.resolve_at(file_id, span) else {
            return ConstantValue::Unknown;
        };
        let Some(symbol) = self.context.project_index().resolve_symbol(symbol_id) else {
            return ConstantValue::Unknown;
        };

        match symbol.kind {
            SymbolKind::Constant => self.evaluate_constant(symbol_id),
            SymbolKind::EnumMember => self.evaluate_enum_member(symbol_id),
            _ => ConstantValue::Unknown,
        }
    }

    fn evaluate_compile_time_call(
        &mut self,
        file_id: FileId,
        call: tolk_syntax::Call<'_>,
        source: &str,
    ) -> ConstantValue {
        let Some(callee) = call.callee_identifier() else {
            return ConstantValue::Unknown;
        };
        let name = callee.utf8_text(source.as_bytes()).unwrap_or("");

        if let Some(receiver) = call.callee_qualifier() {
            let mut arguments = call.arguments();
            let value = if matches!(&receiver, Expr::Ident(identifier) if identifier.text(source) == "string")
            {
                let Some(argument) = arguments.next().and_then(|argument| argument.expr()) else {
                    return ConstantValue::Unknown;
                };
                if arguments.next().is_some() {
                    return ConstantValue::Unknown;
                }
                self.evaluate_expression(file_id, argument, source)
            } else {
                if arguments.next().is_some() {
                    return ConstantValue::Unknown;
                }
                self.evaluate_expression(file_id, receiver, source)
            };
            let ConstantValue::String(value) = value else {
                return ConstantValue::Unknown;
            };
            return evaluate_string_function(name, &value);
        }

        let mut arguments = call.arguments();
        let Some(Expr::StringLit(argument)) = arguments.next().and_then(|argument| argument.expr())
        else {
            return ConstantValue::Unknown;
        };
        if arguments.next().is_some() {
            return ConstantValue::Unknown;
        }
        let value = argument.content(source);

        match name {
            "grams" | "ton" => parse_nanograms(value),
            "stringCrc32" => evaluate_string_function("crc32", value),
            "stringCrc16" => evaluate_string_function("crc16", value),
            "stringSha256" => evaluate_string_function("sha256", value),
            "stringSha256_32" => evaluate_string_function("sha256_32", value),
            "stringToBase256" => evaluate_string_function("toBase256", value),
            _ => ConstantValue::Unknown,
        }
    }
}

#[must_use]
pub const fn is_simple_literal(expression: &Expr<'_>) -> bool {
    matches!(
        expression,
        Expr::NumberLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) | Expr::NullLit(_)
    )
}

fn parse_integer(text: &str) -> ConstantValue {
    let Some(parsed) = parse_tolk_int_literal(text) else {
        return ConstantValue::Unknown;
    };
    let Some(value) = BigInt::parse_bytes(parsed.digits().as_bytes(), parsed.radix()) else {
        return ConstantValue::Unknown;
    };
    checked_integer(value)
}

fn checked_integer(value: BigInt) -> ConstantValue {
    let limit = BigInt::from(1u8) << 256;
    if value < -&limit || value >= limit {
        ConstantValue::Overflow
    } else {
        ConstantValue::Int(value)
    }
}

fn normalize_enum_value(value: ConstantValue) -> ConstantValue {
    match value {
        ConstantValue::Int(value) => ConstantValue::Int(value),
        ConstantValue::Bool(true) => ConstantValue::Int(BigInt::from(-1)),
        ConstantValue::Bool(false) => ConstantValue::Int(BigInt::ZERO),
        ConstantValue::String(_) | ConstantValue::Unknown => ConstantValue::Unknown,
        ConstantValue::Overflow => ConstantValue::Overflow,
    }
}

fn apply_binary(operator: &str, left: ConstantValue, right: ConstantValue) -> ConstantValue {
    if matches!(&left, ConstantValue::Overflow) || matches!(&right, ConstantValue::Overflow) {
        return ConstantValue::Overflow;
    }

    if let (ConstantValue::Int(left), ConstantValue::Int(right)) = (&left, &right) {
        return match operator {
            "+" => checked_integer(left + right),
            "-" => checked_integer(left - right),
            "*" => checked_integer(left * right),
            "/" if !right.is_zero() => {
                let quotient = left / right;
                let remainder = left % right;
                if !remainder.is_zero() && remainder.sign() != right.sign() {
                    checked_integer(quotient - 1)
                } else {
                    checked_integer(quotient)
                }
            }
            "%" if !right.is_zero() => {
                let remainder = left % right;
                if !remainder.is_zero() && remainder.sign() != right.sign() {
                    checked_integer(remainder + right)
                } else {
                    checked_integer(remainder)
                }
            }
            "/" | "%" => ConstantValue::Overflow,
            "<<" | ">>" => {
                let Some(shift) = right.to_usize().filter(|shift| *shift <= 256) else {
                    return ConstantValue::Overflow;
                };
                if operator == "<<" {
                    checked_integer(left << shift)
                } else {
                    checked_integer(left >> shift)
                }
            }
            "&" => checked_integer(left & right),
            "|" => checked_integer(left | right),
            "^" => checked_integer(left ^ right),
            "&&" => ConstantValue::Bool(!left.is_zero() && !right.is_zero()),
            "||" => ConstantValue::Bool(!left.is_zero() || !right.is_zero()),
            "==" => ConstantValue::Bool(left == right),
            "!=" => ConstantValue::Bool(left != right),
            "<" => ConstantValue::Bool(left < right),
            ">" => ConstantValue::Bool(left > right),
            "<=" => ConstantValue::Bool(left <= right),
            ">=" => ConstantValue::Bool(left >= right),
            _ => ConstantValue::Unknown,
        };
    }

    if let (ConstantValue::Bool(left), ConstantValue::Bool(right)) = (&left, &right) {
        return match operator {
            "&" | "&&" => ConstantValue::Bool(*left && *right),
            "|" | "||" => ConstantValue::Bool(*left || *right),
            "^" => ConstantValue::Bool(*left ^ *right),
            "==" => ConstantValue::Bool(left == right),
            "!=" => ConstantValue::Bool(left != right),
            _ => ConstantValue::Unknown,
        };
    }

    ConstantValue::Unknown
}

fn apply_unary(operator: &str, argument: ConstantValue) -> ConstantValue {
    match (operator, argument) {
        (_, ConstantValue::Overflow) => ConstantValue::Overflow,
        ("!", argument) => {
            to_bool(&argument).map_or(ConstantValue::Unknown, |value| ConstantValue::Bool(!value))
        }
        ("-", ConstantValue::Int(value)) => checked_integer(-value),
        ("+", value @ ConstantValue::Int(_)) => value,
        ("~", ConstantValue::Int(value)) => checked_integer(!value),
        _ => ConstantValue::Unknown,
    }
}

fn to_bool(value: &ConstantValue) -> Option<bool> {
    match value {
        ConstantValue::Bool(value) => Some(*value),
        ConstantValue::Int(value) => Some(!value.is_zero()),
        ConstantValue::String(_) | ConstantValue::Overflow | ConstantValue::Unknown => None,
    }
}

fn evaluate_string_function(name: &str, value: &str) -> ConstantValue {
    match name {
        "crc32" => ConstantValue::Int(BigInt::from(crc32(value.as_bytes()))),
        "crc16" => ConstantValue::Int(BigInt::from(crc16(value.as_bytes()))),
        "sha256" => ConstantValue::Int(BigInt::from_bytes_be(
            Sign::Plus,
            &Sha256::digest(value.as_bytes()),
        )),
        "sha256_32" => {
            let digest = Sha256::digest(value.as_bytes());
            ConstantValue::Int(BigInt::from(u32::from_be_bytes([
                digest[0], digest[1], digest[2], digest[3],
            ])))
        }
        "toBase256" => checked_integer(BigInt::from_bytes_be(Sign::Plus, value.as_bytes())),
        _ => ConstantValue::Unknown,
    }
}

fn parse_nanograms(value: &str) -> ConstantValue {
    let (negative, value) = if let Some(value) = value.strip_prefix('-') {
        (true, value)
    } else {
        (false, value.strip_prefix('+').unwrap_or(value))
    };
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or_default();
    let fractional = parts.next().unwrap_or("");
    if parts.next().is_some()
        || integer.is_empty()
        || fractional.len() > 9
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fractional.bytes().all(|byte| byte.is_ascii_digit())
    {
        return ConstantValue::Unknown;
    }
    if integer.len() > 9 {
        return ConstantValue::Overflow;
    }

    let Ok(integer) = integer.parse::<i64>() else {
        return ConstantValue::Unknown;
    };
    let fractional = if fractional.is_empty() {
        0
    } else {
        let Ok(parsed_fractional) = fractional.parse::<i64>() else {
            return ConstantValue::Unknown;
        };
        parsed_fractional * 10_i64.pow(9 - fractional.len() as u32)
    };
    let nanograms = BigInt::from(integer * 1_000_000_000 + fractional);
    ConstantValue::Int(if negative { -nanograms } else { nanograms })
}
