use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt::Write;
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, Span, SymbolId};
use tolk_resolver::resolve_index::{FileResolveIndex, Resolved};
use tolk_syntax::{
    AstNode, ContractField, ContractFieldValue, HasName, TopLevel, Type, TypeAliasUnderlyingType,
    parse_tolk_int_literal,
};

/// ### What it does
/// Checks `contract` header field `incomingMessages` for duplicate message opcodes.
///
/// ### Why is this bad?
/// If two incoming message variants share the same opcode, message decoding is ambiguous.
///
/// ### Example
/// ```tolk twoslash
/// struct (0x1000) MsgA {}
/// struct (0x1000) MsgB {}
/// //      ^^^^^^ E023: incomingMessages contains message variants with duplicate opcode
///
/// contract Wallet {
///     incomingMessages: MsgA | MsgB,
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// struct (0x1000) MsgA {}
/// struct (0x1001) MsgB {}
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct IncomingMessagesDuplicateOpcode;

impl Violation for IncomingMessagesDuplicateOpcode {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "incomingMessages contains message variants with duplicate opcode".to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OpcodeKey {
    value: u64,
    bit_len: u16,
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let resolve_index = checker.resolve_index_for(file_id)?;

    for top_level in file.source().top_levels() {
        let TopLevel::Contract(contract) = top_level else {
            continue;
        };
        let Some(body) = contract.body() else {
            continue;
        };

        for field in body.fields() {
            if !is_incoming_messages_field(checker, file_id, &field) {
                continue;
            }
            let Some(ContractFieldValue::Type(typ)) = field.value() else {
                continue;
            };

            check_incoming_messages_type(
                checker,
                file_id,
                resolve_index.as_ref(),
                typ.span(),
                &typ,
            );
        }
    }

