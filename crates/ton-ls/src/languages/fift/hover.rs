use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::instruction_docs::{build_hover_markdown, get_tasm_spec};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Range};
use tree_sitter::Node;

struct HoverTarget {
    name: String,
    range: Range,
}

impl Backend {
    pub async fn handle_fift_hover(&self, params: HoverParams) -> Option<Hover> {
        crate::profile!(self, "fift: hover");
        let uri = params.text_document_position_params.text_document.uri;

        let position = params.text_document_position_params.position;
        let file = self.registry.find_fift_file(&uri)?;
        let target = find_hover_target(&file, position)?;

        let tasm_spec = get_tasm_spec()?;

        let markdown = build_hover_markdown(&target.name, tasm_spec)?;

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: Some(target.range),
        })
    }
}

fn find_hover_target(
    file: &ParsedSnapshot<fift_syntax::SourceFile>,
    position: lsp_types::Position,
) -> Option<HoverTarget> {
    let node = file.node_at(position)?;

    let raw_name = file.text_of(node);
    if raw_name.is_empty() {
        return None;
    }

    let name = adjusted_hover_name(file, node).unwrap_or_else(|| raw_name.to_string());

    let range = file.range_of(node);
    Some(HoverTarget { name, range })
}

fn adjusted_hover_name(
    file: &ParsedSnapshot<fift_syntax::SourceFile>,
    node: Node<'_>,
) -> Option<String> {
    let instruction = enclosing_instruction(node)?;
    let name_node = instruction.named_child(0)?;
    let instruction_name = file.text_of(name_node).trim();
    if instruction_name.is_empty() {
        return None;
    }

    let args = collect_inline_argument_nodes(file, instruction);
    Some(adjust_name(file, instruction_name, &args))
}

fn enclosing_instruction(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if node.kind() == "instruction" {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn collect_inline_argument_nodes<'tree>(
    file: &ParsedSnapshot<fift_syntax::SourceFile>,
    instruction: Node<'tree>,
) -> Vec<Node<'tree>> {
    let mut args_reversed = Vec::new();
    let mut next_start = instruction.start_byte();
    let mut sibling = instruction.prev_named_sibling();

    while let Some(current) = sibling {
        if current.kind() != "instruction" {
            break;
        }
        if contains_line_break(file.source(), current.end_byte(), next_start) {
            break;
        }

        if let Some(argument_node) = current.named_child(0) {
            args_reversed.push(argument_node);
        }

        next_start = current.start_byte();
        sibling = current.prev_named_sibling();
    }

    args_reversed.reverse();
    args_reversed
}

fn contains_line_break(source: &str, start: usize, end: usize) -> bool {
    let Some(slice) = source.get(start..end) else {
        return true;
    };
    slice.bytes().any(|byte| matches!(byte, b'\n' | b'\r'))
}

fn adjust_name(
    file: &ParsedSnapshot<fift_syntax::SourceFile>,
    name: &str,
    args: &[Node<'_>],
) -> String {
    let name = name.trim().to_ascii_uppercase();

    if name == "PUSHINT" {
        if args.is_empty() {
            return "PUSHINT_4".to_string();
        }

        let arg = args
            .first()
            .and_then(|&node| file.text_of(node).trim().parse::<i64>().ok());

        let Some(arg) = arg else {
            return "PUSHINT_4".to_string();
        };

        if (0..=15).contains(&arg) {
            return "PUSHINT_4".to_string();
        }
        if (-128..=127).contains(&arg) {
            return "PUSHINT_8".to_string();
        }
        if (-32_768..=32_767).contains(&arg) {
            return "PUSHINT_16".to_string();
        }

        return "PUSHINT_LONG".to_string();
    }

    if name == "PUSH" {
        if args.len() == 1 && is_stack_register(file, args[0]) {
            return "PUSH".to_string();
        }
        if args.len() == 2 {
            return "PUSH2".to_string();
        }
        if args.len() == 3 {
            return "PUSH3".to_string();
        }
        return name;
    }

    if name == "XCHG0" {
        return "XCHG_0I".to_string();
    }

    if name == "XCHG" {
        return "XCHG_IJ".to_string();
    }

    name
}

fn is_stack_register(file: &ParsedSnapshot<fift_syntax::SourceFile>, node: Node<'_>) -> bool {
    if node.kind() == "stack_ref" {
        return true;
    }

    let text = file.text_of(node).trim();
    let Some(rest) = text.strip_prefix('s').or_else(|| text.strip_prefix('S')) else {
        return false;
    };

    let digits = rest.strip_prefix('-').unwrap_or(rest);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}
