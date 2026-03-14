#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionCategory {
    ContextElement,
    Variable,
    Parameter,
    Field,
    Keyword,
    Function,
    Snippet,
    Constant,
    Global,
    Struct,
    Enum,
    TypeAlias,
    Other,
}

impl CompletionCategory {
    #[must_use]
    pub(crate) const fn weight(self) -> u16 {
        match self {
            Self::ContextElement => 0,
            Self::Variable => 50,
            Self::Parameter => 60,
            Self::Field => 70,
            Self::Keyword => 80,
            Self::Function => 90,
            Self::Snippet => 95,
            Self::Constant => 100,
            Self::Global => 105,
            Self::Struct => 110,
            Self::Enum => 115,
            Self::TypeAlias => 120,
            Self::Other => 500,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompletionRank {
    pub(crate) category: CompletionCategory,
    pub(crate) context_match: bool,
    pub(crate) prefix_score: u16,
    pub(crate) locality_boost: u8,
}

impl Default for CompletionRank {
    fn default() -> Self {
        Self {
            category: CompletionCategory::Other,
            context_match: true,
            prefix_score: 0,
            locality_boost: 0,
        }
    }
}

#[must_use]
pub(crate) const fn rank_key(rank: CompletionRank) -> (u16, u8, u16, u8) {
    let context_penalty = if rank.context_match { 0 } else { 1 };
    (
        rank.category.weight(),
        context_penalty,
        rank.prefix_score,
        rank.locality_boost,
    )
}

#[must_use]
pub(crate) fn encode_sort_text(rank: CompletionRank, label: &str) -> String {
    let (category, context_penalty, prefix_score, locality_boost) = rank_key(rank);
    format!("{category:03}:{context_penalty}:{prefix_score:04}:{locality_boost:03}:{label}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_deterministic_sort_text() {
        let sort_text = encode_sort_text(
            CompletionRank {
                category: CompletionCategory::Function,
                context_match: false,
                prefix_score: 7,
                locality_boost: 2,
            },
            "foo",
        );

        assert_eq!(sort_text, "090:1:0007:002:foo");
    }
}
