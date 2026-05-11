use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;

pub mod analysis;

/// ### What it does
/// Detects cases where division is evaluated before multiplication.
///
/// ### Why is this bad?
/// Division done too early may lose precision (especially for integer arithmetic)
/// and can produce unintended results in subsequent multiplication.
///
/// ### Example
/// ```tolk twoslash
/// fun main(a: int, b: int, c: int): int {
///     return a / b * c;
///     //     ^^^^^^^^^ E019: division before multiplication may cause precision loss
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun main(a: int, b: int, c: int): int {
///     return a * c / b;
/// }
/// ```
///
/// ### Behavior notes
/// The check uses CFG + dataflow:
/// - reports direct patterns like `(x / y) * z` and `x * (y / z)`;
/// - tracks locals tainted by division through assignments and warns when
///   such values are later used in multiplication.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "v0.0.1")]
pub struct DivideBeforeMultiply;

impl Violation for DivideBeforeMultiply {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "division before multiplication may cause precision loss".to_owned()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    if !file.is_workspace_file() {
        return None;
    }

    for top_level in file.source().top_levels() {
        let Some(symbol) = file.find_declaration(&top_level) else {
            continue;
        };
        if !symbol.is_func() {
            continue;
        }

        let Some(cfg) = checker.cfg_for_symbol(symbol.id) else {
            continue;
        };

        let report = analysis::run(cfg.as_ref());
        for issue in report.issues {
            emit_issue(checker, file_id, issue);
        }
    }

    Some(())
}

fn emit_issue(checker: &mut Checker, file_id: FileId, issue: analysis::DivideBeforeMultiplyIssue) {
    let Some(primary_span) = issue.span else {
        return;
    };

    let primary_message = match issue.kind {
        analysis::DivideBeforeMultiplyKind::Direct => "multiplication happened here",
        analysis::DivideBeforeMultiplyKind::Tainted => {
            "this multiplication uses a value produced by an earlier division"
        }
    };

    let mut annotations = vec![Annotation {
        span: primary_span,
        message: Some(primary_message.to_owned()),
        is_primary: true,
        tags: vec![],
    }];

    if let Some(origin) = issue.division_origin
        && origin.span != primary_span
    {
        annotations.push(Annotation {
            span: origin.span,
            message: Some("division that feeds this multiplication happens here".to_owned()),
            is_primary: false,
            tags: vec![],
        });
    }

    let diagnostic = Diagnostic::warning_for(file_id, DivideBeforeMultiply)
        .with_annotations(annotations)
        .with_help(
            "doing division first can change the result because precision is lost and rounding/truncation happens earlier\nwhen arithmetic intent allows it, multiply first and divide afterward",
        );

    checker.emit_diagnostic(diagnostic);
}
