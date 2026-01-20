use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix, Severity};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::{Expr, HasName, ObjectLit};

/// ### What it does
/// Checks for struct initialization where the field name and the variable used to initialize it are the same.
///
/// ### Why is this bad?
/// It's redundant and can be simplified using the shorthand syntax.
///
/// ### Example
/// ```tolk
/// struct Foo { bar: int }
///
/// fun main() {
///     val bar = 1;
///     val foo = Foo { bar: bar };
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// struct Foo { bar: int }
///
/// fun main() {
///     val bar = 1;
///     val foo = Foo { bar };
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct FieldInitCanBeFolded;

impl Violation for FieldInitCanBeFolded {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    fn message(&self) -> String {
        "field initialization can be folded".to_string()
    }
}

pub fn check_struct_literal(
    checker: &mut Checker,
    file_id: FileId,
    expr: &ObjectLit,
) -> Option<()> {
    let arguments = expr.arguments();
    for argument in arguments {
        let Some(Expr::Ident(ident)) = argument.value() else {
            // if argument doesn't have value or non-identifier value we can return early
            continue;
        };
        let Some(key) = argument.name() else { continue };

        let same_name = checker.file_db.have_same_text(file_id, &key, &ident);

        // Foo { bar: bar }
        if same_name {
            let key_name = checker.file_db.text_of(file_id, &key);
            let key_name_str = key_name.unwrap_or_default().to_owned();
            let diagnostic = Diagnostic {
                file_id,
                severity: Severity::Warning,
                message: FieldInitCanBeFolded.message(),
                annotations: vec![Annotation {
                    span: argument.span(),
                    message: Some(format!("can be folded to just '{}'", key_name_str)),
                    is_primary: true,
                    tags: vec![],
                }],
                fixes: vec![Fix {
                    message: "fold initialization".to_string(),
                    edits: vec![Edit {
                        span: argument.span(),
                        replacement: key_name_str.to_string(),
                    }],
                    applicability: Applicability::Auto,
                }],
            };
            checker.diagnostics.push(diagnostic);
        }
    }

    Some(())
}
