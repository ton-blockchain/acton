use super::super::context::TasmCompletionContext;
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};
use crate::{CompletionItem, CompletionItemKind};

pub(crate) struct InstructionCompletionProvider;

impl CompletionProvider<TasmCompletionContext<'_>> for InstructionCompletionProvider {
    fn collect(&self, context: &TasmCompletionContext<'_>, collector: &mut CompletionCollector) {
        for instruction in context.spec.instructions.values() {
            let snippet = instruction_snippet(&instruction.name, &instruction.operands);
            let detail = if instruction.operands.is_empty() {
                instruction.stack.clone().unwrap_or_default()
            } else {
                format!("{} {}", instruction.name, instruction.operands.join(" "))
            };
            let mut item = CompletionItem::new(&instruction.name, CompletionItemKind::Function)
                .with_snippet_replacement(context.replacement_range, snippet)
                .with_documentation(instruction.render_hover());
            if !detail.is_empty() {
                item.detail = Some(detail);
            }
            collector.add(
                item,
                context.rank_for(CompletionCategory::Function, &instruction.name),
            );
        }
    }
}

fn instruction_snippet(name: &str, operands: &[String]) -> String {
    if operands.is_empty() {
        return name.to_owned();
    }
    let mut snippet = name.to_owned();
    for (index, operand) in operands.iter().enumerate() {
        snippet.push(' ');
        snippet.push_str("${");
        snippet.push_str(&(index + 1).to_string());
        snippet.push(':');
        snippet.push_str(&escape_snippet_placeholder(operand));
        snippet.push('}');
    }
    snippet.push_str("$0");
    snippet
}

fn escape_snippet_placeholder(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('}', "\\}")
}
