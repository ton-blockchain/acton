use expect_test::Expect;
use lsp_types::{GotoDefinitionResponse, SemanticTokensResult, Url};
use ton_ls::Backend;

use crate::self_contained::harness::case::normalize_case_name;
use crate::self_contained::harness::lsp::{goto_definition_params, semantic_tokens_params};
use crate::self_contained::harness::runner::{
    ResolveFeature, SemanticTokensFeature, case_resolve, case_semantic_tokens,
};

pub(crate) fn case_tlb_resolve(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_resolve_");
    case_resolve::<TlbResolveFeature>(&case_name, source, expect);
}

pub(crate) fn case_tlb_semantic_tokens(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_semantic_tokens_");
    case_semantic_tokens::<TlbSemanticTokensFeature>(&case_name, source, expect);
}

struct TlbResolveFeature;

impl ResolveFeature for TlbResolveFeature {
    const LANGUAGE_ID: &'static str = "tlb";
    const FILE_EXT: &'static str = "tlb";

    async fn request(
        backend: &Backend,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<GotoDefinitionResponse> {
        backend
            .handle_tlb_goto_definition(goto_definition_params(uri, position))
            .await
    }
}

struct TlbSemanticTokensFeature;

impl SemanticTokensFeature for TlbSemanticTokensFeature {
    const LANGUAGE_ID: &'static str = "tlb";
    const FILE_EXT: &'static str = "tlb";

    async fn request(backend: &Backend, uri: Url) -> Option<SemanticTokensResult> {
        backend
            .handle_tlb_semantic_tokens_full(semantic_tokens_params(uri))
            .await
    }
}
