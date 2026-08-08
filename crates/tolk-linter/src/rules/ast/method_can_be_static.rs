use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, Span, SymbolId};
use tolk_resolver::resolve_index::{FileResolveIndex, LocalDefId, LocalDefKind, NameUse, Resolved};
use tolk_resolver::{AstNodeSpanExt, SymbolKind};
use tolk_syntax::TryFromNode;
use tolk_syntax::ast::expressions::{Call, DotAccess};
use tolk_ty::InferenceResult;
use tree_sitter::Node;

/// ### What it does
/// Checks for instance methods where `self` is unnecessary.
///
/// ### Why is this bad?
/// If `self` is never used, the method can be static which makes the API clearer.
///
/// ### Example
/// ```tolk twoslash
/// struct Foo {}
///
/// fun Foo.bar(self, a: int): int {
/// //          ^^^^ S004: method can be static
///     return a + 1;
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// struct Foo {}
///
/// fun Foo.bar(a: int): int {
///     return a + 1;
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct MethodCanBeStatic;

impl Violation for MethodCanBeStatic {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "method can be static".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let resolve_index = checker.resolve_index_for(file_id)?;
    let root = file.source().tree.root_node();
    let use_facts = checker.use_facts(file_id)?;
    let per_decl_inference = checker.body_types.get(&file_id)?;

    for self_local in &resolve_index.locals {
        // find all self parameters in the file
        if !matches!(
            self_local.kind,
            LocalDefKind::Param {
                is_self: true,
                in_asm_or_builtin: false,
                ..
            }
        ) {
            continue;
        }

        let Some(facts) = use_facts.per_local.get(&self_local.id) else {
            continue;
        };

        // then for each self find owner symbol for it
        let Some(owner_symbol) = file.find_symbol_at(self_local.def_span.start()) else {
            continue;
        };
        if !matches!(
            &owner_symbol.kind,
            SymbolKind::Method {
                is_instance: true,
                ..
            }
        ) {
            continue;
        }

        if facts.flags.is_empty() {
            // if self is unused we can make method static
            fire_diagnostic(
                checker,
                file_id,
                self_local.def_span,
                &owner_symbol.name,
                SelfUsageKind::Unused,
            );
            continue;
        }

        let Some(method_inference) = per_decl_inference.get(&owner_symbol.id) else {
            continue;
        };

        if is_used_only_for_recursion(
            root,
            &resolve_index,
            self_local.id,
            owner_symbol.id,
            method_inference,
        ) {
            fire_diagnostic(
                checker,
                file_id,
                self_local.def_span,
                &owner_symbol.name,
                SelfUsageKind::RecursiveOnly,
            );
        }
    }

    Some(())
}

#[derive(Debug, Clone, Copy)]
enum SelfUsageKind {
    Unused,
    RecursiveOnly,
}

fn is_used_only_for_recursion(
    root: Node,
    resolve_index: &FileResolveIndex,
    self_local_id: LocalDefId,
    method_symbol_id: SymbolId,
    method_inference: &InferenceResult,
) -> bool {
    let mut has_usages = false;
    for usage in resolve_index.local_usages_of(self_local_id) {
        has_usages = true;
        if !is_recursive_self_call_usage(
            root,
            resolve_index,
            usage,
            method_symbol_id,
            method_inference,
        ) {
            return false;
        }
    }

    has_usages
}

fn is_recursive_self_call_usage(
    root: Node,
    resolve_index: &FileResolveIndex,
    usage: &NameUse,
    method_symbol_id: SymbolId,
    method_inference: &InferenceResult,
) -> bool {
    let Some(usage_node) = root.descendant_for_byte_range(usage.span.start(), usage.span.end())
    else {
        return false;
    };

    let Some(call) = find_call_with_usage_as_receiver(usage_node) else {
        return false;
    };

    is_same_method_call(resolve_index, &call, method_symbol_id, method_inference)
}

fn find_call_with_usage_as_receiver(usage_node: Node) -> Option<Call> {
    let mut current = usage_node.parent();
    while let Some(node) = current {
        if let Ok(dot_access) = DotAccess::try_from_node(node)
            && dot_access.is_obj(&usage_node)
            && let Some(parent) = node.parent()
            && let Ok(call) = Call::try_from_node(parent)
        {
            return Some(call);
        }
        current = node.parent();
    }
    None
}

fn is_same_method_call(
    resolve_index: &FileResolveIndex,
    call: &Call,
    method_symbol_id: SymbolId,
    method_inference: &InferenceResult,
) -> bool {
    let Some(callee_ident) = call.callee_identifier() else {
        return false;
    };

    if let Some(name_use) = resolve_index.find_use(callee_ident.start_byte())
        && let Resolved::Global(symbol_id) = name_use.resolved
    {
        return symbol_id == method_symbol_id;
    }

    if let Some(name_use) = method_inference.resolve(callee_ident.span())
        && let Resolved::Global(symbol_id) = name_use.resolved
    {
        return symbol_id == method_symbol_id;
    }

    false
}

#[cold]
fn fire_diagnostic(
    checker: &mut Checker,
    file_id: FileId,
    self_span: Span,
    method_name: &str,
    usage_kind: SelfUsageKind,
) {
    let (annotation_message, help) = match usage_kind {
        SelfUsageKind::Unused => (
            "receiver `self` is not used".to_string(),
            "remove `self` parameter to make this method static".to_string(),
        ),
        SelfUsageKind::RecursiveOnly => (
            "receiver `self` is used only for recursion".to_string(),
            "replace `self.<method>(...)` with a static recursive call and remove `self` parameter"
                .to_string(),
        ),
    };

    let diagnostic = Diagnostic::warning_for(file_id, MethodCanBeStatic)
        .with_annotations(vec![Annotation {
            span: self_span,
            message: Some(format!("{annotation_message} in `{method_name}`")),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(help);

    checker.emit_diagnostic(diagnostic);
}
