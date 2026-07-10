use super::FiftParsedDocument;
use super::reference::{FiftReference, is_definition_name};
use crate::DocumentSnapshot;
use crate::semantic_tokens::{SemanticToken, SemanticTokenType, SemanticTokensBuilder};
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

            if !is_definition_name(parent, node)
                && FiftReference::new(node, source_file)
                    .and_then(|reference| reference.resolve())
                    .is_some()
            {
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

fn push_function_token(
    builder: &mut SemanticTokensBuilder,
    node: Node<'_>,
    document: &DocumentSnapshot,
) {
    let range = document.text_index().range_of_node(document.text(), node);
    builder.add_token_at_range(range, SemanticTokenType::Function, 0);
}
