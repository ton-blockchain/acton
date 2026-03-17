use crate::backend::Backend;
use crate::completion::collector::CompletionCollector;
use crate::completion::context::CompletionRequestContext;
use crate::completion::provider::CompletionProvider;
use lsp_types::{CompletionParams, CompletionResponse};

mod context;
pub(super) mod providers;
mod reference_completion_processor;

use context::TlbCompletionContext;
use providers::builtin_types::BuiltinTypesCompletionProvider;
use providers::reference::ReferenceCompletionProvider;

impl Backend {
    pub async fn handle_tlb_completion(
        &self,
        params: CompletionParams,
    ) -> Option<CompletionResponse> {
        crate::profile!(self, "tlb: completion");

        let request = CompletionRequestContext::from_params(&params);
        let file = self.registry.find_tlb_file(request.uri)?;
        let ctx = TlbCompletionContext::new(file, request.position)?;

        let reference_provider = ReferenceCompletionProvider;
        let builtin_provider = BuiltinTypesCompletionProvider;
        let providers: [&dyn CompletionProvider<TlbCompletionContext>; 2] =
            [&reference_provider, &builtin_provider];

        let mut collector = CompletionCollector::new();
        for provider in providers {
            if !provider.is_applicable(&ctx) {
                continue;
            }
            provider.collect(&ctx, &mut collector);
        }

        collector.into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::engine::cache::ParsedSnapshot;
    use lsp_types::Url;

    fn context(source: &str, offset: usize) -> TlbCompletionContext {
        let file = ParsedSnapshot::new(
            Url::parse("file:///tmp/test.tlb").expect("uri"),
            1,
            source,
            std::sync::Arc::new(tlb_syntax::parse(source).expect("parse")),
        );

        let position = file.position(offset);
        TlbCompletionContext::new(file, position).expect("context")
    }

    #[test]
    fn type_context_includes_declaration_and_builtins() {
        let source = "foo$0 field:Bar = Baz ;\n";
        let offset = source.find("Bar").expect("offset");
        let ctx = context(source, offset);

        assert!(ctx.is_type);

        let mut collector = CompletionCollector::new();
        ReferenceCompletionProvider.collect(&ctx, &mut collector);
        BuiltinTypesCompletionProvider.collect(&ctx, &mut collector);
        let labels = collector
            .into_sorted_items()
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        assert!(labels.iter().any(|label| label == "Baz"));
        assert!(labels.iter().any(|label| label == "UInt"));
    }

    #[test]
    fn value_context_contains_only_fields() {
        let source = "foo$0 a:Type b:Type = Bar ;\n";
        let offset = source.find("foo").expect("offset");
        let ctx = context(source, offset);

        assert!(!ctx.is_type);

        let mut collector = CompletionCollector::new();
        ReferenceCompletionProvider.collect(&ctx, &mut collector);
        let labels = collector
            .into_sorted_items()
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        assert!(labels.iter().any(|label| label == "a"));
        assert!(labels.iter().any(|label| label == "b"));
        assert!(!labels.iter().any(|label| label == "Bar"));
    }
}
