use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
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
    pub async fn handle_tlb_semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Option<SemanticTokensResult> {
        crate::profile!(self, "tlb: semantic_tokens");
        let uri = params.text_document.uri;

        let file = self.registry.find_tlb_file(&uri)?;
        let data = collect_tlb_tokens(&file);

        Some(Tokens(SemanticTokens {
            result_id: Some(semantic_tokens_result_id()),
            data,
        }))
    }
}

fn collect_tlb_tokens(file: &ParsedSnapshot<tlb_syntax::SourceFile>) -> Vec<SemanticToken> {
    let mut builder = CommonSemanticTokensBuilder::new();

    for node in file.traverse() {
        match node.kind() {
            "#" | "##" | "#<" | "#<=" | "builtin_field"
                if !parent_has_kind(node, "constructor_tag") =>
            {
                push_token(&mut builder, file, node, TokenType::Macro);
            }
            "field_named" => {
                if let Some(identifier) = node.child_by_field_name("name") {
                    push_token(&mut builder, file, identifier, TokenType::Property);
                }
            }
            "constructor_" => {
                if let Some(identifier) = node.child_by_field_name("name") {
                    push_token(&mut builder, file, identifier, TokenType::Type);
                }
            }
            "type_identifier" => {
                let token_type = classify_type_identifier(file, node);
                push_token(&mut builder, file, node, token_type);
            }
            _ => {}
        }
    }

    builder.build()
}

fn classify_type_identifier(
    file: &ParsedSnapshot<tlb_syntax::SourceFile>,
    node: Node<'_>,
) -> TokenType {
    let name = file.text_of(node);

    if is_builtin_type(name) {
        return TokenType::Macro;
    }

    if parent_has_kind(node, "type_parameter") {
        return TokenType::TypeParameter;
    }

    if let Some(parent) = node.parent()
        && matches!(parent.kind(), "combinator" | "combinator_expr")
    {
        return TokenType::Struct;
    }

    TokenType::Type
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
    builder: &mut CommonSemanticTokensBuilder,
    file: &ParsedSnapshot<tlb_syntax::SourceFile>,
    node: Node<'_>,
    token_type: TokenType,
) {
    let range = file.range_of(node);
    builder.add_token_at_range(range, token_type as u32, 0);
}

fn is_builtin_type(name: &str) -> bool {
    matches!(name, "Any" | "Cell" | "Int" | "UInt" | "Bits")
        || name.starts_with("bits")
        || name.starts_with("uint")
        || name.starts_with("int")
}
