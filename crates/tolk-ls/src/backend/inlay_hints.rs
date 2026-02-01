use lsp_types::*;
use std::sync::Arc;
use tolk_resolver::file_db::FileInfo;
use tolk_resolver::file_index::FileId;
use tolk_resolver::project_index::ProjectIndex;
use tolk_syntax::{FunctionLike, TopLevel};
use tolk_ty::{InferenceResult, TyId, TypeInterner};
use crate::backend::utils::offset_to_lsp_pos;

pub fn collect_inlay_hints(
    inference_result: &InferenceResult,
    project_index: &ProjectIndex,
    type_interner: &TypeInterner,
    file_id: FileId,
    file_info: &Arc<FileInfo>,
    decl: &TopLevel,
    hints: &mut Vec<InlayHint>,
) {
    let Some(file_resolve_index) = project_index.resolved_uses.get(&file_id) else {
        return;
    };

    for local_def in &file_resolve_index.locals {
        match local_def.kind {
            tolk_resolver::resolve_index::LocalDefKind::Param { .. }
            | tolk_resolver::resolve_index::LocalDefKind::Var { .. }
            | tolk_resolver::resolve_index::LocalDefKind::Catch => {
                if let Some(ty_id) = inference_result.type_of(local_def.def_span) {
                    if ty_id == type_interner.ty_unknown {
                        continue;
                    }

                    let type_string = type_interner.display(ty_id).to_string();

                    let position = offset_to_lsp_pos(
                        local_def.def_span.end as usize,
                        &file_info.source().source,
                    );
                    let hint = InlayHint {
                        position,
                        label: InlayHintLabel::String(format!(": {}", type_string)),
                        kind: Some(InlayHintKind::TYPE),
                        text_edits: None,
                        tooltip: None,
                        padding_left: Some(false),
                        padding_right: Some(true),
                        data: None,
                    };
                    hints.push(hint);
                }
            }
            _ => {}
        }
    }

    if let Some(inferred_ty) = inference_result.inferred_return_type
        && inferred_ty != type_interner.ty_unknown
    {
        match decl {
            TopLevel::Func(f) => {
                if f.return_type().is_none() {
                    add_return_type_hint(f, inferred_ty, type_interner, file_info, hints);
                }
            }
            TopLevel::Method(m) => {
                if m.return_type().is_none() {
                    add_return_type_hint(m, inferred_ty, type_interner, file_info, hints);
                }
            }
            TopLevel::GetMethod(g) => {
                if g.return_type().is_none() {
                    add_return_type_hint(g, inferred_ty, type_interner, file_info, hints);
                }
            }
            _ => {}
        }
    }
}

fn add_return_type_hint<'tree, T: tolk_syntax::AstNode<'tree>>(
    node: &T,
    return_ty: TyId,
    type_interner: &TypeInterner,
    file_info: &Arc<FileInfo>,
    hints: &mut Vec<InlayHint>,
) {
    let parameters_node = node.syntax().child_by_field_name("parameters");
    if let Some(params_node) = parameters_node {
        let position =
            offset_to_lsp_pos(params_node.end_byte(), &file_info.source().source);

        let type_string = type_interner.display(return_ty).to_string();

        let hint = InlayHint {
            position,
            label: InlayHintLabel::String(format!(": {}", type_string)),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: Some(true),
            padding_right: Some(true),
            data: None,
        };
        hints.push(hint);
    }
}
