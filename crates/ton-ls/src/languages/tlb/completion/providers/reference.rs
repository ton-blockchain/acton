use crate::completion::collector::CompletionCollector;
use crate::completion::provider::CompletionProvider;
use crate::languages::tlb::completion::context::TlbCompletionContext;
use crate::languages::tlb::completion::reference_completion_processor::ReferenceCompletionProcessor;
use crate::languages::tlb::psi::TlbReference;

#[derive(Default)]
pub(crate) struct ReferenceCompletionProvider;

impl CompletionProvider<TlbCompletionContext> for ReferenceCompletionProvider {
    fn id(&self) -> &'static str {
        "tlb.reference"
    }

    fn is_applicable(&self, _ctx: &TlbCompletionContext) -> bool {
        true
    }

    fn collect(&self, ctx: &TlbCompletionContext, out: &mut CompletionCollector) {
        let Some(node) = ctx.cursor_node() else {
            return;
        };

        let Some(reference) = TlbReference::new(node, ctx.file.syntax()) else {
            return;
        };

        let mut processor = ReferenceCompletionProcessor::new(ctx);
        reference.process_resolve_variants(&mut processor);
        out.extend(processor.into_candidates());
    }
}
