use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

/// Completes top-level declaration templates and the test get-method template.
///
/// The test-only template is restricted to `.test.tolk` files; regular declarations
/// remain available in every top-level context.
pub(crate) struct TopLevelCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for TopLevelCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::TopLevel
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        for &(label, detail, snippet) in TOP_LEVEL_SNIPPETS {
            collector.add(
                CompletionItem::new(label, CompletionItemKind::Keyword)
                    .with_label_detail(detail)
                    .with_snippet_replacement(context.syntax.replacement_range, snippet),
                CompletionRank::new(CompletionCategory::Keyword)
                    .with_prefix(&context.syntax.prefix, label),
            );
        }
        if context.document.uri().as_str().ends_with(".test.tolk") {
            collector.add(
                CompletionItem::new("get fun test", CompletionItemKind::Keyword)
                    .with_label_detail("() {}")
                    .with_snippet_replacement(
                        context.syntax.replacement_range,
                        "get fun `test $1`() {$0}",
                    ),
                CompletionRank::new(CompletionCategory::Keyword)
                    .with_prefix(&context.syntax.prefix, "get fun test"),
            );
        }
        Some(())
    }
}

const TOP_LEVEL_SNIPPETS: &[(&str, &str, &str)] = &[
    ("import", " \"\"", "import \"$1\"$0"),
    (
        "contract",
        " Name {}",
        "contract ${1:Name} {\n    author: \"${2:}\"\n    version: \"${3:1.0.0}\"\n    description: \"${4:My TON contract}\"\n    incomingMessages: ${5:AllowedMessages}\n    storage: ${6:Storage}\n}$0",
    ),
    ("struct", " Name {}", "struct ${1:Name} {\n    $0\n}"),
    ("enum", " Name {}", "enum ${1:Name} {\n    $0\n}"),
    ("type", " Int = int", "type ${1:Int} = ${2:int}$0"),
    (
        "const",
        " FOO: <type> = <value>",
        "const ${1:FOO}: ${2:int} = ${3:0}$0",
    ),
    (
        "global",
        " foo: <type> = <value>",
        "global ${1:foo}: ${2:int}$0",
    ),
    ("fun", " name() {}", "fun ${1:name}($2)$3 {\n    $0\n}"),
    (
        "inline fun",
        " name() {}",
        "@inline\nfun ${1:name}($2)$3 {\n    $0\n}",
    ),
    (
        "inline_ref fun",
        " name() {}",
        "@inline_ref\nfun ${1:name}($2)$3 {\n    $0\n}",
    ),
    (
        "asm fun",
        " name() asm \"...\"",
        "fun ${1:name}($2)$3 asm \"$0\"",
    ),
    (
        "method fun",
        " Foo.name(self) {}",
        "fun ${1:Foo}.${2:name}(${3:self}$4)$5 {\n    $0\n}",
    ),
    (
        "static method fun",
        " Foo.name() {}",
        "fun ${1:Foo}.${2:name}($3)$4 {\n    $0\n}",
    ),
    (
        "get fun",
        " name() {}",
        "get fun ${1:name}($2)$3 {\n    $0\n}",
    ),
];
