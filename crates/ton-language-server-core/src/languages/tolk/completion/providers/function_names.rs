use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

/// Completes well-known Tolk entry-point and serialization method names.
///
/// The inserted form depends on the typed declaration shape, including receiver,
/// parameters, return type, and whether a body is already present.
pub(crate) struct FunctionNameCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for FunctionNameCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::FunctionName
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let declaration = context.syntax.ancestor_base_function()?;
        let has_body_and_params = declaration.has_parameters() && declaration.body().is_some();

        if declaration.is_method() {
            let return_type = declaration
                .receiver_type()
                .map(|receiver| format!(": {}", context.syntax.text_of(receiver)))
                .unwrap_or_default();

            let unpack = if has_body_and_params {
                "unpackFromSlice".to_owned()
            } else {
                format!("unpackFromSlice(mutate s: slice){return_type} {{$0}}")
            };

            add_function_name(context, collector, "unpackFromSlice", unpack);

            let pack = if has_body_and_params {
                "packToBuilder".to_owned()
            } else {
                "packToBuilder(self, mutate b: builder) {$0}".to_owned()
            };
            add_function_name(context, collector, "packToBuilder", pack);
            return Some(());
        }

        let defined = context
            .snapshot
            .project_index
            .files()
            .get(&context.file_id)
            .map(|file| {
                file.decls
                    .iter()
                    .filter(|symbol| symbol.is_func())
                    .map(|symbol| symbol.name.as_ref())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        for &(label, signature) in FUNCTION_NAMES {
            if defined.contains(label) {
                continue;
            }
            let insertion = if has_body_and_params {
                label.to_owned()
            } else {
                format!("{label}({signature}) {{$0}}")
            };
            add_function_name(context, collector, label, insertion);
        }
        Some(())
    }
}

fn add_function_name(
    context: &TolkCompletionProviderContext<'_>,
    collector: &mut CompletionCollector,
    label: &str,
    insertion: String,
) {
    collector.add(
        CompletionItem::new(label, CompletionItemKind::Function)
            .with_snippet_replacement(context.syntax.replacement_range, insertion),
        CompletionRank::new(CompletionCategory::Function)
            .with_prefix(&context.syntax.prefix, label),
    );
}

const FUNCTION_NAMES: &[(&str, &str)] = &[
    ("onInternalMessage", "in: InMessage"),
    ("onExternalMessage", "inMsg: slice"),
    ("onBouncedMessage", "in: InMessageBounced"),
    ("onRunTickTock", "isTock: bool"),
    ("onSplitPrepare", ""),
    ("onSplitInstall", ""),
    ("main", ""),
];
