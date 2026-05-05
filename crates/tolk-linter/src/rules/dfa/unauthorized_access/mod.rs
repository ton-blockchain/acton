use crate::Checker;
use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;
use tolk_resolver::resolve_index::{FileResolveIndex, LocalDefKind};

pub mod analysis;

/// ### What it does
/// Detects storage mutations (`contract.setData(...)`, `*.save()`) that are reachable
/// without a preceding admin sender check.
///
/// ### Why is this bad?
/// State-changing operations that are not guarded by admin authorization may allow
/// arbitrary inbound senders to mutate contract storage.
///
/// ### Example
/// ```tolk twoslash
/// fun onInternalMessage(in: InMessage) {
///     val storage = lazy Storage.fromCell(contract.getData());
///     storage.save();
///     //      ^^^^ E013: possible storage write without admin sender check
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun onInternalMessage(in: InMessage) {
///     val storage = lazy Storage.fromCell(contract.getData());
///     assert (in.senderAddress == storage.adminAddress) throw ERR_UNAUTHORIZED;
///     storage.save();
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "v0.0.1")]
pub struct UnauthorizedAccess;

impl Violation for UnauthorizedAccess {
    fn message(&self) -> String {
        "possible storage write without admin sender check".to_owned()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let resolve_index = checker.resolve_index_for(file_id)?;

    for top_level in file.source().top_levels() {
        let Some(symbol) = file.find_declaration(&top_level) else {
            continue;
        };
        if !symbol.is_func() {
            continue;
        }
        if symbol.name.as_ref() != "onInternalMessage" {
            continue;
        }

        let Some(cfg) = checker.cfg_for_symbol(symbol.id) else {
            continue;
        };

        let report = analysis::run(cfg.as_ref());
        for issue in report.issues {
            emit_issue(checker, file_id, resolve_index.as_ref(), issue);
        }
    }

    Some(())
}

fn emit_issue(
    checker: &mut Checker,
    file_id: FileId,
    resolve_index: &FileResolveIndex,
    issue: analysis::UncheckedStorageWrite,
) {
    let Some(primary_span) = issue.span else {
        return;
    };

    let mut annotations = vec![Annotation {
        span: primary_span,
        message: Some(
            "storage mutation (`save` / `setData`) is reachable without admin sender check"
                .to_owned(),
        ),
        is_primary: true,
        tags: vec![],
    }];

    if let Some(local) = resolve_index.locals.iter().find(|local| {
        matches!(local.kind, LocalDefKind::Param { .. }) && local.name.as_ref() == "in"
    }) {
        annotations.push(Annotation {
            span: local.def_span,
            message: Some("inbound sender comes from message parameter `in`".to_owned()),
            is_primary: false,
            tags: vec![],
        });
    }

    let diagnostic = Diagnostic::warning_for(file_id, UnauthorizedAccess)
        .with_annotations(annotations)
        .with_help("add `assert (in.senderAddress == storage.adminAddress) throw <code>;` before storage write");

    checker.emit_diagnostic(diagnostic);
}
