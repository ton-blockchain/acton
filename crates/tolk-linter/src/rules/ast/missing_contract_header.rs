use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, Span};
use tolk_syntax::{CONTRACT_ENTRYPOINTS, HasName, TopLevel};

/// ### What it does
/// Warns when a contract source defines standard contract entrypoints but omits the `contract` header.
///
/// ### Why is this bad?
/// The contract header is the ABI source of truth for metadata like `storage` and `incomingMessages`.
/// Without it, wrapper generation and other tooling cannot recover the contract shape reliably.
///
/// ### Example
/// ```tolk twoslash
/// fun onInternalMessage(in: InMessage) {
/// //  ^^^^^^^^^^^^^^^^^ E025: contract defines entrypoints but is missing a `contract` header
///     // ...
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// contract Wallet {
///     storage: Storage
///     incomingMessages: AllowedMessage
/// }
///
/// fun onInternalMessage(in: InMessage) {
///     // ...
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct MissingContractHeader;

impl Violation for MissingContractHeader {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "contract defines entrypoints but is missing a `contract` header".to_owned()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    if !checker.is_contract_root_file(file_id) {
        return None;
    }

    let file = checker.file_db.get_by_id(file_id)?;
    if file.has_contract_declaration() {
        return Some(());
    }

    let entrypoint_span = first_contract_entrypoint_span(checker, file_id)?;

    fire_diagnostic(checker, file_id, entrypoint_span);
    Some(())
}

fn first_contract_entrypoint_span(checker: &Checker, file_id: FileId) -> Option<Span> {
    let file = checker.file_db.get_by_id(file_id)?;

    for top_level in file.source().top_levels() {
        let TopLevel::Func(func) = top_level else {
            continue;
        };
        let Some(name) = func.name() else {
            continue;
        };
        if CONTRACT_ENTRYPOINTS
            .iter()
            .any(|entrypoint| checker.file_db.text_matches(file_id, &name, entrypoint))
        {
            return Some(name.span());
        }
    }

    None
}

#[cold]
fn fire_diagnostic(checker: &mut Checker, file_id: FileId, entrypoint_span: Span) {
    let diagnostic = Diagnostic::warning_for(file_id, MissingContractHeader)
        .with_annotations(vec![Annotation {
            span: entrypoint_span,
            message: Some("contract entrypoint declared here".to_owned()),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(
            "Add `contract <Name> { ... }` near the top of the file so compiler ABI and wrappers can see `storage`, `incomingMessages`, and other contract metadata.",
        );
    checker.emit_diagnostic(diagnostic);
}
