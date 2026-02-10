use crate::rules::diagnostic::{
    Annotation, Applicability, Diagnostic, DiagnosticTag, Edit, Fix, Severity,
};
use crate::rules::violation::Violation;
use crate::rules::violation::ViolationMetadata;
use crate::{Checker, FixAvailability};
use heck::{ToLowerCamelCase, ToShoutySnakeCase, ToUpperCamelCase};
use std::collections::HashSet;
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;
use tolk_resolver::{Resolved, Symbol};

/// ### What it does
/// Checks identifier naming style and suggests consistent casing.
///
/// ### Why is this bad?
/// Inconsistent naming makes code harder to read and maintain.
/// This rule enforces:
/// - `camelCase` for variables, functions, methods, and struct fields
/// - `UpperCamelCase` for structs, enums, enum members, and type aliases
/// - `SCREAMING_SNAKE_CASE` for constants
///
/// ### Example
/// ```tolk
/// struct low_struct {
///     TheBad: int
/// }
///
/// const iAmConst_variable = 1
///
/// fun BadFunctionName() {}
/// ```
///
/// Use instead:
/// ```tolk
/// struct LowStruct {
///     theBad: int
/// }
///
/// const I_AM_CONST_VARIABLE = 1
///
/// fun badFunctionName() {}
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct NameCaseChecker;

impl Violation for NameCaseChecker {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    fn message(&self) -> String {
        "name should be in the expected case".to_owned()
    }
}

enum CaseRules {
    LowerCamelCase,
    UpperCamelCase,
    ScreamingSnakeCase,
}

fn check_case(symbol: &Symbol, checker: &mut Checker, symbol_def_file_id: FileId, case: CaseRules) {
    let (correct_case, case_name) = match case {
        CaseRules::LowerCamelCase => (symbol.name.to_lower_camel_case(), "camelCase"),
        CaseRules::UpperCamelCase => (symbol.name.to_upper_camel_case(), "UpperCamelCase"),
        CaseRules::ScreamingSnakeCase => {
            (symbol.name.to_shouty_snake_case(), "SCREAMING_SNAKE_CASE")
        }
    };

    if symbol.name == correct_case.clone().into() {
        return;
    }

    let mut edits = vec![];
    let mut seen = HashSet::new();
    // we need the definition itself too
    if seen.insert((
        symbol_def_file_id,
        symbol.name_span.start,
        symbol.name_span.end,
    )) {
        edits.push(Edit {
            span: symbol.name_span,
            replacement: correct_case.clone(),
            file_id: symbol_def_file_id,
        });
    }

    for (usage_file_id, resolved_index) in checker.type_db.project_index.resolved_uses.iter() {
        for usage in resolved_index.global_usages_of(symbol.id) {
            if seen.insert((*usage_file_id, usage.span.start, usage.span.end)) {
                edits.push(Edit {
                    span: usage.span,
                    replacement: correct_case.clone(),
                    file_id: *usage_file_id,
                });
            }
        }
    }

    // Extra usages resolved only during type inference (e.g. struct literal field keys).
    for (usage_file_id, file_body_types) in checker.body_types.iter() {
        for inference in file_body_types.values() {
            for usage in &inference.resolved_refs {
                if let Resolved::Global(resolved_id) = usage.resolved
                    && resolved_id == symbol.id
                    && seen.insert((*usage_file_id, usage.span.start, usage.span.end))
                {
                    edits.push(Edit {
                        span: usage.span,
                        replacement: correct_case.clone(),
                        file_id: *usage_file_id,
                    });
                }
            }
        }
    }

    let str_sym_name = &symbol.name;

    let diagnostic = Diagnostic {
        file_id: symbol_def_file_id,
        severity: Severity::Warning,
        name: NameCaseChecker::rule().name(),
        code: NameCaseChecker::code().map(|c| c.to_string()),
        message: NameCaseChecker.message(),
        annotations: vec![Annotation {
            span: symbol.name_span,
            message: Some(format!("not {case_name}: `{str_sym_name}`",)),
            is_primary: true,
            tags: vec![DiagnosticTag::Unnecessary],
        }],
        fixes: vec![Fix {
            message: format!("rename to {case_name}: {}", correct_case),
            edits,
            applicability: Applicability::Auto,
        }],
        help: None,
    };
    checker.emit_diagnostic(NameCaseChecker::rule(), diagnostic);
}

pub fn check_name_cases(checker: &mut Checker) -> Option<()> {
    // locals
    for file_info_iter in checker.file_db.iter() {
        if file_info_iter.is_stdlib_file() {
            continue;
        }

        let resolved_local_by_file = checker
            .type_db
            .project_index
            .get_resolved_uses(file_info_iter.id())?;

        for local_def in resolved_local_by_file.locals.iter() {
            // TODO: check type for params
            let cameled = local_def.name.to_lower_camel_case();
            let name = local_def.name.clone().to_string();

            if cameled == name {
                continue;
            }

            let usages = resolved_local_by_file.local_usages_of(local_def.id);
            let mut edits = vec![];
            // we need the definition itself too
            edits.push(Edit {
                span: local_def.def_span,
                replacement: cameled.clone(),
                file_id: file_info_iter.id(),
            });

            usages.for_each(|usage| {
                edits.push(Edit {
                    span: usage.span,
                    replacement: cameled.clone(),
                    file_id: file_info_iter.id(),
                });
            });

            let diagnostic = Diagnostic {
                file_id: file_info_iter.id(),
                severity: Severity::Warning,
                name: NameCaseChecker::rule().name(),
                code: NameCaseChecker::code().map(|c| c.to_string()),
                message: NameCaseChecker.message(),
                annotations: vec![Annotation {
                    span: local_def.def_span,
                    message: Some(format!("not camelCase: {name}",)),
                    is_primary: true,
                    tags: vec![DiagnosticTag::Unnecessary],
                }],
                fixes: vec![Fix {
                    message: format!("rename to camelCase: {}", cameled),
                    edits,
                    applicability: Applicability::Auto,
                }],
                help: None,
            };
            checker.emit_diagnostic(NameCaseChecker::rule(), diagnostic);
        }
    }

    // globals
    let globals = checker.type_db.project_index.global_symbols();

    for symbol_ids in globals.values() {
        for symbol_id in symbol_ids {
            let symbol = checker
                .type_db
                .project_index
                .resolve_symbol(*symbol_id)
                .unwrap();

            let symbol_def_file_id = symbol_id.file_id;
            let definition_info = checker.file_db.get_by_id(symbol_def_file_id)?;

            if definition_info.is_stdlib_file() {
                continue;
            }

            match &symbol.kind {
                tolk_resolver::SymbolKind::GlobalVariable
                | tolk_resolver::SymbolKind::Function { .. }
                | tolk_resolver::SymbolKind::StructField
                | tolk_resolver::SymbolKind::Method { .. }
                | tolk_resolver::SymbolKind::GetMethod { .. } => check_case(
                    symbol,
                    checker,
                    symbol_def_file_id,
                    CaseRules::LowerCamelCase,
                ),
                tolk_resolver::SymbolKind::Struct { .. }
                | tolk_resolver::SymbolKind::Enum { .. }
                | tolk_resolver::SymbolKind::EnumMember
                | tolk_resolver::SymbolKind::TypeAlias { .. } => check_case(
                    symbol,
                    checker,
                    symbol_def_file_id,
                    CaseRules::UpperCamelCase,
                ),
                tolk_resolver::SymbolKind::Constant => check_case(
                    symbol,
                    checker,
                    symbol_def_file_id,
                    CaseRules::ScreamingSnakeCase,
                ),
            }
        }
    }

    Some(())
}