    Some(())
}

fn check_incoming_messages_type(
    checker: &mut Checker,
    file_id: FileId,
    resolve_index: &FileResolveIndex,
    field_span: Span,
    typ: &Type,
) {
    let mut opcodes = Vec::new();
    let mut alias_stack = FxHashSet::default();
    collect_message_opcodes(checker, resolve_index, typ, &mut alias_stack, &mut opcodes);

    if opcodes.len() < 2 {
        return;
    }

    let mut by_opcode: FxHashMap<OpcodeKey, Vec<String>> = FxHashMap::default();
    for (opcode, message_name) in opcodes {
        by_opcode.entry(opcode).or_default().push(message_name);
    }

    for (opcode, mut message_names) in by_opcode {
        if message_names.len() < 2 {
            continue;
        }

        message_names.sort_unstable();
        message_names.dedup();
        emit_duplicate_opcode_diagnostic(
            checker,
            file_id,
            field_span,
            format_opcode(opcode),
            &message_names,
        );
    }
}

fn collect_message_opcodes(
    checker: &Checker,
    resolve_index: &FileResolveIndex,
    typ: &Type,
    alias_stack: &mut FxHashSet<SymbolId>,
    out: &mut Vec<(OpcodeKey, String)>,
) {
    match typ {
        Type::UnionType(union_type) => {
            if let Some(lhs) = union_type.lhs() {
                collect_message_opcodes(checker, resolve_index, &lhs, alias_stack, out);
            }
            if let Some(rhs) = union_type.rhs() {
                collect_message_opcodes(checker, resolve_index, &rhs, alias_stack, out);
            }
        }
        Type::ParenthesizedType(parenthesized) => {
            if let Some(inner) = parenthesized.inner() {
                collect_message_opcodes(checker, resolve_index, &inner, alias_stack, out);
            }
        }
        Type::TypeIdent(type_ident) => {
            collect_type_ident_opcodes(checker, resolve_index, *type_ident, alias_stack, out);
        }
        Type::TypeInstantiatedTs(type_with_args) => {
            if let Some(type_ident) = type_with_args.name() {
                collect_type_ident_opcodes(checker, resolve_index, type_ident, alias_stack, out);
            }
        }
        _ => {}
    }
}

fn collect_type_ident_opcodes(
    checker: &Checker,
    resolve_index: &FileResolveIndex,
    type_ident: tolk_syntax::TypeIdent<'_>,
    alias_stack: &mut FxHashSet<SymbolId>,
    out: &mut Vec<(OpcodeKey, String)>,
) {
    let Some(symbol_id) = resolve_type_ident_symbol(resolve_index, type_ident) else {
        return;
    };

    if let Some((opcode, message_name)) = try_get_struct_opcode(checker, symbol_id) {
        out.push((opcode, message_name));
        return;
    }

    collect_type_alias_opcodes(checker, symbol_id, alias_stack, out);
}

fn resolve_type_ident_symbol(
    resolve_index: &FileResolveIndex,
    type_ident: tolk_syntax::TypeIdent<'_>,
) -> Option<SymbolId> {
    let name_use = resolve_index.find_use(type_ident.span().start())?;
    match &name_use.resolved {
        Resolved::Global(symbol_id) => Some(*symbol_id),
        _ => None,
    }
}

fn try_get_struct_opcode(checker: &Checker, symbol_id: SymbolId) -> Option<(OpcodeKey, String)> {
    let symbol = checker.type_db.project_index.resolve_symbol(symbol_id)?;
    let message_name = symbol.name.to_string();

    let file = checker.file_db.get_by_id(symbol_id.file_id)?;
    let decl = file.find_syntax_declaration(symbol_id)?;
    let TopLevel::Struct(message_struct) = decl else {
        return None;
    };
    let pack_prefix = message_struct.pack_prefix()?;
    let source = file.source().source.as_ref();
    let opcode = parse_struct_opcode_literal(pack_prefix.text(source))?;

    Some((opcode, message_name))
}

fn collect_type_alias_opcodes(
    checker: &Checker,
    symbol_id: SymbolId,
    alias_stack: &mut FxHashSet<SymbolId>,
    out: &mut Vec<(OpcodeKey, String)>,
) {
    if !alias_stack.insert(symbol_id) {
        // cyclic alias chain
        return;
    }

    let Some(file) = checker.file_db.get_by_id(symbol_id.file_id) else {
        alias_stack.remove(&symbol_id);
        return;
    };
    let Some(TopLevel::TypeAlias(type_alias)) = file.find_syntax_declaration(symbol_id) else {
        alias_stack.remove(&symbol_id);
        return;
    };
    let Some(TypeAliasUnderlyingType::Type(underlying_type)) = type_alias.underlying_type() else {
        alias_stack.remove(&symbol_id);
        return;
    };
    let Some(resolve_index) = checker.resolve_index_for(symbol_id.file_id) else {
        alias_stack.remove(&symbol_id);
        return;
    };

    collect_message_opcodes(
        checker,
        resolve_index.as_ref(),
        &underlying_type,
        alias_stack,
        out,
    );
    alias_stack.remove(&symbol_id);
}

fn is_incoming_messages_field(checker: &Checker, file_id: FileId, field: &ContractField) -> bool {
    field.name().is_some_and(|name| {
        checker
            .file_db
            .text_matches(file_id, &name, "incomingMessages")
    })
}

#[cold]
fn emit_duplicate_opcode_diagnostic(
    checker: &mut Checker,
    file_id: FileId,
    field_span: Span,
    opcode: String,
    message_names: &[String],
) {
    let messages = message_names.join(", ");
    let diagnostic = Diagnostic::warning_for(file_id, IncomingMessagesDuplicateOpcode)
        .with_annotations(vec![Annotation {
            span: field_span,
            message: Some(format!(
                "duplicate opcode {opcode} for messages: {messages}"
            )),
            is_primary: true,
            tags: vec![],
        }])
        .with_help("ensure every variant in `incomingMessages` has a unique opcode");

    checker.emit_diagnostic(diagnostic);
}

fn parse_struct_opcode_literal(raw: &str) -> Option<OpcodeKey> {
    let literal = parse_tolk_int_literal(raw)?;
    match literal.radix() {
        16 => {
            let value = literal.parse_u64()?;
            let bit_len: u16 = (literal.digit_len() * 4).try_into().ok()?;
            Some(OpcodeKey { value, bit_len })
        }
        2 => {
            let value = literal.parse_u64()?;
            let bit_len: u16 = literal.digit_len().try_into().ok()?;
            Some(OpcodeKey { value, bit_len })
        }
        _ => None,
    }
}

fn format_opcode(opcode: OpcodeKey) -> String {
    if opcode.bit_len.is_multiple_of(4) {
        let width = usize::from(opcode.bit_len / 4);
        return format!("0x{:0width$x}", opcode.value, width = width);
    }

    let width = usize::from(opcode.bit_len);
    let mut out = String::with_capacity(width + 2);
    out.push_str("0b");
    let _ = write!(&mut out, "{:0width$b}", opcode.value, width = width);
    out
}

#[cfg(test)]
mod tests {
    use super::{OpcodeKey, parse_struct_opcode_literal};
    use tolk_syntax::{AstNode, TopLevel, parse};

    fn parse_pack_prefix(code: &str) -> String {
        let file = parse(code).expect("parse failed");
        let strukt = file
            .top_levels()
            .find_map(|top_level| match top_level {
                TopLevel::Struct(strukt) => Some(strukt),
                _ => None,
            })
            .expect("struct should exist");
        let prefix = strukt.pack_prefix().expect("pack prefix should exist");
        prefix.text(file.source.as_ref()).to_string()
    }

    #[test]
    fn parse_struct_opcode_literal_supports_hex_separators() {
        let prefix = parse_pack_prefix("struct (0xFF_FF) Msg {}");
        assert_eq!(
            parse_struct_opcode_literal(&prefix),
            Some(OpcodeKey {
                value: 0xFFFF,
                bit_len: 16,
            })
        );
    }

    #[test]
    fn parse_struct_opcode_literal_supports_binary_separators() {
        let prefix = parse_pack_prefix("struct (0b_0____1_) Msg {}");
        assert_eq!(
            parse_struct_opcode_literal(&prefix),
            Some(OpcodeKey {
                value: 1,
                bit_len: 2,
            })
        );
    }
}
