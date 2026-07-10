use super::{TolkCompletionContext, TolkCompletionProviderContext};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::languages::tolk::TolkResolveSnapshot;
use crate::languages::tolk::completion::items;
use crate::{CompletionItem, CompletionItemKind};
use std::collections::BTreeSet;
use tolk_resolver::{Symbol, SymbolKind};
use tolk_syntax::{Match, MatchPattern};
use tolk_ty::{TyData, TyId};

/// Completes missing match arms for unions, enums, structs, and value matches.
/// It also offers a single `else` arm and a fill-all snippet where that is meaningful.
pub(crate) struct MatchArmCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for MatchArmCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        context.syntax.expect_match_arm()
    }

    fn collect(
        &self,
        provider_context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let snapshot = provider_context.snapshot;
        let context = provider_context.syntax;
        let match_expression = context.ancestor_as::<Match>()?;
        let expression = match_expression.expr()?;
        let ty = provider_context.type_of_node(expression)?;

        let base_ty = snapshot.type_interner.unwrap_alias(ty);
        let mut variants = BTreeSet::new();
        collect_type_variants(snapshot, ty, &mut variants);
        let existing = existing_arms(match_expression, context);

        if variants.is_empty() {
            collect_non_type_match_arms(provider_context, collector);
        } else {
            let is_enum = matches!(snapshot.type_interner.data(base_ty), TyData::Enum { .. });
            if !is_enum && existing.is_empty() {
                add_fill_all_completion(context, collector, &variants);
            }

            add_variant_completions(context, collector, variants, &existing);
        }

        if !existing.contains("else") {
            add_else_completion(context, collector);
        }

        Some(())
    }
}

fn add_fill_all_completion(
    context: &TolkCompletionContext,
    collector: &mut CompletionCollector,
    variants: &BTreeSet<String>,
) {
    let mut lines = variants
        .iter()
        .enumerate()
        .map(|(index, variant)| format!("{variant} => {{{}}}", if index == 0 { "$0" } else { "" }))
        .collect::<Vec<_>>();
    lines.push("else => {}".to_owned());

    collector.add(
        CompletionItem::new("Fill all cases...", CompletionItemKind::Snippet)
            .with_snippet_replacement(context.replacement_range, lines.join("\n")),
        CompletionRank::new(CompletionCategory::ContextElement),
    );
}

fn add_variant_completions(
    context: &TolkCompletionContext,
    collector: &mut CompletionCollector,
    variants: BTreeSet<String>,
    existing: &BTreeSet<String>,
) {
    for variant in variants {
        if existing.contains(&variant) {
            continue;
        }

        let snippet = format!("{variant} => {{\n\t$0\n}}");
        collector.add(
            CompletionItem::new(&variant, CompletionItemKind::EnumMember)
                .with_snippet_replacement(context.replacement_range, snippet),
            CompletionRank::new(CompletionCategory::ContextElement)
                .with_prefix(&context.prefix, &variant),
        );
    }
}

fn add_else_completion(context: &TolkCompletionContext, collector: &mut CompletionCollector) {
    let snippet = "else => {\n\t$0\n}";
    collector.add(
        CompletionItem::new("else", CompletionItemKind::Keyword)
            .with_snippet_replacement(context.replacement_range, snippet),
        CompletionRank::new(CompletionCategory::ContextElement)
            .with_prefix(&context.prefix, "else"),
    );
}

fn collect_non_type_match_arms(
    context: &TolkCompletionProviderContext<'_>,
    collector: &mut CompletionCollector,
) {
    let mut candidates = CompletionCollector::new();
    for local in context.visible_locals() {
        if let Some(candidate) = items::local(context, local) {
            candidates.add(candidate.item, candidate.rank);
        }
    }

    for symbol in context.visible_globals() {
        if is_expression_symbol(symbol)
            && let Some(candidate) = items::symbol(context, symbol, false)
        {
            candidates.add(candidate.item, candidate.rank);
        }

        if matches!(symbol.kind, SymbolKind::Enum { .. }) {
            let SymbolKind::Enum { members } = &symbol.kind else {
                return;
            };
            for member in members {
                let candidate = items::prefixed_enum_member(context, symbol, member);
                candidates.add(candidate.item, candidate.rank);
            }
        }
    }
    for candidate in candidates.finish().items {
        let insertion = candidate
            .text_edit
            .as_ref()
            .map(|edit| edit.new_text.as_str())
            .or(candidate.insert_text.as_deref())
            .unwrap_or(&candidate.label);
        let snippet = format!("{insertion}$1 => {{$0}}");
        collector.add(
            CompletionItem::new(
                &candidate.label,
                candidate.kind.unwrap_or(CompletionItemKind::Value),
            )
            .with_snippet_replacement(context.syntax.replacement_range, snippet),
            CompletionRank::new(CompletionCategory::ContextElement)
                .with_prefix(&context.syntax.prefix, &candidate.label),
        );
    }
}

fn is_expression_symbol(symbol: &Symbol) -> bool {
    !(symbol.name.starts_with("__")
        || matches!(symbol.kind, SymbolKind::GetMethod { .. })
            && tolk_syntax::is_test_get_method_name(symbol.name.as_ref()))
        && matches!(
            symbol.kind,
            SymbolKind::Function { .. }
                | SymbolKind::GetMethod { .. }
                | SymbolKind::Constant
                | SymbolKind::GlobalVariable
                | SymbolKind::Struct { .. }
                | SymbolKind::Enum { .. }
        )
}

fn collect_type_variants(
    snapshot: &TolkResolveSnapshot,
    ty: TyId,
    variants: &mut BTreeSet<String>,
) {
    let ty = snapshot.type_interner.unwrap_alias(ty);
    match snapshot.type_interner.data(ty) {
        TyData::Union(types) => {
            for &ty in types {
                collect_union_variant(snapshot, ty, variants);
            }
        }
        TyData::Enum { def, name } => {
            if let Some(symbol) = snapshot.project_index.resolve_symbol(*def)
                && let SymbolKind::Enum { members } = &symbol.kind
            {
                for member in members {
                    variants.insert(format!("{name}.{}", member.name));
                }
            }
        }
        TyData::Struct { name, .. } => {
            variants.insert(name.to_string());
        }
        _ => {}
    }
}

fn collect_union_variant(
    snapshot: &TolkResolveSnapshot,
    ty: TyId,
    variants: &mut BTreeSet<String>,
) {
    let ty = snapshot.type_interner.unwrap_alias(ty);
    if let TyData::Union(types) = snapshot.type_interner.data(ty) {
        for &ty in types {
            collect_union_variant(snapshot, ty, variants);
        }
    } else {
        variants.insert(snapshot.type_interner.format(ty));
    }
}

fn existing_arms(match_expression: Match<'_>, context: &TolkCompletionContext) -> BTreeSet<String> {
    match_expression
        .arms()
        .map(|arm| match arm.pattern() {
            MatchPattern::Type(pattern) => context.text_of(pattern),
            MatchPattern::Expr(pattern) => context.text_of(pattern),
            MatchPattern::Else => "else",
        })
        .map(str::to_owned)
        .collect()
}
