use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::resolve_index::LocalDefKind;
use tolk_resolver::{AstNodeSpanExt, FileId, FileResolveIndex, Symbol, SymbolKind};
use tolk_syntax::{Call, TryFromNode};
use tree_sitter::Node;

/// Available since Acton 1.2.
///
/// ### What it does
/// Replaces the standard library's `ton()` alias with `grams()`.
///
/// ### Why is this bad?
/// `ton()` is the old name of `grams()`. The replacement preserves the amount.
///
/// ### Behavior notes
/// This rule reports standard-library calls when the standard library provides `grams`.
/// It leaves user-defined functions and older standard libraries without `grams` unchanged.
/// If a local binding shadows `grams`, the rule reports a warning without an automatic fix.
/// E003 does not report the same call when this rule reports it.
///
/// ### Example
/// ```tolk twoslash
/// const amount = ton("0.1");
/// //             ^^^ S009: prefer `grams()` over `ton()`
/// ```
///
/// Use instead:
/// ```tolk
/// const amount = grams("0.1");
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v1.2.0")]
pub struct PreferGrams;

impl Violation for PreferGrams {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    fn message(&self) -> String {
        "prefer `grams()` over `ton()`".to_owned()
    }
}

/// Reports standard-library calls and offers an identifier-only edit when no local shadows `grams`.
/// Returns `Some` only when it emits a diagnostic, so the caller can avoid a duplicate E003.
pub fn check_resolved_reference(
    checker: &mut Checker,
    file_id: FileId,
    ident: &Node,
    symbol: &Symbol,
    resolve_index: &FileResolveIndex,
) -> Option<()> {
    if !checker.has_stdlib_grams
        || symbol.name.as_ref() != "ton"
        || !matches!(symbol.kind, SymbolKind::Function { .. })
        || !checker.file_db.is_stdlib_file(symbol.id.file_id)
    {
        return None;
    }

    let call = Call::try_from_node(ident.parent()?).ok()?;
    if call.callee_identifier() != Some(*ident) {
        return None;
    }

    let file = checker.file_db.get_by_id(file_id)?;
    let shadowed = resolve_index.locals.iter().any(|local| {
        local.name.as_ref() == "grams"
            && !matches!(local.kind, LocalDefKind::TypeParameter)
            && local.is_visible_at(file.source().tree.root_node(), ident.start_byte())
    });

    let mut diagnostic =
        Diagnostic::warning_for(file_id, PreferGrams).with_annotations(vec![Annotation {
            span: ident.span(),
            message: Some("use the standard-library function `grams`".to_owned()),
            is_primary: true,
            tags: vec![],
        }]);

    if !shadowed {
        diagnostic = diagnostic.with_fixes(vec![Fix {
            message: "replace `ton` with `grams`".to_owned(),
            edits: vec![Edit {
                span: ident.span(),
                replacement: "grams".to_owned(),
                file_id,
            }],
            applicability: Applicability::Auto,
        }]);
    }

    checker.emit_diagnostic(diagnostic);
    Some(())
}
