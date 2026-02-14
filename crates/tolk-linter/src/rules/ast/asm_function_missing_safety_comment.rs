use crate::rules::diagnostic::{Annotation, Diagnostic, Severity};
use crate::rules::violation::{Violation, ViolationMetadata};
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
/// ```tolk
/// fun lowLevelLoad(x: slice): int asm "LDI";
/// ```
///
/// Use instead:
/// ```tolk
/// // SAFETY: `x` always contains at least 32 bits.
/// fun lowLevelLoad(x: slice): int asm "LDI";
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
    let lines = source.lines().collect::<Vec<_>>();
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

        if has_safety_comment_above_declaration(&lines, line_offsets, decl_span.start()) {
            continue;
        }

        fire_diagnostic(checker, file_id, name_span.unwrap_or(decl_span));
    }

    Some(())
}

fn has_safety_comment_above_declaration(
    lines: &[&str],
    line_offsets: &[usize],
    declaration_start: usize,
) -> bool {
    let declaration_line = offset_to_line(line_offsets, declaration_start);
    if declaration_line == 0 {
        return false;
    }

    let mut line_idx = declaration_line as isize - 1;
    while line_idx >= 0 {
        let Some(line) = lines.get(line_idx as usize) else {
            break;
        };
        let trimmed = line.trim_start();

        if trimmed.is_empty() || !trimmed.starts_with("//") {
            break;
        }

        if contains_safety_word(trimmed) {
            return true;
        }

        line_idx -= 1;
    }

    false
}

fn offset_to_line(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(line) => line,
        Err(0) => 0,
        Err(next_line) => next_line - 1,
    }
}

fn contains_safety_word(text: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token.eq_ignore_ascii_case("safety"))
}

#[cold]
fn fire_diagnostic(checker: &mut Checker, file_id: FileId, span: Span) {
    let diagnostic = Diagnostic {
        file_id,
        severity: Severity::Warning,
        name: AsmFunctionMissingSafetyComment::rule().name(),
        code: AsmFunctionMissingSafetyComment::code().map(|code| code.to_string()),
        message: AsmFunctionMissingSafetyComment.message(),
        annotations: vec![Annotation {
            span,
            message: Some(
                "add `// SAFETY: ...` or a doc comment section like `/// # Safety` above this declaration"
                    .to_string(),
            ),
            is_primary: true,
            tags: vec![],
        }],
        fixes: vec![],
        help: Some(
            "document why this asm usage is sound and what preconditions callers must satisfy"
                .to_string(),
        ),
    };

    checker.emit_diagnostic(AsmFunctionMissingSafetyComment::rule(), diagnostic);
}
