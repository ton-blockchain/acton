use crate::completion::collector::CompletionCollector;

pub(crate) trait CompletionProvider<Ctx> {
    fn id(&self) -> &'static str;

    fn is_applicable(&self, ctx: &Ctx) -> bool;

    fn collect(&self, ctx: &Ctx, out: &mut CompletionCollector);
}
