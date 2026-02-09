use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix, Severity};
use crate::rules::violation::Violation;
use crate::rules::violation::ViolationMetadata;
use crate::{Checker, FixAvailability};
use memchr::memchr;
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::{AstNode, InstanceArg};

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

pub fn check_instance_arg(
    checker: &mut Checker,
    file_id: FileId,
    argument: &InstanceArg,
) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let source = &file.source().source.as_bytes();

    // Well...
    // we can use code like this:
    // ```
    // let Some(Expr::Ident(ident)) = argument.value() else {
    //     // if argument doesn't have value or non-identifier value we can return early
    //     return None;
    // };
    // let Some(key) = argument.name() else { return None; };
    //
    // let same_name = checker.file_db.have_same_text(file_id, &key, &ident);
    // ```
    // But it **much** slower due syntax tree access.
    let syntax = argument.syntax();
    let start = syntax.start_byte();
    let end = syntax.end_byte();
    let slice = &source.get(start..end)?;
    let colon_pos = memchr(b':', slice)?; // no `:` means there is no value only key
    let key_bytes = slice[0..colon_pos].trim_ascii();
    let value_bytes = slice[colon_pos + 1..].trim_ascii();

    if key_bytes == value_bytes {
        // Foo { bar: bar }
        fire_diagnostic(checker, file_id, argument, key_bytes);
    }

    Some(())
}

#[cold]
#[inline(never)]
fn fire_diagnostic(
    checker: &mut Checker,
    file_id: FileId,
    argument: &InstanceArg,
    key_bytes: &[u8],
) {
    let key_name = String::from_utf8_lossy(key_bytes).to_string();
    let diagnostic = Diagnostic {
        file_id,
        severity: Severity::Warning,
        name: FieldInitCanBeFolded::rule().name(),
        code: FieldInitCanBeFolded::code().map(|c| c.to_string()),
        message: FieldInitCanBeFolded.message(),
        annotations: vec![Annotation {
            span: argument.span(),
            message: Some(format!("can be folded to just '{key_name}'")),
            is_primary: true,
            tags: vec![],
        }],
        fixes: vec![Fix {
            message: "fold initialization".to_string(),
            edits: vec![Edit {
                span: argument.span(),
                replacement: key_name,
                file_id: None,
            }],
            applicability: Applicability::Auto,
        }],
        help: None,
    };
    checker.emit_diagnostic(FieldInitCanBeFolded::rule(), diagnostic);
}
