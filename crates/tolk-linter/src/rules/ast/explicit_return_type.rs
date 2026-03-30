use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, Span};
use tolk_syntax::{FunctionLike, Ident};
use tolk_ty::{InferenceResult, TyData, TyId, TypeInterner};

/// ### What it does
/// Requires explicit return types for function declarations.
///
/// ### Why is this bad?
/// Omitting return types makes APIs less clear and can hide accidental signature changes.
///
/// ### Example
/// ```tolk
/// fun buildSlice() {
///     return beginCell().toSlice();
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun buildSlice(): slice {
///     return beginCell().toSlice();
/// }
/// ```
///
/// ### Behavior notes
/// - Contract entrypoints `main`, `onInternalMessage`, `onExternalMessage`, `onRunTickTock`, `onSplitPrepare`, `onSplitInstall`, and `onBouncedMessage` are ignored by this rule.
/// - Functions whose inferred return type is `void` are ignored by this rule.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct ExplicitReturnType;

const EXEMPT_ENTRYPOINTS: [&str; 7] = [
    "main",
    "onInternalMessage",
    "onExternalMessage",
    "onRunTickTock",
    "onSplitPrepare",
    "onSplitInstall",
    "onBouncedMessage",
];

impl Violation for ExplicitReturnType {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    fn message(&self) -> String {
        "function return type should be explicit".to_string()
    }
}

pub fn check_return_type<'tree, T>(
    checker: &mut Checker,
    file_id: FileId,
    node: &T,
    inference: Option<&InferenceResult>,
) -> Option<()>
where
    T: FunctionLike<'tree>,
{
    if node.return_type().is_some() {
        return None;
    }

    let name = node.name();
    if let Some(name) = name.as_ref()
        && is_exempt_entrypoint(checker, file_id, name)
    {
        return None;
    }

    let name_span = name.map_or_else(|| node.span(), |name| name.span());
    let insert_offset = node.syntax().child_by_field_name("parameters")?.end_byte() as u32;

    fire_diagnostic(checker, file_id, name_span, insert_offset, inference)
}

fn is_exempt_entrypoint(checker: &Checker, file_id: FileId, name: &Ident) -> bool {
    let file_db = checker.file_db;
    EXEMPT_ENTRYPOINTS
        .iter()
        .any(|entrypoint| file_db.text_matches(file_id, name, entrypoint))
}

fn fire_diagnostic(
    checker: &mut Checker,
    file_id: FileId,
    name_span: Span,
    insert_offset: u32,
    inference: Option<&InferenceResult>,
) -> Option<()> {
    let ty = inferred_return_type(inference)?;

    let type_interner = &checker.type_db.intrn;
    if impossible_to_add_type(type_interner, ty) {
        return None;
    }

    let return_type = type_interner.format(ty);

    let diagnostic = Diagnostic::warning_for(file_id, ExplicitReturnType)
        .with_annotations(vec![Annotation {
            span: name_span,
            message: Some("return type is not explicitly declared".to_string()),
            is_primary: true,
            tags: vec![],
        }])
        .with_fixes(vec![Fix {
            message: format!("add `: {return_type}`"),
            edits: vec![Edit {
                span: Span {
                    start: insert_offset,
                    end: insert_offset,
                },
                replacement: format!(": {return_type}"),
                file_id,
            }],
            applicability: Applicability::Auto,
        }])
        .with_help("declare return type explicitly to keep function signatures clear and stable");
    checker.emit_diagnostic(diagnostic);
    Some(())
}

const fn inferred_return_type(inference: Option<&InferenceResult>) -> Option<TyId> {
    match inference {
        Some(inference) => inference.inferred_return_type,
        None => None,
    }
}

fn impossible_to_add_type(type_interner: &TypeInterner, ty: TyId) -> bool {
    matches!(
        type_interner.data(ty),
        TyData::Auto | TyData::Unknown | TyData::Undefined | TyData::Void
    )
}
