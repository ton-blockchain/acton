use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, Symbol, SymbolKind};
use tree_sitter::Node;

/// ### What it does
/// Warns when code uses the low-level `dict` type.
///
/// ### Why is this bad?
/// `dict` erases key and value types, which makes code less type-safe and harder to understand.
/// In most cases, `map<K, V>` communicates intent better and gives stronger type checking.
///
/// ### Example
/// ```tolk
/// fun main(data: dict) {}
/// ```
///
/// Use instead:
/// ```tolk
/// fun main(data: map<uint32, cell>) {}
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct DictTypeUse;

impl Violation for DictTypeUse {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "low-level `dict` type should be replaced with typed `map<K, V>`".to_owned()
    }
}

pub fn check_resolved_reference(
    checker: &mut Checker,
    file_id: FileId,
    node: &Node,
    symbol: &Symbol,
) -> Option<()> {
    if !matches!(symbol.kind, SymbolKind::TypeAlias { .. }) || symbol.name.as_ref() != "dict" {
        return None;
    }

    let diagnostic = Diagnostic::warning_for(file_id, DictTypeUse)
        .with_annotations(vec![Annotation {
            span: node.span(),
            message: Some("prefer `map<K, V>` over low-level `dict`".to_owned()),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(
            "Use `map<K, V>` when key and value types are known; keep `dict` only for low-level TVM interop boundaries.",
        );
    checker.emit_diagnostic(diagnostic);

    Some(())
}
