use crate::completion::candidate::CompletionCandidate;
use crate::completion::ranking::CompletionCategory;
use crate::languages::tlb::completion::context::TlbCompletionContext;
use crate::languages::tlb::psi::{ScopeProcessor, TlbNamedItem, TlbNamedItemKind};
use lsp_types::{CompletionItemKind, InsertTextFormat};
use std::collections::HashSet;

const DUMMY_IDENTIFIER: &str = "DummyIdentifier";

pub(super) struct ReferenceCompletionProcessor<'ctx> {
    ctx: &'ctx TlbCompletionContext,
    seen: HashSet<(TlbNamedItemKind, String)>,
    candidates: Vec<CompletionCandidate>,
}

impl<'ctx> ReferenceCompletionProcessor<'ctx> {
    pub(super) fn new(ctx: &'ctx TlbCompletionContext) -> Self {
        Self {
            ctx,
            seen: HashSet::new(),
            candidates: Vec::new(),
        }
    }

    pub(super) fn into_candidates(self) -> Vec<CompletionCandidate> {
        self.candidates
    }

    fn allowed_in_context(&self, item: TlbNamedItem<'_>) -> bool {
        if self.ctx.is_type {
            return true;
        }

        item.kind == TlbNamedItemKind::NamedField
    }
}

impl<'ctx> ScopeProcessor<'ctx> for ReferenceCompletionProcessor<'ctx> {
    fn execute(&mut self, item: TlbNamedItem<'ctx>) -> bool {
        let source = self.ctx.file.source();
        let Some(name) = item.name(source) else {
            return true;
        };

        if name.is_empty() || name.ends_with(DUMMY_IDENTIFIER) {
            return true;
        }

        if !self.allowed_in_context(item) {
            return true;
        }

        let key = (item.kind, name.to_string());
        if self.seen.contains(&key) {
            return true;
        }
        self.seen.insert(key);

        let (kind, category) = match item.kind {
            TlbNamedItemKind::Declaration => {
                (CompletionItemKind::CLASS, CompletionCategory::Struct)
            }
            TlbNamedItemKind::NamedField => (CompletionItemKind::FIELD, CompletionCategory::Field),
            TlbNamedItemKind::Parameter => (
                CompletionItemKind::TYPE_PARAMETER,
                CompletionCategory::Parameter,
            ),
        };

        let mut candidate = CompletionCandidate::new(name);
        candidate.kind = Some(kind);
        candidate.insert_text = Some(format!("{name}$0"));
        candidate.insert_text_format = Some(InsertTextFormat::SNIPPET);
        candidate.rank = self.ctx.rank_for(category, name);

        if item.kind == TlbNamedItemKind::Parameter
            && let Some(owner) = item.owner_name(source)
            && !owner.is_empty()
        {
            candidate.detail = Some(format!("of {owner}"));
        }

        self.candidates.push(candidate);
        true
    }
}
