use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, Span, SymbolId, SymbolKind};
use tolk_resolver::resolve_index::Resolved;
use tolk_resolver::{AstNodeSpanExt, Symbol};
use tolk_syntax::AstNode;
use tolk_syntax::ast::expressions::{DotAccess, DotAccessField, Expr};
use tree_sitter::Node;

/// ### What it does
/// Requires documentation for enum values used as `throw` codes.
///
/// ### Why is this bad?
/// Symbolic exit codes become part of the contract interface. Without a short
/// note on the enum value, callers and tests can see the name but not the exact
/// failure condition it represents.
///
/// ### Behavior notes
/// - This preview rule is disabled (`allow`) by default. Enable it in config:
///
/// ```toml
/// [lint.rules]
/// throw-requires-documented-error-value = "warn"
/// ```
///
/// - Or run only this rule with `acton check --enable-only E030`.
/// - The documentation must be a non-empty `///` doc comment immediately above
///   the enum value.
///
/// ### Example
/// ```tolk
/// enum Errors {
///     NotOwner = 401
/// }
///
/// fun onInternalMessage(in: InMessage) {
///     // ...
///     assert (in.senderAddress == storage.ownerAddress) throw Errors.NotOwner;
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// enum Errors {
///     /// Sender is not the current owner.
///     NotOwner = 401
/// }
///
/// fun onInternalMessage(in: InMessage) {
///     // ...
///     assert (in.senderAddress == storage.ownerAddress) throw Errors.NotOwner;
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "v0.0.1")]
pub struct ThrowRequiresDocumentedErrorValue;

impl Violation for ThrowRequiresDocumentedErrorValue {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "enum value used as throw code should be documented".to_owned()
    }
}

pub fn check_throw_expr(checker: &mut Checker, file_id: FileId, expr: &Expr<'_>) -> Option<()> {
    let usage = find_undocumented_enum_member_usage(checker, file_id, expr)?;

    emit_usage_diagnostic(checker, file_id, &usage);

    if usage.declaration_file_id != file_id {
        emit_declaration_help_diagnostic(checker, &usage);
    }

    Some(())
}

fn find_undocumented_enum_member_usage(
    checker: &Checker,
    file_id: FileId,
    expr: &Expr<'_>,
) -> Option<EnumMemberUsage> {
    match expr {
        Expr::Paren(paren) => {
            find_undocumented_enum_member_usage(checker, file_id, &paren.inner()?)
        }
        Expr::AsCast(as_cast) => {
            find_undocumented_enum_member_usage(checker, file_id, &as_cast.expr()?)
        }
        Expr::Tensor(tensor) => {
            find_undocumented_enum_member_usage(checker, file_id, &tensor.elements().next()?)
        }
        Expr::Bin(bin) => find_undocumented_enum_member_usage(checker, file_id, &bin.left()?)
            .or_else(|| find_undocumented_enum_member_usage(checker, file_id, &bin.right()?)),
        Expr::Unary(unary) => {
            find_undocumented_enum_member_usage(checker, file_id, &unary.argument()?)
        }
        Expr::DotAccess(dot_access) => {
            enum_member_usage_if_undocumented(checker, file_id, dot_access)
        }
        _ => None,
    }
}

fn enum_member_usage_if_undocumented(
    checker: &Checker,
    file_id: FileId,
    dot_access: &DotAccess<'_>,
) -> Option<EnumMemberUsage> {
    let field = dot_access.field()?;
    let DotAccessField::Ident(field_ident) = field else {
        return None;
    };

    let symbol_id = resolve_enum_member_symbol_id(checker, file_id, dot_access, field_ident)?;
    let symbol = checker.type_db.project_index.resolve_symbol(symbol_id)?;
    if !matches!(&symbol.kind, SymbolKind::EnumMember) {
        return None;
    }

    if enum_member_has_documentation(checker, symbol) {
        return None;
    }

    Some(EnumMemberUsage {
        declaration_file_id: symbol_id.file_id,
        fqn: symbol.fqn.to_string(),
        usage_span: field_ident.span(),
        declaration_span: symbol.name_span,
    })
}

