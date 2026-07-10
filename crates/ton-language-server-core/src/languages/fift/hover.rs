use super::FiftParsedDocument;
use super::reference::{FiftReference, definition_parent};
use crate::languages::instruction_docs::InstructionSpec;
use crate::{DocumentSnapshot, Hover, Position};
use tree_sitter::Node;

pub(super) fn hover(
    spec: Option<&InstructionSpec>,
    document: &DocumentSnapshot,
    parsed: &FiftParsedDocument,
    position: Position,
) -> Option<Hover> {
    let point = document
        .text_index()
        .position_to_point(document.text(), position);
    let node = parsed
        .source_file
        .root_node()
        .descendant_for_point_range(point, point)?;

    if let Some(definition) =
        FiftReference::new(node, &parsed.source_file).and_then(|reference| reference.resolve())
    {
        let body = definition_parent(definition).unwrap_or(definition);
        return Some(Hover::new(
            format!("```fift\n{}\n```", document.text_of(body)),
            Some(
                document
                    .text_index()
                    .range_of_node(document.text(), definition),
            ),
        ));
    }

    let spec = spec?;
    let instruction = instruction_ancestor(node)?;
    let name_node = instruction.named_child(0)?;
    if name_node != node {
        return None;
    }
    let name = adjusted_instruction_name(document, instruction)?;
    let instruction_doc = spec.instruction(&name)?;

    Some(Hover::new(
        instruction_doc.render_hover(),
        Some(
            document
                .text_index()
                .range_of_node(document.text(), name_node),
        ),
    ))
}

fn instruction_ancestor(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if node.kind() == "instruction" {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn adjusted_instruction_name(document: &DocumentSnapshot, instruction: Node<'_>) -> Option<String> {
    let name = document
        .text_of(instruction.named_child(0)?)
        .trim()
        .to_ascii_uppercase();
    let arguments = inline_arguments(document, instruction);

    match name.as_str() {
        "PUSHINT" => {
            let value = arguments
                .first()
                .and_then(|node| document.text_of(*node).trim().parse::<i64>().ok());
            Some(
                match value {
                    Some(0..=15) | None => "PUSHINT_4",
                    Some(-128..=127) => "PUSHINT_8",
                    Some(-32_768..=32_767) => "PUSHINT_16",
                    Some(_) => "PUSHINT_LONG",
                }
                .to_owned(),
            )
        }
        "PUSH" => Some(
            match arguments.as_slice() {
                [argument] if is_stack_register(document, *argument) => "PUSH",
                [_, _] => "PUSH2",
                [_, _, _] => "PUSH3",
                _ => "PUSH",
            }
            .to_owned(),
        ),
        "XCHG0" => Some("XCHG_0I".to_owned()),
        "XCHG" => Some("XCHG_IJ".to_owned()),
        _ => Some(name),
    }
}

fn inline_arguments<'tree>(
    document: &DocumentSnapshot,
    instruction: Node<'tree>,
) -> Vec<Node<'tree>> {
    let mut reversed = Vec::new();
    let mut next_start = instruction.start_byte();
    let mut sibling = instruction.prev_named_sibling();

    while let Some(current) = sibling {
        if current.kind() != "instruction"
            || contains_line_break(document.text(), current.end_byte(), next_start)
        {
            break;
        }
        if let Some(argument) = current.named_child(0) {
            reversed.push(argument);
        }

        next_start = current.start_byte();
        sibling = current.prev_named_sibling();
    }

    reversed.reverse();
    reversed
}

fn contains_line_break(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_none_or(|text| text.bytes().any(|byte| matches!(byte, b'\n' | b'\r')))
}

fn is_stack_register(document: &DocumentSnapshot, node: Node<'_>) -> bool {
    if node.kind() == "stack_ref" {
        return true;
    }

    let text = document.text_of(node).trim();
    let Some(value) = text.strip_prefix('s').or_else(|| text.strip_prefix('S')) else {
        return false;
    };
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}
