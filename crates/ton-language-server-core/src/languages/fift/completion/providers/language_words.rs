use super::super::context::FiftCompletionContext;
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};
use crate::{CompletionItem, CompletionItemKind};

pub(crate) struct LanguageWordCompletionProvider;

impl CompletionProvider<FiftCompletionContext<'_>> for LanguageWordCompletionProvider {
    fn collect(&self, context: &FiftCompletionContext<'_>, collector: &mut CompletionCollector) {
        for &(label, snippet, detail) in FIFT_WORDS {
            let item = CompletionItem::new(label, CompletionItemKind::Snippet)
                .with_detail(detail)
                .with_snippet_replacement(context.replacement_range, snippet);
            collector.add(item, context.rank_for(CompletionCategory::Snippet, label));
        }
    }
}

const FIFT_WORDS: &[(&str, &str, &str)] = &[
    ("PROGRAM{", "PROGRAM{\n\t$0\nEND>c", "program"),
    ("DECLPROC", "DECLPROC ${1:name}$0", "procedure declaration"),
    (
        "DECLMETHOD",
        "DECLMETHOD ${1:id} ${2:name}$0",
        "method declaration",
    ),
    ("GLOBAL", "GLOBAL ${1:name}$0", "global variable"),
    ("PROC:<{", "${1:name} PROC:<{\n\t$0\n}>", "procedure"),
    (
        "PROCINLINE:<{",
        "${1:name} PROCINLINE:<{\n\t$0\n}>",
        "inline procedure",
    ),
    (
        "PROCREF:<{",
        "${1:name} PROCREF:<{\n\t$0\n}>",
        "procedure reference",
    ),
    (
        "METHOD:<{",
        "${1:id} ${2:name} METHOD:<{\n\t$0\n}>",
        "method",
    ),
    ("IF:<{", "IF:<{\n\t$0\n}>", "if block"),
    ("}>ELSE<{", "}>ELSE<{\n\t$0\n}>", "else block"),
    ("IFJMP:<{", "IFJMP:<{\n\t$0\n}>", "if-jump block"),
    ("WHILE:<{", "WHILE:<{\n\t$1\n}>DO<{\n\t$0\n}>", "while loop"),
    ("}>DO<{", "}>DO<{\n\t$0\n}>", "while body"),
    ("REPEAT:<{", "REPEAT:<{\n\t$0\n}>", "repeat loop"),
    ("UNTIL:<{", "UNTIL:<{\n\t$0\n}>", "until loop"),
    ("<{", "<{\n\t$0\n}>", "instruction block"),
];
