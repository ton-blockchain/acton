use super::TlbParsedDocument;
use super::completion::providers::builtin_types::BUILTIN_TYPES;
use super::reference::{reference_identifier, resolved_items_at};
use crate::{DocumentSnapshot, Hover, Position};

pub(super) fn hover(
    document: &DocumentSnapshot,
    parsed: &TlbParsedDocument,
    position: Position,
) -> Option<Hover> {
    let source = document.text();
    let point = document.text_index().position_to_point(source, position);
    let node = parsed
        .source_file
        .root_node()
        .descendant_for_point_range(point, point)?;
    let node_text = document.text_of(node).trim();
    let node_range = document.text_index().range_of_node(source, node);

    if node
        .parent()
        .is_none_or(|parent| parent.kind() != "constructor_tag")
        && let Some((_, description)) = BUILTIN_TYPES
            .iter()
            .find(|(name, description)| *name == node_text && !description.is_empty())
    {
        return Some(Hover::new(*description, Some(node_range)));
    }
    if let Some(contents) = arbitrary_integer_doc(node_text) {
        return Some(Hover::new(contents, Some(node_range)));
    }

    let identifier = reference_identifier(node)?;
    let range = document.text_index().range_of_node(source, identifier);

    let resolved = resolved_items_at(&parsed.source_file, identifier);
    if !resolved.is_empty() {
        let declarations = resolved
            .into_iter()
            .filter_map(|item| declaration_text(item.node, document))
            .collect::<Vec<_>>();
        if !declarations.is_empty() {
            return Some(Hover::new(
                format!("```tlb\n{}\n```", declarations.join("\n\n")),
                Some(range),
            ));
        }
    }

    None
}

fn declaration_text<'a>(
    node: tree_sitter::Node<'_>,
    document: &'a DocumentSnapshot,
) -> Option<&'a str> {
    let declaration = if node.kind() == "declaration" {
        node
    } else {
        ancestor(node, "declaration")?
    };
    Some(document.text_of(declaration))
}

fn ancestor<'tree>(
    mut node: tree_sitter::Node<'tree>,
    kind: &str,
) -> Option<tree_sitter::Node<'tree>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn arbitrary_integer_doc(name: &str) -> Option<String> {
    let (kind, width) = if let Some(width) = name.strip_prefix("uint") {
        ("unsigned integer", width)
    } else if let Some(width) = name.strip_prefix("int") {
        ("signed integer", width)
    } else if let Some(width) = name.strip_prefix("bits") {
        ("data", width)
    } else {
        return None;
    };
    let width = width.parse::<u16>().ok()?;
    let valid = match kind {
        "unsigned integer" => (1..=256).contains(&width),
        _ => (1..=257).contains(&width),
    };
    if !valid {
        return None;
    }

    let range = match kind {
        "unsigned integer" => format!("0 to 2^{width} - 1"),
        "signed integer" => format!("-2^{} to 2^{} - 1", width - 1, width - 1),
        _ => format!("0 to {width} bits"),
    };
    Some(format!(
        "**{name}** - {width}-bit {kind}\n\n- **Range**: {range}\n- **Size**: {width} bits"
    ))
}
