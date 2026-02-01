use crate::backend::utils::offset_to_pos;
use lsp_types::*;
use std::sync::Arc;
use tolk_resolver::FileResolveIndex;
use tolk_resolver::file_db::FileInfo;
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::resolve_index::LocalDefKind;
use tolk_syntax::{FunctionLike, TopLevel};
use tolk_ty::{InferenceResult, TyId, TypeInterner};

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
}

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

        if let LocalDefKind::Param { has_type, .. } = local_def.kind
            && has_type
        {
            // no need to show type hint for parameter with explicit type hint
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

        let type_string = interner.format(ty_id);

        let position = offset_to_pos(local_def.def_span.end(), &file.source().source);
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

fn collect_return_ty_hint(
    inference: &InferenceResult,
    interner: &TypeInterner,
    file: &Arc<FileInfo>,
    decl: &TopLevel,
    hints: &mut Vec<InlayHint>,
) {
    if let Some(inferred_ty) = inference.inferred_return_type
        && inferred_ty != interner.ty_unknown
    {
        match decl {
            TopLevel::Func(f) => {
                if f.return_type().is_none() {
                    add_return_type_hint(f, inferred_ty, interner, file, hints);
                }
            }
            TopLevel::Method(m) => {
                if m.return_type().is_none() {
                    add_return_type_hint(m, inferred_ty, interner, file, hints);
                }
            }
            TopLevel::GetMethod(g) => {
                if g.return_type().is_none() {
                    add_return_type_hint(g, inferred_ty, interner, file, hints);
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
    let Some(params_node) = node.syntax().child_by_field_name("parameters") else {
        return;
    };

    let position = offset_to_pos(params_node.end_byte(), &file_info.source().source);

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
