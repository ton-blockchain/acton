use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

pub(crate) struct TopLevelCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for TopLevelCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::TopLevel
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        for &(label, snippet) in TOP_LEVEL_SNIPPETS {
            add_snippet(
                context.syntax,
                collector,
                label,
                snippet,
                CompletionCategory::Keyword,
            );
        }
        if context.document.uri().as_str().ends_with(".test.tolk") {
            add_snippet(
                context.syntax,
                collector,
                "get fun test",
                "get fun `test $1`() {$0}",
                CompletionCategory::Keyword,
            );
        }
    }
}

const TOP_LEVEL_SNIPPETS: &[(&str, &str)] = &[
    ("import", "import \"$1\"$0"),
    (
        "contract",
        "contract ${1:Name} {\n    author: \"${2:}\"\n    version: \"${3:1.0.0}\"\n    description: \"${4:My TON contract}\"\n    incomingMessages: ${5:AllowedMessages}\n    storage: ${6:Storage}\n}$0",
    ),
    ("struct", "struct ${1:Name} {\n    $0\n}"),
    ("enum", "enum ${1:Name} {\n    $0\n}"),
    ("type", "type ${1:Int} = ${2:int}$0"),
    ("const", "const ${1:FOO}: ${2:int} = ${3:0}$0"),
    ("global", "global ${1:foo}: ${2:int}$0"),
    ("fun", "fun ${1:name}($2)$3 {\n    $0\n}"),
    ("inline fun", "@inline\nfun ${1:name}($2)$3 {\n    $0\n}"),
    (
        "inline_ref fun",
        "@inline_ref\nfun ${1:name}($2)$3 {\n    $0\n}",
    ),
    ("asm fun", "fun ${1:name}($2)$3 asm \"$0\""),
    (
        "method fun",
        "fun ${1:Foo}.${2:name}(${3:self}$4)$5 {\n    $0\n}",
    ),
    (
        "static method fun",
        "fun ${1:Foo}.${2:name}($3)$4 {\n    $0\n}",
    ),
    ("get fun", "get fun ${1:name}($2)$3 {\n    $0\n}"),
];
