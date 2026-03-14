use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::fift::psi::FiftReference;
use crate::languages::semantic_tokens::{
    SemanticTokensBuilder as CommonSemanticTokensBuilder, semantic_tokens_result_id,
};
use crate::languages::tolk::semantic_tokens::TokenType;
use lsp_types::{
    SemanticToken, SemanticTokens, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensResult::Tokens,
};
use tree_sitter::Node;

impl Backend {
    pub async fn handle_fift_semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Option<SemanticTokensResult> {
        crate::profile!(self, "fift: semantic_tokens");
        let uri = params.text_document.uri;

        let file = self.registry.find_fift_file(&uri)?;
        let data = collect_function_tokens(&file);

        Some(Tokens(SemanticTokens {
            result_id: Some(semantic_tokens_result_id()),
            data,
        }))
    }
}

fn collect_function_tokens(file: &ParsedSnapshot<fift_syntax::SourceFile>) -> Vec<SemanticToken> {
    let mut builder = CommonSemanticTokensBuilder::new();
    for node in file.traverse() {
        if !node.is_named() {
            continue;
        }

        if is_function_definition(node.kind())
            && let Some(name_node) = node.child_by_field_name("name")
        {
            push_function_token(&mut builder, name_node, file);
        }

        if node.kind() == "identifier" {
            let Some(parent) = node.parent() else {
                continue;
            };

            if !is_definition_name(parent, node)
                && FiftReference::new(node, file.syntax())
                    .and_then(|reference| reference.resolve())
                    .is_some()
            {
                push_function_token(&mut builder, node, file);
            }
        }
    }

    builder.build()
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
    builder: &mut CommonSemanticTokensBuilder,
    node: Node<'_>,
    file: &ParsedSnapshot<fift_syntax::SourceFile>,
) {
    let range = file.range_of(node);
    builder.add_token_at_range(range, TokenType::Function as u32, 0);
}
