use super::FiftParsedDocument;
use crate::DocumentSnapshot;
use crate::semantic_tokens::{SemanticToken, SemanticTokenType, SemanticTokensBuilder};
use fift_syntax::{AstNode, DefinitionKind, TopLevel};
use tree_sitter::Node;

pub(super) fn semantic_tokens(
    document: &DocumentSnapshot,
    parsed: &FiftParsedDocument,
) -> Vec<SemanticToken> {
    let mut builder = SemanticTokensBuilder::new();
    collect_function_tokens(
        parsed.source_file.tree.root_node(),
        document,
        &parsed.source_file,
        &mut builder,
    );
    builder.build()
}

fn collect_function_tokens(
    node: Node<'_>,
    document: &DocumentSnapshot,
    source_file: &fift_syntax::SourceFile,
    builder: &mut SemanticTokensBuilder,
) {
    if node.is_named() {
        if is_function_definition(node.kind())
            && let Some(name_node) = node.child_by_field_name("name")
        {
            push_function_token(builder, name_node, document);
        }

        if node.kind() == "identifier" {
            let Some(parent) = node.parent() else {
                return;
            };

            if !is_definition_name(parent, node) && resolve_reference(node, source_file).is_some() {
                push_function_token(builder, node, document);
            }
        }
    }

    for child_index in 0..node.child_count() {
        let Ok(child_index) = u32::try_from(child_index) else {
            break;
        };
        let Some(child) = node.child(child_index) else {
            continue;
        };
        collect_function_tokens(child, document, source_file, builder);
    }
}

fn resolve_reference<'tree>(
    node: Node<'tree>,
    source_file: &'tree fift_syntax::SourceFile,
) -> Option<Node<'tree>> {
    let identifier = find_reference_identifier(node)?;
    let source = source_file.source.as_ref();
    let target_name = identifier.utf8_text(source.as_bytes()).ok()?.trim();
    if target_name.is_empty() {
        return None;
    }

    for top_level in source_file.top_levels() {
        let TopLevel::Definition(definition) = top_level else {
            continue;
        };
        let Some(kind) = definition.kind() else {
            continue;
        };
        let Some(name) = kind.name() else {
            continue;
        };
        if name.text(source).trim() != target_name {
            continue;
        }

        let definition_node = match kind {
            DefinitionKind::ProcDefinition(node) => node.syntax(),
            DefinitionKind::ProcInlineDefinition(node) => node.syntax(),
            DefinitionKind::ProcRefDefinition(node) => node.syntax(),
            DefinitionKind::MethodDefinition(node) => node.syntax(),
            DefinitionKind::Unmapped(_) => continue,
        };
        return Some(definition_node);
    }

    None
}

fn find_reference_identifier(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        match node.kind() {
            "identifier" => return Some(node),
            "proc_call" | "instruction" => {
                let identifier = node.named_child(0)?;
                if identifier.kind() == "identifier" {
                    return Some(identifier);
                }
            }
            _ => {}
        }

        node = node.parent()?;
    }
}

fn is_function_definition(kind: &str) -> bool {
    matches!(
        kind,
        "proc_definition"
            | "proc_inline_definition"
            | "proc_ref_definition"
            | "method_definition"
            | "proc_declaration"
            | "method_declaration"
            | "declaration"
    )
}

fn is_definition_name(parent: Node<'_>, node: Node<'_>) -> bool {
    if parent.child_by_field_name("name") != Some(node) {
        return false;
    }

    is_function_definition(parent.kind())
}

fn push_function_token(
    builder: &mut SemanticTokensBuilder,
    node: Node<'_>,
    document: &DocumentSnapshot,
) {
    let range = document.text_index().range_of_node(document.text(), node);
    builder.add_token_at_range(range, SemanticTokenType::Function, 0);
}
