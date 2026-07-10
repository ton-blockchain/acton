use super::super::context::{DUMMY_IDENTIFIER, TlbCompletionContext};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};
use crate::languages::tlb::reference::{TlbNamedItemKind, resolve_variants_at};
use crate::{CompletionItem, CompletionItemKind};

/// Completes visible TL-B declarations, fields, and type parameters.
///
/// In a type position it hides value-only fields; in a value position it keeps
/// only names that can be used as field references.
pub(crate) struct ReferenceCompletionProvider;

impl CompletionProvider<TlbCompletionContext> for ReferenceCompletionProvider {
    fn collect(
        &self,
        context: &TlbCompletionContext,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let node = context.cursor_node()?;
        for item in resolve_variants_at(context.source_file(), node) {
            if !context.is_type && item.kind != TlbNamedItemKind::NamedField {
                continue;
            }
            let Some(name) = item.name(context.source()) else {
                continue;
            };
            if name.is_empty() || name.ends_with(DUMMY_IDENTIFIER) {
                continue;
            }
            let (kind, category) = match item.kind {
                TlbNamedItemKind::Declaration => {
                    (CompletionItemKind::Class, CompletionCategory::Struct)
                }
                TlbNamedItemKind::NamedField => {
                    (CompletionItemKind::Field, CompletionCategory::Field)
                }
                TlbNamedItemKind::Parameter => (
                    CompletionItemKind::TypeParameter,
                    CompletionCategory::Parameter,
                ),
            };
            let insertion = format!("{name}$0");
            let mut completion = CompletionItem::new(name, kind)
                .with_snippet_replacement(context.replacement_range, insertion);
            if item.kind == TlbNamedItemKind::Parameter
                && let Some(owner) = item.owner_name(context.source())
                && !owner.is_empty()
            {
                completion.detail = Some(format!("of {owner}"));
            }
            collector.add(completion, context.rank_for(category, name));
        }
        Some(())
    }
}