fn resolve_enum_member_symbol_id(
    checker: &Checker,
    file_id: FileId,
    dot_access: &DotAccess<'_>,
    field_ident: tolk_syntax::Ident<'_>,
) -> Option<SymbolId> {
    if let Some(symbol_id) = resolve_global_symbol_id(checker, file_id, field_ident.syntax()) {
        return Some(symbol_id);
    }

    let Expr::Ident(enum_ident) = dot_access.obj()? else {
        return None;
    };
    let enum_symbol_id = resolve_global_symbol_id(checker, file_id, enum_ident.syntax())?;
    let enum_symbol = checker
        .type_db
        .project_index
        .resolve_symbol(enum_symbol_id)?;
    if !matches!(&enum_symbol.kind, SymbolKind::Enum { .. }) {
        return None;
    }

    let file = checker.file_db.get_by_id(file_id)?;
    let member_name = field_ident.text(file.source().source.as_ref());
    let member = checker
        .type_db
        .find_enum_member(enum_symbol_id, member_name)?;
    Some(member.id)
}

fn enum_member_has_documentation(checker: &Checker, symbol: &Symbol) -> bool {
    let Some(file) = checker.file_db.get_by_id(symbol.id.file_id) else {
        return false;
    };

    has_enum_member_documentation(
        file.source().source.as_ref(),
        file.line_offsets(),
        symbol.name_span,
    )
}

// TODO: move to doc_span once implemented
fn has_enum_member_documentation(source: &str, line_offsets: &[usize], name_span: Span) -> bool {
    let line_idx = offset_to_line(line_offsets, name_span.start());
    if line_idx == 0 {
        return false;
    }

    source_line(source, line_offsets, line_idx - 1)
        .map(str::trim_start)
        .and_then(|line| line.strip_prefix("///"))
        .is_some_and(|body| !body.trim().is_empty())
}

fn source_line<'a>(source: &'a str, line_offsets: &[usize], line_idx: usize) -> Option<&'a str> {
    let start = *line_offsets.get(line_idx)?;
    let end = line_offsets
        .get(line_idx + 1)
        .copied()
        .unwrap_or(source.len());
    source.get(start..end)
}

fn offset_to_line(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(line) => line,
        Err(0) => 0,
        Err(next_line) => next_line - 1,
    }
}

fn resolve_global_symbol_id(
    checker: &Checker,
    file_id: FileId,
    node: Node<'_>,
) -> Option<SymbolId> {
    let span = Span::from_syntax(&node);

    if let Some(resolve_index) = checker.resolve_index_for(file_id)
        && let Some(name_use) = resolve_index.find_use(span.start())
        && let Resolved::Global(symbol_id) = name_use.resolved
    {
        return Some(symbol_id);
    }

    None
}

#[cold]
fn emit_usage_diagnostic(checker: &mut Checker, file_id: FileId, usage: &EnumMemberUsage) {
    let mut annotations = vec![Annotation {
        span: usage.usage_span,
        message: Some(format!("`{}` has no enum value documentation", usage.fqn)),
        is_primary: true,
        tags: vec![],
    }];

    if usage.declaration_file_id == file_id {
        annotations.push(Annotation {
            span: usage.declaration_span,
            message: Some("document this enum value".to_owned()),
            is_primary: false,
            tags: vec![],
        });
    }

    let diagnostic = Diagnostic::warning_for(file_id, ThrowRequiresDocumentedErrorValue)
        .with_annotations(annotations)
        .with_help("add a `/// ...` doc comment immediately above the enum value explaining when this exit code is thrown");
    checker.emit_diagnostic(diagnostic);
}

#[cold]
fn emit_declaration_help_diagnostic(checker: &mut Checker, usage: &EnumMemberUsage) {
    let diagnostic = Diagnostic::help_for(
        usage.declaration_file_id,
        ThrowRequiresDocumentedErrorValue,
        "enum value is declared here",
    )
    .with_annotations(vec![Annotation {
        span: usage.declaration_span,
        message: Some("document this enum value".to_owned()),
        is_primary: true,
        tags: vec![],
    }]);
    checker.emit_diagnostic(diagnostic);
}

struct EnumMemberUsage {
    declaration_file_id: FileId,
    fqn: String,
    usage_span: Span,
    declaration_span: Span,
}
