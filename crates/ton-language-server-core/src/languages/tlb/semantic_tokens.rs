use super::TlbParsedDocument;
use crate::DocumentSnapshot;
use crate::semantic_tokens::{SemanticToken, SemanticTokenType, SemanticTokensBuilder};
use tree_sitter::Node;

pub(super) fn semantic_tokens(
    document: &DocumentSnapshot,
    parsed: &TlbParsedDocument,
) -> Vec<SemanticToken> {
    let mut builder = SemanticTokensBuilder::new();
    collect_tlb_tokens(parsed.source_file.tree.root_node(), document, &mut builder);
    builder.build()
}

fn collect_tlb_tokens(
    node: Node<'_>,
    document: &DocumentSnapshot,
    builder: &mut SemanticTokensBuilder,
) {
    match node.kind() {
        "#" | "##" | "#<" | "#<=" | "builtin_field"
            if !parent_has_kind(node, "constructor_tag") =>
        {
            push_token(builder, document, node, SemanticTokenType::Macro);
        }
        "field_named" => {
            if let Some(identifier) = node.child_by_field_name("name") {
                push_token(builder, document, identifier, SemanticTokenType::Property);
            }
        }
        "constructor_" => {
            if let Some(identifier) = node.child_by_field_name("name") {
                push_token(builder, document, identifier, SemanticTokenType::Type);
            }
        }
        "type_identifier" => {
            let token_type = classify_type_identifier(document, node);
            push_token(builder, document, node, token_type);
        }
        _ => {}
    }

    for child_index in 0..node.child_count() {
        let Ok(child_index) = u32::try_from(child_index) else {
            break;
        };
        let Some(child) = node.child(child_index) else {
            continue;
        };
        collect_tlb_tokens(child, document, builder);
    }
}

fn classify_type_identifier(document: &DocumentSnapshot, node: Node<'_>) -> SemanticTokenType {
    let name = node
        .utf8_text(document.text().as_bytes())
        .ok()
        .map(str::trim)
        .unwrap_or_default();

    if is_builtin_type(name) {
        return SemanticTokenType::Macro;
    }

    if parent_has_kind(node, "type_parameter") {
        return SemanticTokenType::TypeParameter;
    }

    if let Some(parent) = node.parent()
        && matches!(parent.kind(), "combinator" | "combinator_expr")
    {
        return SemanticTokenType::Struct;
    }

    SemanticTokenType::Type
}

fn parent_has_kind(mut node: Node<'_>, kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return true;
        }
        node = parent;
    }

    false
}

fn push_token(
    builder: &mut SemanticTokensBuilder,
    document: &DocumentSnapshot,
    node: Node<'_>,
    token_type: SemanticTokenType,
) {
    let range = document.text_index().range_of_node(document.text(), node);
    builder.add_token_at_range(range, token_type, 0);
}

fn is_builtin_type(name: &str) -> bool {
    matches!(name, "Any" | "Cell" | "Int" | "UInt" | "Bits")
        || name.starts_with("bits")
        || name.starts_with("uint")
        || name.starts_with("int")
}
