use tolk_macros::ViolationMetadata;
use tolk_resolver::{AstNodeSpanExt, FileId, Symbol};
use tolk_syntax::Call;
use crate::{Checker, FixAvailability, Violation, ViolationMetadata};
use crate::ast::pure_function_call_unused::PureFunctionCallUnused;
use crate::diagnostic::{Annotation, Diagnostic, Severity};

#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct NoBounceHandler;

impl Violation for NoBounceHandler {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "No bounce handler".to_string()
    }
}

fn fire_diagnostic(checker: &mut Checker, file_id: FileId, call: &Call, symbol: &Symbol) {
    let diagnostic = Diagnostic {
        file_id,
        severity: Severity::Warning,
        name: NoBounceHandler::rule().name(),
        code: NoBounceHandler::code().map(|c| c.to_string()),
        message: NoBounceHandler.message(),
        annotations: vec![Annotation {
            span: call.span(),
            message: Some(format!(
                "result of pure function `{}` is not used",
                symbol.name
            )),
            is_primary: true,
            tags: vec![],
        }],
        fixes: vec![],
        help: Some("If the message is marked bounceable".to_string()),
    };
    checker.emit_diagnostic(PureFunctionCallUnused::rule(), diagnostic);
}


