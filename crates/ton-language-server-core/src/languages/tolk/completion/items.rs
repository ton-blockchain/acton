use super::context::DUMMY_IDENTIFIER;
use super::{TolkCompletionProviderContext, semantics};
use crate::completion::{CompletionCategory, CompletionRank};
use crate::{CompletionItem, CompletionItemKind};
use tolk_resolver::file_index::Parameter;
use tolk_resolver::resolve_index::{LocalDef, LocalDefKind};
use tolk_resolver::{Symbol, SymbolKind};
use tolk_syntax::{BaseFunction, EnumMember, TopLevel, TryFromNode};
use tolk_ty::TyData;

pub(super) struct RankedCompletionItem {
    pub(super) item: CompletionItem,
    pub(super) rank: CompletionRank,
}

pub(super) fn local(
    context: &TolkCompletionProviderContext<'_>,
    local: &LocalDef,
) -> Option<RankedCompletionItem> {
    let is_type_parameter = matches!(local.kind, LocalDefKind::TypeParameter);
    if context.syntax.is_type() != is_type_parameter
        || local.name.is_empty()
        || local.name.as_ref() == "_"
        || local.name.starts_with("__")
    {
        return None;
    }
    let (kind, category) = match local.kind {
        LocalDefKind::Param { .. } => (CompletionItemKind::Variable, CompletionCategory::Parameter),
        LocalDefKind::TypeParameter => (
            CompletionItemKind::TypeParameter,
            CompletionCategory::Parameter,
        ),
        LocalDefKind::Var { .. } | LocalDefKind::Catch => {
            (CompletionItemKind::Variable, CompletionCategory::Variable)
        }
    };
    let raw_name = semantics::raw_text(context.snapshot, context.file_id, local.def_span)
        .unwrap_or_else(|| local.name.to_string());
    let mut item = CompletionItem::new(local.name.as_ref(), kind)
        .with_filter_text(local.name.as_ref())
        .with_replacement(context.syntax.replacement_range, raw_name);
    if let Some(ty) = semantics::local_type(context, local) {
        if is_type_parameter {
            if let TyData::TypeParameter {
                default_type: Some(default_type),
                ..
            } = context.snapshot.type_interner.data(ty)
            {
                item = item.with_label_detail(format!(
                    " = {}",
                    context.snapshot.type_interner.format(*default_type)
                ));
            }
            item = item.with_label_description("type parameter");
        } else {
            item = item
                .with_label_description(format!(" {}", context.snapshot.type_interner.format(ty)));
        }
    }
    Some(RankedCompletionItem {
        item,
        rank: CompletionRank::new(category)
            .with_prefix(&context.syntax.prefix, local.name.as_ref())
            .with_locality_penalty(
                u16::try_from(context.syntax.offset.saturating_sub(local.def_span.start()))
                    .unwrap_or(u16::MAX),
            ),
    })
}

