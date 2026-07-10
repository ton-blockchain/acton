use super::super::context::FiftCompletionContext;
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};
use crate::{CompletionItem, CompletionItemKind};
use fift_syntax::{AstNode, HasName, TopLevel};

pub(crate) struct DeclarationCompletionProvider;

impl CompletionProvider<FiftCompletionContext<'_>> for DeclarationCompletionProvider {
    fn collect(&self, context: &FiftCompletionContext<'_>, collector: &mut CompletionCollector) {
        for top_level in context.parsed.source_file.top_levels() {
            let (name, kind) = match top_level {
                TopLevel::Declaration(declaration) => (
                    declaration.name(),
                    declaration
                        .kind()
                        .map_or(CompletionItemKind::Reference, |kind| match kind {
                            fift_syntax::DeclarationKind::GlobalVar(_) => {
                                CompletionItemKind::Variable
                            }
                            fift_syntax::DeclarationKind::ProcDeclaration(_)
                            | fift_syntax::DeclarationKind::MethodDeclaration(_) => {
                                CompletionItemKind::Function
                            }
                            fift_syntax::DeclarationKind::Unmapped(_) => {
                                CompletionItemKind::Reference
                            }
                        }),
                ),
                TopLevel::Definition(definition) => {
                    (definition.name(), CompletionItemKind::Function)
                }
                TopLevel::Unmapped(_) => continue,
            };
            let Some(name) = name else {
                continue;
            };
            let name = name.text(context.document.text()).trim();
            if name.is_empty() {
                continue;
            }
            let category = if kind == CompletionItemKind::Variable {
                CompletionCategory::Global
            } else {
                CompletionCategory::Function
            };
            collector.add(
                CompletionItem::new(name, kind).with_replacement(context.replacement_range, name),
                context.rank_for(category, name),
            );
        }
    }
}
