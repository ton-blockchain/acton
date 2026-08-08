use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, Symbol};
use tolk_resolver::resolve_index::Resolved;
use tolk_resolver::{AstNodeSpanExt, NameUse, SymbolKind};
use tolk_syntax::ast::expressions::Call;
use tolk_ty::InferenceResult;

/// ### What it does
/// Checks for calls to `@pure` functions where the result is not used.
///
/// ### Why is this bad?
/// `@pure` functions have no side effects, so calling them without using the result is a no-op and likely a bug.
///
/// ### Example
/// ```tolk twoslash
/// @pure
/// fun add(a: int, b: int): int { return a + b; }
///
/// fun main() {
///     add(1, 2);
/// //  ^^^^^^^^^ E006: result of pure function is not used
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct PureFunctionCallUnused;

impl Violation for PureFunctionCallUnused {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "result of pure function is not used".to_string()
    }
}

pub fn check_expr_stmt_call(
    checker: &mut Checker,
    file_id: FileId,
    call: &Call,
    current_inference: Option<&InferenceResult>,
) -> Option<()> {
    let callee_ident = call.callee_identifier()?;
    let resolve_index = checker.resolve_index_for(file_id)?;

    // Try to resolve as standalone function
    if let Some(name_use) = resolve_index.find_use(callee_ident.start_byte()) {
        check_symbol(checker, file_id, call, name_use);
        // since we already resolve this symbol we don't need to do anything else
        return None;
    }

    // Try to resolve as method call using current inference result
    if let Some(inference) = current_inference
        && let Some(name_use) = inference.resolve(callee_ident.span())
    {
        check_symbol(checker, file_id, call, name_use);
    }

    None
}

fn check_symbol(checker: &mut Checker, file_id: FileId, call: &Call, name_use: &NameUse) {
    if let Resolved::Global(symbol_id) = name_use.resolved
        && let Some(symbol) = checker.type_db.project_index.resolve_symbol(symbol_id)
        && symbol.is_pure
    {
        if let SymbolKind::Method {
            is_mutable: true, ..
        } = symbol.kind
        {
            // if we call some mutable method we shouldn't report them as unused
            return;
        }

        fire_diagnostic(checker, file_id, call, symbol);
    }
}

fn fire_diagnostic(checker: &mut Checker, file_id: FileId, call: &Call, symbol: &Symbol) {
    let diagnostic = Diagnostic::warning_for(file_id, PureFunctionCallUnused)
        .with_annotations(vec![Annotation {
            span: call.span(),
            message: Some(format!(
                "result of pure function `{}` is not used",
                symbol.name
            )),
            is_primary: true,
            tags: vec![],
        }])
        .with_help("functions marked with `@pure` have no side effects. Calling them without using the result does nothing and may indicate a bug");
    checker.emit_diagnostic(diagnostic);
}
