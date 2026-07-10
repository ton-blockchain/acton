use super::{CodeActionProvider, TolkCodeActionContext};
use crate::{CodeAction, Range, TextEdit};
use tolk_resolver::{Resolved, Symbol, SymbolId, SymbolKind};
use tolk_syntax::{AstNode, ObjectLit, StructField, TryFromNode};
use tolk_ty::{IntTy, TyData, TyId};

/// Fills an empty struct literal with all fields and, separately, with required fields only.
pub(super) struct FillStructFieldsProvider;

impl CodeActionProvider for FillStructFieldsProvider {
    fn collect(
        &self,
        context: &TolkCodeActionContext<'_>,
        actions: &mut Vec<CodeAction>,
    ) -> Option<()> {
        let object = context.ancestor_as::<ObjectLit>()?;
        if object.arguments().next().is_some() {
            return None;
        }
        let fields = resolve_fields(context, object)?;
        if fields.is_empty() {
            return None;
        }

        if let Some(edit) = fill_fields_edit(context, object, fields, true) {
            actions.push(context.action("Fill all fields...", edit));
        }
        if let Some(edit) = fill_fields_edit(context, object, fields, false) {
            actions.push(context.action("Fill required fields...", edit));
        }
        Some(())
    }
}

fn resolve_fields<'a>(
    context: &'a TolkCodeActionContext<'_>,
    object: ObjectLit<'_>,
) -> Option<&'a [Symbol]> {
    let symbol_id = object
        .typ()
        .and_then(|typ| {
            let Resolved::Global(symbol_id) = context
                .snapshot
                .resolved_at(context.file_id, typ.syntax().start_byte())?
            else {
                return None;
            };
            Some(symbol_id)
        })
        .or_else(|| {
            let ty = context.type_of_node(object)?;
            struct_id_for_type(context, ty)
        })?;
    let symbol = context.snapshot.project_index.resolve_symbol(symbol_id)?;
    match &symbol.kind {
        SymbolKind::Struct { fields, .. } => Some(fields),
        _ => None,
    }
}

fn struct_id_for_type(context: &TolkCodeActionContext<'_>, ty: TyId) -> Option<SymbolId> {
    match context.snapshot.type_interner.data(ty) {
        TyData::Struct { def, .. } => Some(*def),
        TyData::TypeAlias { inner_ty, .. } | TyData::GenericTypeWithTs { inner_ty, .. } => {
            struct_id_for_type(context, *inner_ty)
        }
        _ => None,
    }
}

fn fill_fields_edit(
    context: &TolkCodeActionContext<'_>,
    object: ObjectLit<'_>,
    fields: &[Symbol],
    all_fields: bool,
) -> Option<TextEdit> {
    let body = object.body()?;
    let open_brace = body.open_brace()?;
    let close_brace = body.close_brace()?;
    let fields = fields
        .iter()
        .filter_map(|field| {
            let value = field_default(context, field, all_fields)?;
            Some((field.name.as_ref(), value))
        })
        .collect::<Vec<_>>();
    if fields.is_empty() {
        return None;
    }

    let line = context
        .document
        .text()
        .lines()
        .nth(object.syntax().start_position().row)
        .unwrap_or_default();
    let indent = line
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    let field_indent = " ".repeat(indent + 4);
    let fields = fields
        .into_iter()
        .map(|(name, value)| format!("{field_indent}{name}: {value},"))
        .collect::<Vec<_>>()
        .join("\n");
    let single_line = open_brace.start_position().row == close_brace.end_position().row;
    let suffix = single_line.then(|| format!("\n{}", " ".repeat(indent)));
    let text = format!("\n{fields}{}", suffix.unwrap_or_default());
    let position = context
        .document
        .text_index()
        .offset_to_position(context.document.text(), open_brace.end_byte());
    Some(TextEdit::new(Range::new(position, position), text))
}

fn field_default(
    context: &TolkCodeActionContext<'_>,
    field: &Symbol,
    all_fields: bool,
) -> Option<String> {
    let file = context.snapshot.file_db.get_by_id(field.id.file_id)?;
    let identifier = file
        .source()
        .tree
        .root_node()
        .descendant_for_byte_range(field.name_span.start(), field.name_span.end())?;
    let declaration = StructField::try_from_node(identifier.parent()?).ok()?;
    if !all_fields && declaration.default().is_some() {
        return None;
    }
    if let Some(default) = declaration.default() {
        return Some(
            default
                .syntax()
                .text(file.source().source.as_ref())
                .to_owned(),
        );
    }
    Some(
        context
            .snapshot
            .type_db_cache
            .top_level_type(field.id)
            .map_or_else(|| "null".to_owned(), |ty| type_default(context, ty)),
    )
}

fn type_default(context: &TolkCodeActionContext<'_>, ty: TyId) -> String {
    match context.snapshot.type_interner.data(ty) {
        TyData::Union(elements) => elements
            .iter()
            .find(|element| matches!(context.snapshot.type_interner.data(**element), TyData::Null))
            .map_or_else(
                || {
                    elements
                        .first()
                        .map_or_else(|| "null".to_owned(), |ty| type_default(context, *ty))
                },
                |_| "null".to_owned(),
            ),
        TyData::Bool { .. } => "false".to_owned(),
        TyData::Int(IntTy::Coins) => "ton(\"0.1\")".to_owned(),
        TyData::Int(_) => "0".to_owned(),
        TyData::Bits { .. } | TyData::Bytes { .. } | TyData::Slice => {
            "createEmptySlice()".to_owned()
        }
        TyData::Address(_) => "address(\"\")".to_owned(),
        TyData::Builder => "beginCell()".to_owned(),
        TyData::Cell => "createEmptyCell()".to_owned(),
        TyData::Struct {
            name,
            args: Some(args),
            ..
        } if name.as_ref() == "Cell" => args.first().map_or_else(
            || "createEmptyCell()".to_owned(),
            |contained| format!("{}.toCell()", type_default(context, *contained)),
        ),
        TyData::Struct { name, .. } => format!("{name} {{}}"),
        TyData::Enum { def, name } => context
            .snapshot
            .project_index
            .resolve_symbol(*def)
            .and_then(|symbol| match &symbol.kind {
                SymbolKind::Enum { members } => members.first(),
                _ => None,
            })
            .map_or_else(|| name.to_string(), |member| member.name.to_string()),
        TyData::Tuple(elements) => format!("[{}]", type_defaults(context, elements)),
        TyData::Tensor(elements) => format!("({})", type_defaults(context, elements)),
        TyData::TypeAlias { inner_ty, .. } => type_default(context, *inner_ty),
        TyData::GenericTypeWithTs { inner_ty, types } => {
            if context.snapshot.type_interner.format(*inner_ty) == "Cell"
                && let Some(contained) = types.first()
            {
                format!("{}.toCell()", type_default(context, *contained))
            } else {
                type_default(context, *inner_ty)
            }
        }
        _ => "null".to_owned(),
    }
}

fn type_defaults(context: &TolkCodeActionContext<'_>, elements: &[TyId]) -> String {
    elements
        .iter()
        .map(|ty| type_default(context, *ty))
        .collect::<Vec<_>>()
        .join(", ")
}
