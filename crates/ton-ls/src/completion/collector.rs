use crate::completion::candidate::CompletionCandidate;
use crate::completion::ranking::rank_key;
use lsp_types::{CompletionItem, CompletionResponse};

#[derive(Debug, Default)]
pub(crate) struct CompletionCollector {
    candidates: Vec<CompletionCandidate>,
}

impl CompletionCollector {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add(&mut self, candidate: CompletionCandidate) {
        if let Some(existing_index) = self
            .candidates
            .iter()
            .position(|existing| is_duplicate(existing, &candidate))
        {
            if is_better_candidate(&candidate, &self.candidates[existing_index]) {
                self.candidates[existing_index] = candidate;
            }
            return;
        }

        self.candidates.push(candidate);
    }

    pub(crate) fn extend(&mut self, candidates: impl IntoIterator<Item = CompletionCandidate>) {
        for candidate in candidates {
            self.add(candidate);
        }
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    #[must_use]
    pub(crate) fn into_sorted_items(mut self) -> Vec<CompletionItem> {
        self.candidates.sort_by(compare_candidates);
        self.candidates
            .into_iter()
            .map(CompletionCandidate::convert_to_completion_item)
            .collect()
    }

    #[must_use]
    pub(crate) fn into_response(self) -> Option<CompletionResponse> {
        let items = self.into_sorted_items();
        if items.is_empty() {
            return None;
        }
        Some(CompletionResponse::Array(items))
    }
}

fn is_duplicate(left: &CompletionCandidate, right: &CompletionCandidate) -> bool {
    left.label == right.label && left.kind == right.kind && left.insert_text == right.insert_text
}

fn is_better_candidate(left: &CompletionCandidate, right: &CompletionCandidate) -> bool {
    compare_candidates(left, right).is_lt()
}

fn compare_candidates(
    left: &CompletionCandidate,
    right: &CompletionCandidate,
) -> std::cmp::Ordering {
    rank_key(left.rank)
        .cmp(&rank_key(right.rank))
        .then_with(|| left.label.cmp(&right.label))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::ranking::{CompletionCategory, CompletionRank};
    use lsp_types::CompletionItemKind;

    #[test]
    fn dedups_and_keeps_higher_ranked_candidate() {
        let mut collector = CompletionCollector::new();

        let mut low = CompletionCandidate::new("foo");
        low.kind = Some(CompletionItemKind::FUNCTION);
        low.rank = CompletionRank {
            category: CompletionCategory::Other,
            context_match: false,
            prefix_score: 10,
            locality_boost: 10,
        };

        let mut high = CompletionCandidate::new("foo");
        high.kind = Some(CompletionItemKind::FUNCTION);
        high.rank = CompletionRank {
            category: CompletionCategory::Function,
            context_match: true,
            prefix_score: 1,
            locality_boost: 0,
        };

        collector.add(low);
        collector.add(high);

        let items = collector.into_sorted_items();
        assert_eq!(items.len(), 1);
        assert!(
            items[0]
                .sort_text
                .as_deref()
                .is_some_and(|sort_text| sort_text.starts_with("090:0:0001:000"))
        );
    }
}