pub(super) fn symbol(
    context: &TolkCompletionProviderContext<'_>,
    symbol: &Symbol,
    member: bool,
) -> Option<RankedCompletionItem> {
    let raw_name = semantics::raw_text(context.snapshot, symbol.id.file_id, symbol.name_span)
        .unwrap_or_else(|| symbol.name.to_string());
    if raw_name.ends_with(DUMMY_IDENTIFIER) || raw_name == "_" {
        return None;
    }
    let (kind, category, insertion) = match &symbol.kind {
        SymbolKind::Function { parameters, .. } | SymbolKind::GetMethod { parameters, .. } => (
            CompletionItemKind::Function,
            CompletionCategory::Function,
            callable_snippet(&raw_name, parameters, false, context),
        ),
        SymbolKind::Method {
            parameters,
            is_instance,
            ..
        } => (
            CompletionItemKind::Method,
            CompletionCategory::Function,
            callable_snippet(&raw_name, parameters, *is_instance, context),
        ),
        SymbolKind::Struct { fields, .. } => {
            let insertion = if context.syntax.is_type() || member || fields.is_empty() {
                raw_name
            } else {
                format!("{raw_name} {{$1}}$0")
            };
            (
                CompletionItemKind::Struct,
                CompletionCategory::Struct,
                insertion,
            )
        }
        SymbolKind::Enum { .. } => (CompletionItemKind::Enum, CompletionCategory::Enum, raw_name),
        SymbolKind::TypeAlias { .. } => (
            CompletionItemKind::TypeParameter,
            CompletionCategory::TypeAlias,
            raw_name,
        ),
        SymbolKind::Constant => (
            CompletionItemKind::Constant,
            CompletionCategory::Constant,
            raw_name,
        ),
        SymbolKind::GlobalVariable => (
            CompletionItemKind::Variable,
            CompletionCategory::Global,
            raw_name,
        ),
        SymbolKind::StructField => (
            CompletionItemKind::Property,
            CompletionCategory::Field,
            raw_name,
        ),
        SymbolKind::EnumMember => (
            CompletionItemKind::EnumMember,
            CompletionCategory::Field,
            raw_name,
        ),
    };
    let mut item = CompletionItem::new(symbol.name.as_ref(), kind)
        .with_filter_text(symbol.name.as_ref())
        .with_snippet_replacement(context.syntax.replacement_range, insertion);
    item = with_symbol_label_details(item, context, symbol, member);
    item.deprecated = symbol.is_deprecated;
    if let Some(documentation) = symbol
        .doc_span
        .and_then(|span| semantics::raw_text(context.snapshot, symbol.id.file_id, span))
    {
        item.documentation = Some(documentation);
    }
    Some(RankedCompletionItem {
        item,
        rank: CompletionRank::new(category)
            .with_prefix(&context.syntax.prefix, symbol.name.as_ref()),
    })
}

pub(super) fn prefixed_enum_member(
    context: &TolkCompletionProviderContext<'_>,
    owner: &Symbol,
    member: &Symbol,
) -> RankedCompletionItem {
    let raw_owner = semantics::raw_text(context.snapshot, owner.id.file_id, owner.name_span)
        .unwrap_or_else(|| owner.name.to_string());
    let raw_member = semantics::raw_text(context.snapshot, member.id.file_id, member.name_span)
        .unwrap_or_else(|| member.name.to_string());
    let label = format!("{raw_owner}.{raw_member}");
    let item = CompletionItem::new(&label, CompletionItemKind::EnumMember)
        .with_filter_text(member.name.as_ref())
        .with_replacement(context.syntax.replacement_range, &label);
    RankedCompletionItem {
        item: with_symbol_label_details(item, context, member, true),
        rank: CompletionRank::new(CompletionCategory::Field)
            .with_prefix(&context.syntax.prefix, member.name.as_ref()),
    }
}

pub(super) fn with_symbol_label_details(
    mut item: CompletionItem,
    context: &TolkCompletionProviderContext<'_>,
    symbol: &Symbol,
    member: bool,
) -> CompletionItem {
    match &symbol.kind {
        SymbolKind::Function { .. } | SymbolKind::GetMethod { .. } => {
            if let Some(signature) = callable_label_detail(context, symbol) {
                item = item.with_label_detail(signature);
            }
        }
        SymbolKind::Method { is_instance, .. } => {
            if let Some(signature) = callable_label_detail(context, symbol) {
                item = item.with_label_detail(signature);
            }

            if !is_instance
                && let Some(receiver) = context
                    .snapshot
                    .type_db_cache
                    .method_receiver_type(symbol.id)
            {
                item = item.with_label_description(format!(
                    "of {}",
                    context.snapshot.type_interner.format(receiver)
                ));
            }
        }
        SymbolKind::Struct { fields, .. } => {
            if !context.syntax.is_type() && !member && !fields.is_empty() {
                item = item.with_label_detail(" {}");
            }
        }
        SymbolKind::Constant => {
            if let Some(typ) = symbol_type(context, symbol) {
                let value = constant_value(context, symbol)
                    .map(|value| format!(" = {value}"))
                    .unwrap_or_default();
                item = item.with_label_detail(format!(": {typ}{value}"));
            }
        }
        SymbolKind::GlobalVariable => {
            if let Some(typ) = symbol_type(context, symbol) {
                item = item.with_label_detail(format!(": {typ}"));
            }
        }
        SymbolKind::StructField => {
            if let Some(typ) = symbol_type(context, symbol) {
                item = item.with_label_detail(format!(": {typ}"));
            }
            if let Some(owner) = owner_name(symbol) {
                item = item.with_label_description(format!(" of {owner}"));
            }
        }
        SymbolKind::EnumMember => {
            if let Some(default) = enum_member_default(context, symbol) {
                item = item.with_label_detail(format!(" = {default}"));
            }
            if let Some(owner) = owner_name(symbol) {
                item = item.with_label_description(format!(" of {owner}"));
            }
        }
        SymbolKind::Enum { .. } | SymbolKind::TypeAlias { .. } => {}
    }

    item
}

