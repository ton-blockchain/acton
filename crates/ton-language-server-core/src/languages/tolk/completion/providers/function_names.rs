use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

pub(crate) struct FunctionNameCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for FunctionNameCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::FunctionName
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let Some(declaration) = context.syntax.cursor_node().and_then(|node| node.parent()) else {
            return;
        };
        let has_body_and_params = declaration.child_by_field_name("parameters").is_some()
            && declaration.child_by_field_name("body").is_some();
        if declaration.kind() == "method_declaration" {
            let receiver = declaration
                .child_by_field_name("receiver")
                .and_then(|receiver| receiver.child_by_field_name("receiver_type"))
                .and_then(|receiver| receiver.utf8_text(context.syntax.source().as_bytes()).ok())
                .unwrap_or_default();
            let return_type = (!receiver.is_empty()).then(|| format!(": {receiver}"));
            let unpack = if has_body_and_params {
                "unpackFromSlice".to_owned()
            } else {
                format!(
                    "unpackFromSlice(mutate s: slice){} {{$0}}",
                    return_type.as_deref().unwrap_or_default()
                )
            };
            add_function_name(context, collector, "unpackFromSlice", unpack);
            let pack = if has_body_and_params {
                "packToBuilder".to_owned()
            } else {
                "packToBuilder(self, mutate b: builder) {$0}".to_owned()
            };
            add_function_name(context, collector, "packToBuilder", pack);
            return;
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
