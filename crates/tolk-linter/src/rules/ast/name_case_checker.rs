use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, DiagnosticTag, Edit, Fix};
use crate::rules::utils;
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use heck::{ToLowerCamelCase, ToShoutySnakeCase, ToUpperCamelCase};
use tolk_macros::ViolationMetadata;
use tolk_resolver::Symbol;
use tolk_resolver::file_index::FileId;
use tolk_resolver::resolve_index::LocalDefKind;
use tolk_ty::GlobalUsages;

/// ### What it does
/// Checks identifier naming style and suggests consistent casing.
///
/// ### Why is this bad?
/// Inconsistent naming makes code harder to read and maintain.
/// This rule enforces:
/// - `camelCase` for variables, functions, methods, and struct fields
/// - `PascalCase` for structs, enums, enum members, and type aliases
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
    Camel,
    Pascal,
    ScreamingSnake,
}

fn check_case(symbol: &Symbol, checker: &mut Checker, symbol_def_file_id: FileId, case: CaseRules) {
    if symbol.name.starts_with('_') {
        // internal names
        return;
    }

    let (correct_case, case_name) = match case {
        CaseRules::Camel => {
            if utils::cases::is_camel_ascii(symbol.name.as_ref()) {
                return;
            }
            (symbol.name.to_lower_camel_case(), "camelCase")
        }
        CaseRules::Pascal => {
            if utils::cases::is_pascal_ascii(symbol.name.as_ref()) {
                return;
            }
            (symbol.name.to_upper_camel_case(), "PascalCase")
        }
        CaseRules::ScreamingSnake => {
            if utils::cases::is_screaming_snake_ascii(symbol.name.as_ref()) {
                return;
            }
            (symbol.name.to_shouty_snake_case(), "SCREAMING_SNAKE_CASE")
        }
    };

    if symbol.name.as_bytes() == correct_case.as_bytes() {
        return;
    }

    let mut edits = vec![
        // definition itself
        Edit {
            span: symbol.name_span,
            replacement: correct_case.clone(),
            file_id: symbol_def_file_id,
        },
    ];

    let usages = GlobalUsages::new(checker.type_db.project_index, checker.body_types);
    for usage in usages.for_symbol(symbol.id) {
        edits.push(Edit {
            span: usage.usage.span,
            replacement: correct_case.clone(),
            file_id: usage.file_id,
        });
    }

    let diagnostic = Diagnostic::warning_for(symbol_def_file_id, NameCaseChecker)
        .with_annotations(vec![Annotation {
            span: symbol.name_span,
            message: Some(format!("not {case_name}: `{}`", symbol.name)),
            is_primary: true,
            tags: vec![DiagnosticTag::Unnecessary],
        }])
        .with_fixes(vec![Fix {
            message: format!("rename to {case_name}: {correct_case}"),
            edits,
            applicability: Applicability::Auto,
        }]);
    checker.emit_diagnostic(diagnostic);
}

pub fn check_name_cases(checker: &mut Checker) -> Option<()> {
    for file_index in checker.type_db.project_index.workspace_files() {
        let file_id = file_index.id;
        let Some(resolve_index) = checker.resolve_index_for(file_id) else {
            continue;
        };

        // First check local declarations
        for local_def in &resolve_index.locals {
            let name = local_def.name.clone();
            if name.starts_with('_') {
                // don't check explicitly unused symbols
                // we also skip something like `_foo_bar` but I think it's ok
                continue;
            }

            let (correct_case, case_name) = if matches!(local_def.kind, LocalDefKind::TypeParameter)
            {
                if utils::cases::is_pascal_ascii(name.as_ref()) {
                    continue;
                }
                (name.to_upper_camel_case(), "PascalCase")
            } else {
                if utils::cases::is_camel_ascii(name.as_ref()) {
                    continue;
                }
                (name.to_lower_camel_case(), "camelCase")
            };

            if correct_case.as_bytes() == name.as_bytes() {
                continue;
            }

            let usages = resolve_index.local_usages_of(local_def.id);
            let mut edits = vec![
                // definition itself
                Edit {
                    span: local_def.def_span,
                    replacement: correct_case.clone(),
                    file_id,
                },
            ];

            for usage in usages {
                edits.push(Edit {
                    span: usage.span,
                    replacement: correct_case.clone(),
                    file_id,
                });
            }

            let diagnostic = Diagnostic::warning_for(file_id, NameCaseChecker)
                .with_annotations(vec![Annotation {
                    span: local_def.def_span,
                    message: Some(format!("not {case_name}: {name}",)),
                    is_primary: true,
                    tags: vec![DiagnosticTag::Unnecessary],
                }])
                .with_fixes(vec![Fix {
                    message: format!("rename to {case_name}: {correct_case}"),
                    edits,
                    applicability: Applicability::Auto,
                }]);
            checker.emit_diagnostic(diagnostic);
        }

        // And then global ones
        for symbol in &file_index.decls {
            match &symbol.kind {
                tolk_resolver::SymbolKind::GetMethod { .. } => {
                    // Since the get method name defines the method ID and there are names from TEPs in snake case (e.g. `get_wallet_info`),
                    // we cannot warn about the get method names
                    continue;
                }
                tolk_resolver::SymbolKind::GlobalVariable
                | tolk_resolver::SymbolKind::Function { .. }
                | tolk_resolver::SymbolKind::Method { .. } => {
                    check_case(symbol, checker, file_id, CaseRules::Camel);
                }
                tolk_resolver::SymbolKind::Struct { fields, .. } => {
                    check_case(symbol, checker, file_id, CaseRules::Pascal);
                    for field in fields {
                        check_case(field, checker, file_id, CaseRules::Camel);
                    }
                }
                tolk_resolver::SymbolKind::Enum { members } => {
                    check_case(symbol, checker, file_id, CaseRules::Pascal);
                    for member in members {
                        check_case(member, checker, file_id, CaseRules::Pascal);
                    }
                }
                tolk_resolver::SymbolKind::TypeAlias { .. } => {
                    check_case(symbol, checker, file_id, CaseRules::Pascal);
                }
                tolk_resolver::SymbolKind::Constant => {
                    check_case(symbol, checker, file_id, CaseRules::ScreamingSnake);
                }
                tolk_resolver::SymbolKind::StructField | tolk_resolver::SymbolKind::EnumMember => {
                    // checked in struct and enum arms
                }
            }
        }
    }

    Some(())
}
