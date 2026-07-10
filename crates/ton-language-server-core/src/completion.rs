use crate::{DocumentSnapshot, Position, Range, TextEdit};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionTriggerKind {
    Invoked,
    TriggerCharacter,
    TriggerForIncompleteCompletions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionTrigger {
    pub kind: CompletionTriggerKind,
    pub character: Option<String>,
}

impl CompletionTrigger {
    #[must_use]
    pub const fn invoked() -> Self {
        Self {
            kind: CompletionTriggerKind::Invoked,
            character: None,
        }
    }

    #[must_use]
    pub fn character(character: impl Into<String>) -> Self {
        Self {
            kind: CompletionTriggerKind::TriggerCharacter,
            character: Some(character.into()),
        }
    }
}

impl Default for CompletionTrigger {
    fn default() -> Self {
        Self::invoked()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompletionItemKind {
    Text,
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Unit,
    Value,
    Enum,
    Keyword,
    Snippet,
    Color,
    File,
    Reference,
    Folder,
    EnumMember,
    Constant,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InsertTextFormat {
    #[default]
    PlainText,
    Snippet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<CompletionItemKind>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
    pub insert_text: Option<String>,
    pub insert_text_format: InsertTextFormat,
    pub text_edit: Option<TextEdit>,
    pub additional_text_edits: Vec<TextEdit>,
    pub deprecated: bool,
}

impl CompletionItem {
    #[must_use]
    pub fn new(label: impl Into<String>, kind: CompletionItemKind) -> Self {
        Self {
            label: label.into(),
            kind: Some(kind),
            detail: None,
            documentation: None,
            sort_text: None,
            filter_text: None,
            insert_text: None,
            insert_text_format: InsertTextFormat::PlainText,
            text_edit: None,
            additional_text_edits: Vec::new(),
            deprecated: false,
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[must_use]
    pub fn with_documentation(mut self, documentation: impl Into<String>) -> Self {
        self.documentation = Some(documentation.into());
        self
    }

    #[must_use]
    pub fn with_filter_text(mut self, filter_text: impl Into<String>) -> Self {
        self.filter_text = Some(filter_text.into());
        self
    }

    #[must_use]
    pub fn with_insert_text(mut self, insert_text: impl Into<String>) -> Self {
        self.insert_text = Some(insert_text.into());
        self
    }

    #[must_use]
    pub fn with_replacement(mut self, range: Range, new_text: impl Into<String>) -> Self {
        self.text_edit = Some(TextEdit::new(range, new_text));
        self
    }

    #[must_use]
    pub fn with_snippet_replacement(mut self, range: Range, snippet: impl Into<String>) -> Self {
        let snippet = snippet.into();
        self.text_edit = Some(TextEdit::new(range, snippet.clone()));
        self.insert_text = Some(snippet);
        self.insert_text_format = InsertTextFormat::Snippet;
        self
    }

    #[must_use]
    pub fn with_additional_text_edit(mut self, edit: TextEdit) -> Self {
        self.additional_text_edits.push(edit);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

impl CompletionList {
    #[must_use]
    pub const fn new(items: Vec<CompletionItem>) -> Self {
        Self {
            is_incomplete: false,
            items,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

pub(crate) trait CompletionProvider<Context> {
    fn is_applicable(&self, _context: &Context) -> bool {
        true
    }

    fn collect(&self, context: &Context, collector: &mut CompletionCollector) -> Option<()>;
}

pub(crate) fn collect_from_providers<Context>(
    context: &Context,
    providers: &[&dyn CompletionProvider<Context>],
) -> CompletionList {
    let mut collector = CompletionCollector::new();
    for provider in providers {
        if provider.is_applicable(context) {
            provider.collect(context, &mut collector);
        }
    }
    collector.finish()
}

impl CompletionCategory {
    const fn weight(self) -> u16 {
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
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompletionRank {
    category: CompletionCategory,
    context_match: bool,
    prefix_score: u16,
    locality_penalty: u16,
}

impl CompletionRank {
    #[must_use]
    pub(crate) const fn new(category: CompletionCategory) -> Self {
        Self {
            category,
            context_match: true,
            prefix_score: 0,
            locality_penalty: 0,
        }
    }

    #[must_use]
    pub(crate) fn with_prefix(mut self, prefix: &str, label: &str) -> Self {
        if prefix.is_empty() {
            return self;
        }
        let prefix = prefix.to_ascii_lowercase();
        let label = label.to_ascii_lowercase();
        if label.starts_with(&prefix) {
            self.prefix_score =
                u16::try_from(label.len().saturating_sub(prefix.len())).unwrap_or(u16::MAX);
        } else if label.contains(&prefix) {
            self.prefix_score = 500;
        } else {
            self.context_match = false;
            self.prefix_score = 1000;
        }
        self
    }

    #[must_use]
    pub(crate) const fn with_locality_penalty(mut self, locality_penalty: u16) -> Self {
        self.locality_penalty = locality_penalty;
        self
    }

    const fn key(self) -> (u16, u8, u16, u16) {
        (
            self.category.weight(),
            if self.context_match { 0 } else { 1 },
            self.prefix_score,
            self.locality_penalty,
        )
    }

    fn sort_text(self, label: &str) -> String {
        let (category, context_penalty, prefix_score, locality_penalty) = self.key();
        format!("{category:03}:{context_penalty}:{prefix_score:04}:{locality_penalty:04}:{label}")
    }
}

struct RankedCompletionItem {
    item: CompletionItem,
    rank: CompletionRank,
}

#[derive(Default)]
pub(crate) struct CompletionCollector {
    items: Vec<RankedCompletionItem>,
}

impl CompletionCollector {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub(crate) fn add(&mut self, item: CompletionItem, rank: CompletionRank) {
        if item.label.is_empty() {
            return;
        }
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|existing| same_candidate(&existing.item, &item))
        {
            if rank.key() < existing.rank.key() {
                *existing = RankedCompletionItem { item, rank };
            }
            return;
        }
        self.items.push(RankedCompletionItem { item, rank });
    }

    #[must_use]
    pub(crate) fn finish(mut self) -> CompletionList {
        self.items.sort_by(compare_ranked);
        let items = self
            .items
            .into_iter()
            .map(|mut ranked| {
                if ranked.item.sort_text.is_none() {
                    ranked.item.sort_text = Some(ranked.rank.sort_text(&ranked.item.label));
                }
                ranked.item
            })
            .collect();
        CompletionList::new(items)
    }
}

fn same_candidate(left: &CompletionItem, right: &CompletionItem) -> bool {
    left.label == right.label
        && left.kind == right.kind
        && left.insert_text == right.insert_text
        && left.text_edit == right.text_edit
}

fn compare_ranked(left: &RankedCompletionItem, right: &RankedCompletionItem) -> Ordering {
    left.rank
        .key()
        .cmp(&right.rank.key())
        .then_with(|| left.item.label.cmp(&right.item.label))
        .then_with(|| left.item.insert_text.cmp(&right.item.insert_text))
}

#[must_use]
pub(crate) fn identifier_prefix(document: &DocumentSnapshot, position: Position) -> (&str, Range) {
    let source = document.text();
    let end = document
        .text_index()
        .position_to_offset(source, position)
        .min(source.len());
    let mut start = end;
    while start > 0 {
        let byte = source.as_bytes()[start - 1];
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' {
            start -= 1;
        } else {
            break;
        }
    }
    let mut replacement_end = end;
    while replacement_end < source.len() {
        let byte = source.as_bytes()[replacement_end];
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' {
            replacement_end += 1;
        } else {
            break;
        }
    }
    let range = document
        .text_index()
        .range_for_offsets(source, start, replacement_end);
    (&source[start..end], range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DocumentUri, LanguageId};

    #[test]
    fn collector_keeps_the_better_duplicate_and_sorts_deterministically() {
        let mut collector = CompletionCollector::new();
        let item = CompletionItem::new("foo", CompletionItemKind::Function);
        collector.add(
            item.clone(),
            CompletionRank::new(CompletionCategory::TypeAlias),
        );
        collector.add(
            item,
            CompletionRank::new(CompletionCategory::Function).with_prefix("fo", "foo"),
        );
        let result = collector.finish();
        assert_eq!(result.items.len(), 1);
        assert_eq!(
            result.items[0].sort_text.as_deref(),
            Some("090:0:0001:0000:foo")
        );
    }

    #[test]
    fn identifier_prefix_uses_utf16_positions() {
        let document = DocumentSnapshot::new(
            DocumentUri::from("memory:///main.tolk"),
            LanguageId::from("tolk"),
            1,
            "val emoji = \"😀\";\nstor",
        );
        let (prefix, range) = identifier_prefix(&document, Position::new(1, 4));
        assert_eq!(prefix, "stor");
        assert_eq!(range, Range::new(Position::new(1, 0), Position::new(1, 4)));
    }

    #[test]
    fn identifier_replacement_covers_the_whole_token() {
        let document = DocumentSnapshot::new(
            DocumentUri::from("memory:///main.tolk"),
            LanguageId::from("tolk"),
            1,
            "storage",
        );
        let (prefix, range) = identifier_prefix(&document, Position::new(0, 4));
        assert_eq!(prefix, "stor");
        assert_eq!(range, Range::new(Position::new(0, 0), Position::new(0, 7)));
    }

    struct TestProvider {
        applicable: bool,
        label: &'static str,
    }

    impl CompletionProvider<()> for TestProvider {
        fn is_applicable(&self, _context: &()) -> bool {
            self.applicable
        }

        fn collect(&self, _context: &(), collector: &mut CompletionCollector) -> Option<()> {
            collector.add(
                CompletionItem::new(self.label, CompletionItemKind::Keyword),
                CompletionRank::new(CompletionCategory::Keyword),
            );
            Some(())
        }
    }

    #[test]
    fn provider_pipeline_composes_applicable_providers_in_order() {
        let skipped = TestProvider {
            applicable: false,
            label: "skipped",
        };
        let first = TestProvider {
            applicable: true,
            label: "first",
        };
        let second = TestProvider {
            applicable: true,
            label: "second",
        };
        let providers: [&dyn CompletionProvider<()>; 3] = [&skipped, &first, &second];

        let completion = collect_from_providers(&(), &providers);

        assert_eq!(
            completion
                .items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
    }
}