fn callable_label_detail(
    context: &TolkCompletionProviderContext<'_>,
    symbol: &Symbol,
) -> Option<String> {
    let file = context.snapshot.file_db.get_by_id(symbol.id.file_id)?;
    let declaration = file.find_syntax_declaration(symbol.id)?;
    let function = BaseFunction::try_from_node(declaration.syntax()).ok()?;
    let type_parameters = function
        .type_parameters_node()
        .map(|parameters| file.text(&parameters))
        .unwrap_or_default();
    let parameters = function
        .parameters()
        .map(|parameter| file.text(&parameter))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = function
        .return_type()
        .map(|return_type| format!(": {}", file.text(&return_type)))
        .unwrap_or_default();

    Some(format!("{type_parameters}({parameters}){return_type}"))
}

fn symbol_type(context: &TolkCompletionProviderContext<'_>, symbol: &Symbol) -> Option<String> {
    context
        .snapshot
        .type_db_cache
        .top_level_type(symbol.id)
        .map(|ty| context.snapshot.type_interner.format(ty))
}

fn constant_value(context: &TolkCompletionProviderContext<'_>, symbol: &Symbol) -> Option<String> {
    let file = context.snapshot.file_db.get_by_id(symbol.id.file_id)?;
    let TopLevel::Constant(constant) = file.find_syntax_declaration(symbol.id)? else {
        return None;
    };
    let value = constant.value()?;

    Some(file.text(&value).to_owned())
}

fn enum_member_default(
    context: &TolkCompletionProviderContext<'_>,
    symbol: &Symbol,
) -> Option<String> {
    let file = context.snapshot.file_db.get_by_id(symbol.id.file_id)?;
    let source_file = file.source();
    let node = source_file
        .tree
        .root_node()
        .descendant_for_byte_range(symbol.body_span.start(), symbol.body_span.end())?;
    let member = EnumMember::try_from_node(node).ok()?;
    let default = member.default()?;

    Some(file.text(&default).to_owned())
}

fn owner_name(symbol: &Symbol) -> Option<&str> {
    symbol.fqn.rsplit_once('.').map(|(owner, _)| owner)
}

fn callable_snippet(
    name: &str,
    parameters: &[Parameter],
    skip_self: bool,
    context: &TolkCompletionProviderContext<'_>,
) -> String {
    let mut result = name.to_owned();
    if !context.syntax.before_paren {
        let parameters = parameters
            .iter()
            .filter(|parameter| !(skip_self && parameter.name.as_ref() == "self"))
            .collect::<Vec<_>>();
        if parameters.is_empty() {
            result.push_str("()");
        } else {
            result.push('(');
            for (index, parameter) in parameters.iter().enumerate() {
                if index > 0 {
                    result.push_str(", ");
                }
                result.push_str("${");
                result.push_str(&(index + 1).to_string());
                result.push(':');
                result.push_str(parameter.name.as_ref());
                result.push('}');
            }
            result.push(')');
        }
    }
    if context.syntax.needs_semicolon_for_call() {
        result.push(';');
    }
    result.push_str("$0");
    result
}
