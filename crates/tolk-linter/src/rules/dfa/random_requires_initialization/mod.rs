use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use rustc_hash::FxHashMap;
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, SymbolId};

pub mod analysis;

/// ### What it does
/// Requires random generator initialization before random value generation calls.
///
/// ### Why is this bad?
/// Calling `random.uint256(...)` or `random.range(...)` before
/// `random.initialize(...)` / `random.initializeBy(...)` can lead to predictable
/// or invalid randomness behavior.
///
/// ### Example
/// ```tolk twoslash
/// fun main() {
///     val x = random.uint256();
///     //      ^^^^^^^^^^^^^^^^ E018: random generator must be initialized before `random.uint256`/`random.range` call
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun main() {
///     random.initializeBy(123);
///     val x = random.uint256();
/// }
/// ```
///
/// ### Behavior notes
///
/// This check is control-flow aware. It does not just scan text for "init before use".
/// It builds a control-flow graph (CFG) and verifies that for each
/// `random.uint256(...)` / `random.range(...)` call, initialization is guaranteed
/// on all reachable execution paths before that call.
///
/// It also follows function calls, so helper functions that initialize random
/// are taken into account.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "v0.0.1")]
pub struct RandomRequiresInitialization;

impl Violation for RandomRequiresInitialization {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "random generator must be initialized before `random.uint256`/`random.range` call"
            .to_owned()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let mut issues = Vec::new();

    let mut summaries = RandomSummaryComputer::new(checker);
    for top_level in file.source().top_levels() {
        let Some(symbol) = file.find_declaration(&top_level) else {
            continue;
        };
        if !symbol.is_func() {
            continue;
        }

        let symbol_id = symbol.id;
        let Some(cfg) = summaries.checker.cfg_for_symbol(symbol_id) else {
            continue;
        };
        let report = analysis::run(cfg.as_ref(), symbol.id.file_id, &mut summaries);
        issues.extend(report.issues);
    }

    for issue in issues {
        emit_issue(checker, file_id, issue);
    }

    Some(())
}

fn emit_issue(checker: &mut Checker, file_id: FileId, issue: analysis::UninitializedRandomUsage) {
    let Some(primary_span) = issue.span else {
        return;
    };

    let diagnostic = Diagnostic::warning_for(file_id, RandomRequiresInitialization)
        .with_annotations(vec![Annotation {
            span: primary_span,
            message: Some(
                "`random.uint256` / `random.range` is reachable before initialization".to_owned(),
            ),
            is_primary: true,
            tags: vec![],
        }])
        .with_help("add `random.initialize(...)` or `random.initializeBy(...)` before this call");

    checker.emit_diagnostic(diagnostic);

    if let Some(site) = issue.conditional_initialization_site {
        let help = Diagnostic::help_for(
            site.file_id,
            RandomRequiresInitialization,
            "random initialization exists, but it is not guaranteed on all execution paths",
        )
        .with_annotations(vec![Annotation {
            span: site.span,
            message: Some("initialization is performed here".to_owned()),
            is_primary: true,
            tags: vec![],
        }]);
        checker.emit_diagnostic(help);
    }
}

enum CachedSummary {
    InProgress,
    Ready(analysis::InitializationSummary),
}

struct RandomSummaryComputer<'a, 'b> {
    checker: &'a mut Checker<'b>,
    summary_cache: FxHashMap<SymbolId, CachedSummary>,
}

impl<'a, 'b> RandomSummaryComputer<'a, 'b> {
    fn new(checker: &'a mut Checker<'b>) -> Self {
        Self {
            checker,
            summary_cache: FxHashMap::default(),
        }
    }

    fn summary_for_symbol(&mut self, symbol_id: SymbolId) -> analysis::InitializationSummary {
        if let Some(cached) = self.summary_cache.get(&symbol_id) {
            return match cached {
                CachedSummary::InProgress => analysis::InitializationSummary {
                    is_guaranteed: false,
                    has_any_initialization: false,
                    sample_site: None,
                },
                CachedSummary::Ready(summary) => *summary,
            };
        }

        let Some(symbol) = self.checker.type_db.project_index.resolve_symbol(symbol_id) else {
            return analysis::InitializationSummary {
                is_guaranteed: false,
                has_any_initialization: false,
                sample_site: None,
            };
        };

        if !symbol.is_func() {
            return analysis::InitializationSummary {
                is_guaranteed: false,
                has_any_initialization: false,
                sample_site: None,
            };
        }

        self.summary_cache
            .insert(symbol_id, CachedSummary::InProgress);

        let symbol_id1 = symbol.id;
        let summary = if let Some(cfg) = self.checker.cfg_for_symbol(symbol_id1) {
            analysis::function_summary(cfg.as_ref(), symbol_id.file_id, self)
        } else {
            analysis::InitializationSummary {
                is_guaranteed: false,
                has_any_initialization: false,
                sample_site: None,
            }
        };

        self.summary_cache
            .insert(symbol_id, CachedSummary::Ready(summary));
        summary
    }
}

impl analysis::SummaryProvider for RandomSummaryComputer<'_, '_> {
    fn summary_for(&mut self, symbol_id: SymbolId) -> analysis::InitializationSummary {
        self.summary_for_symbol(symbol_id)
    }
}
