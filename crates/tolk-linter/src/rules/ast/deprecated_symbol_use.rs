use crate::rules::diagnostic::{Annotation, Diagnostic, DiagnosticTag};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;
use tolk_resolver::{AstNodeSpanExt, Symbol};
use tolk_syntax::{AstNode, Expr, HasAnnotations, HasName};
use tree_sitter::Node;

/// ### What it does
/// Checks for usage of deprecated symbols in code.
///
/// ### Why is this bad?
/// Using deprecated symbols can lead to compatibility issues,
/// unexpected behavior, or reliance on outdated APIs. Such symbols
/// may be removed in future versions.
///
/// ### Example
/// ```tolk twoslash
/// @deprecated
/// fun oldFunction() {
///     // some deprecated logic
/// }
///
/// fun main() {
///     oldFunction();
/// //  ^^^^^^^^^^^ E003: usage of deprecated symbol
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct DeprecatedSymbolUse;

impl Violation for DeprecatedSymbolUse {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "usage of deprecated symbol".to_string()
    }
}

pub fn check_resolved_reference(
    checker: &mut Checker,
    file_id: FileId,
    ident: &Node,
    symbol: &Symbol,
) -> Option<()> {
    if !symbol.is_deprecated {
        // fast path for 99.9% of declarations
        return None;
    }

    // Search for deprecated message is quite expensive but this is fine for this rule.
    let message =
        find_deprecated_message(checker, symbol).map_or_else(String::new, |msg| format!(". {msg}"));

    let diagnostic = Diagnostic::warning_for(file_id, DeprecatedSymbolUse)
        .with_annotations(vec![Annotation {
            span: ident.span(),
            message: Some(format!(
                "{} is deprecated and should not be used{message}",
                symbol.name
            )),
            is_primary: true,
            tags: vec![DiagnosticTag::Deprecated],
        }])
        .with_help("deprecated symbols may be removed in future versions");
    checker.emit_diagnostic(diagnostic);

    Some(())
}

fn find_deprecated_message(checker: &mut Checker, symbol: &Symbol) -> Option<String> {
    let file = checker.file_db.get_by_id(symbol.id.file_id)?;
    let decl = file.find_syntax_declaration(symbol.id)?;

    let source = &file.source().source;

    if let Some(a) = decl.annotations() {
        for a in a.annotations() {
            if a.name()
                .is_some_and(|name| name.text_matches(source, "deprecated"))
            {
                let Some(args) = a.args() else { continue };

                if let Some(Expr::StringLit(first)) = args.args().first() {
                    return Some(first.content(source).to_owned());
                }
            }
        }
    }

    None
}
