use crate::backend::Backend;
use crate::backend::utils::SpanExt;
use lsp_types::*;
use std::sync::Arc;
use tolk_resolver::file_db::FileInfo;
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::resolve_index::LocalDefKind;
use tolk_resolver::{AstNodeSpanExt, FileResolveIndex};
use tolk_syntax::ast::expressions::Expr;
use tolk_syntax::{AstNode, FunctionLike, HasName, TopLevel};
use tolk_ty::{InferenceResult, TyId, TypeInterner};
use tower_lsp::jsonrpc::Result as LspResult;

impl Backend {
    pub async fn handle_inlay_hint(
        &self,
        params: InlayHintParams,
    ) -> LspResult<Option<Vec<InlayHint>>> {
        crate::profile!(self, "inlay_hint");
        let now = std::time::Instant::now();
        let uri = params.text_document.uri;
        log::info!("Request: inlay_hint for {}", uri);

        let result = (|| {
            let analysis = self.analysis.get(&uri)?;
            let path = uri.to_file_path().ok()?;
            let file_info = self.file_db.get_by_path(&path)?;

            let mut hints = Vec::with_capacity(10);

            let body_types = analysis.all_body_types.get(&file_info.id())?;

            for (&symbol_id, inference_result) in body_types {
                let decl = file_info.find_syntax_declaration(symbol_id);
                let Some(decl) = decl else { continue };

                collect_inlay_hints(
                    inference_result,
                    &analysis.project_index,
                    &analysis.type_interner,
                    &file_info,
                    &decl,
                    &mut hints,
                );
            }

            Some(hints)
        })();

        log::info!("Response: inlay_hint took {:?}", now.elapsed());
        Ok(result)
    }
}

pub fn collect_inlay_hints(
    inference: &InferenceResult,
    project_index: &ProjectIndex,
    interner: &TypeInterner,
    file: &Arc<FileInfo>,
    decl: &TopLevel,
    hints: &mut Vec<InlayHint>,
) {
    let Some(resolve_index) = project_index.get_resolved_uses(file.id()) else {
        return;
    };

    collect_locals_hints(inference, interner, file, hints, resolve_index);
    collect_return_ty_hint(inference, interner, file, decl, hints);
    collect_constants_hints(inference, interner, file, decl, hints);
}

/// Collect hints for local variables and parameters.
///
/// ```tolk
/// fun main() {
///     val a/*: int*/ = 100;
///     //   ^^^^^^^^^ this one
/// }
/// ```
fn collect_locals_hints(
    inference: &InferenceResult,
    interner: &TypeInterner,
    file: &Arc<FileInfo>,
    hints: &mut Vec<InlayHint>,
    resolve_index: &Arc<FileResolveIndex>,
) {
    for local_def in &resolve_index.locals {
        if matches!(local_def.kind, LocalDefKind::TypeParameter) {
            // no need to show type hint for type parameters
            continue;
        }

        if local_def.name.starts_with("_") {
            // no need to show type hint for _ or _foo
            continue;
        }

        if let LocalDefKind::Param {
            has_type, is_self, ..
        } = local_def.kind
            && (has_type || is_self)
        {
            // no need to show type hint for parameter with explicit type hint
            // or if it is self parameter
            continue;
        }

        if let LocalDefKind::Var { has_type, .. } = local_def.kind
            && has_type
        {
            // no need to show type hint for variable with explicit type hint
            continue;
        }

        let Some(ty_id) = inference.type_of(local_def.def_span) else {
            continue;
        };

        if ty_id == interner.ty_unknown {
            continue;
        }

        hints.push(create_type_hint(
            local_def.def_span.end_position(file),
            interner.format(ty_id),
        ));
    }
}

/// Collect hints for return type of functions
///
/// ```tolk
/// fun main()/*: void*/ {
///     //    ^^^^^^^^^^ this one
/// }
/// ```
fn collect_return_ty_hint(
    inference: &InferenceResult,
    interner: &TypeInterner,
    file: &Arc<FileInfo>,
    decl: &TopLevel,
    hints: &mut Vec<InlayHint>,
) {
    let Some(inferred_ty) = inference.inferred_return_type else {
        return;
    };

    if inferred_ty == interner.ty_unknown {
        return;
    }

    match decl {
        TopLevel::Func(f) if f.return_type().is_none() => {
            add_return_type_hint(f, inferred_ty, interner, file, hints);
        }
        TopLevel::Method(m) if m.return_type().is_none() => {
            add_return_type_hint(m, inferred_ty, interner, file, hints);
        }
        TopLevel::GetMethod(m) if m.return_type().is_none() => {
            add_return_type_hint(m, inferred_ty, interner, file, hints);
        }
        _ => {}
    }
}

fn add_return_type_hint<'tree, T: AstNode<'tree>>(
    node: &T,
    return_ty: TyId,
    interner: &TypeInterner,
    file: &Arc<FileInfo>,
    hints: &mut Vec<InlayHint>,
) {
    let Some(params_node) = node.syntax().child_by_field_name("parameters") else {
        return;
    };

    hints.push(create_type_hint(
        params_node.span().end_position(file),
        interner.format(return_ty),
    ));
}

/// Collect hints for constants.
///
/// ```tolk
/// const FOO/*: int*/ = 100
/// //       ^^^^^^^^^ this one
/// ```
fn collect_constants_hints(
    inference: &InferenceResult,
    interner: &TypeInterner,
    file: &Arc<FileInfo>,
    decl: &TopLevel,
    hints: &mut Vec<InlayHint>,
) {
    let TopLevel::Constant(c) = decl else {
        return;
    };

    if c.typ().is_some() {
        // already have type hint
        return;
    }

    let Some(name) = c.name() else {
        // no name, likely incomplete code
        return;
    };

    let Some(expr) = c.value() else {
        // no value, likely incomplete code
        return;
    };

    let source = &file.source().source;
    if has_obvious_type(&expr, source) {
        return;
    }

    let Some(ty_id) = inference.type_of(expr.span()) else {
        return;
    };

    if ty_id == interner.ty_unknown {
        return;
    }

    hints.push(create_type_hint(
        name.span().end_position(file),
        interner.format(ty_id),
    ));
}

fn has_obvious_type(expr: &Expr, source: &str) -> bool {
    match expr {
        // don't show a hint for:
        // val params = SomeParams{}
        Expr::ObjectLit(_) => true,
        // don't show a hint for:
        // val foo = Foo.fromCell(cell)
        Expr::Call(call) => {
            if let Some(callee) = call.callee_identifier() {
                let name = callee.text(source);
                if name == "fromCell" || name == "fromSlice" {
                    return true;
                }
            }
            false
        }
        // don't show a hint for:
        // val params = lazy SomeParams.fromCell()
        Expr::Lazy(lazy) => {
            let Some(inner) = lazy.expr() else {
                return false;
            };
            has_obvious_type(&inner, source)
        }
        _ => false,
    }
}

fn create_type_hint(position: Position, typ: String) -> InlayHint {
    InlayHint {
        position,
        label: InlayHintLabel::String(format!(": {typ}")),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: None,
        padding_left: Some(false),
        padding_right: Some(false),
        data: None,
    }
}
