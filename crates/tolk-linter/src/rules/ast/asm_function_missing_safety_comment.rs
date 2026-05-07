use super::safety_comment_helpers::has_safety_comment_above;
use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, Span};
use tolk_syntax::{FuncBody, FunctionLike, HasName, TopLevel};

/// ### What it does
/// Requires documenting safety requirements for every declaration with an `asm` body.
///
/// ### Why is this bad?
/// `asm` bypasses high-level safety guarantees. Without an explicit safety note,
/// it is hard to review assumptions and soundness constraints.
///
/// ### Example
/// ```tolk twoslash
/// fun lowLevelLoad(x: slice): int asm "32 LDI";
/// //                              ^^^ E011: asm function requires safety comment
/// ```
///
/// Use instead:
/// ```tolk
/// // SAFETY: `x` always contains at least 32 bits.
/// fun lowLevelLoad(x: slice): int asm "32 LDI";
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct AsmFunctionMissingSafetyComment;

impl Violation for AsmFunctionMissingSafetyComment {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "asm function requires safety comment".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();
    let line_offsets = file.line_offsets();

    for top_level in file.source().top_levels() {
        let (decl_span, name_span, has_asm_body) = match top_level {
            TopLevel::Func(func) => (
                func.span(),
                func.name().map(|ident| ident.span()),
                matches!(func.body(), Some(FuncBody::AsmBody(_))),
            ),
            TopLevel::Method(method) => (
                method.span(),
                method.name().map(|ident| ident.span()),
                matches!(method.body(), Some(FuncBody::AsmBody(_))),
            ),
            TopLevel::GetMethod(get_method) => (
                get_method.span(),
                get_method.name().map(|ident| ident.span()),
                matches!(get_method.body(), Some(FuncBody::AsmBody(_))),
            ),
            _ => continue,
        };

        if !has_asm_body {
            continue;
        }

        if has_safety_comment_above(source, line_offsets, decl_span.start()) {
            continue;
        }

        fire_diagnostic(checker, file_id, name_span.unwrap_or(decl_span));
    }

    Some(())
}

#[cold]
fn fire_diagnostic(checker: &mut Checker, file_id: FileId, span: Span) {
    let diagnostic = Diagnostic::warning_for(file_id, AsmFunctionMissingSafetyComment)
        .with_annotations(vec![Annotation {
            span,
            message: Some(
                "add `// SAFETY: ...` or a doc comment section like `/// # Safety` above this declaration"
                    .to_string(),
            ),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(
            "document why this asm usage is sound and what preconditions callers must satisfy"
        );

    checker.emit_diagnostic(diagnostic);
}
