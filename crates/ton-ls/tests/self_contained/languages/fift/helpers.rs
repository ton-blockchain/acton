use expect_test::Expect;
use lsp_types::{FoldingRange, GotoDefinitionResponse, Hover, Location, SemanticTokensResult, Url};
use ton_ls::Backend;

use crate::self_contained::harness::case::normalize_case_name;
use crate::self_contained::harness::lsp::{
    folding_range_params, goto_definition_params, hover_params, references_params,
    semantic_tokens_params,
};
use crate::self_contained::harness::runner::{
    FoldingFeature, HoverFeature, ReferencesFeature, ResolveFeature, SemanticTokensFeature,
    case_folding, case_hover, case_references, case_resolve, case_semantic_tokens,
};

pub(crate) fn case_fift_resolve(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_resolve_");
    case_resolve::<FiftResolveFeature>(&case_name, source, expect);
}

pub(crate) fn case_fift_references(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_references_");
    case_references::<FiftReferencesFeature>(&case_name, source, expect);
}

pub(crate) fn case_fift_references_with_declaration(
    test_fn_name: &str,
    source: &str,
    expect: Expect,
) {
    let case_name = normalize_case_name(test_fn_name, "test_references_with_declaration_");
    case_references::<FiftReferencesWithDeclarationFeature>(&case_name, source, expect);
}

pub(crate) fn case_fift_hover(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_hover_");
    case_hover::<FiftHoverFeature>(&case_name, source, expect);
}

pub(crate) fn case_fift_folding(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_folding_");
    case_folding::<FiftFoldingFeature>(&case_name, source, expect);
}

pub(crate) fn case_fift_semantic_tokens(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_semantic_tokens_");
    case_semantic_tokens::<FiftSemanticTokensFeature>(&case_name, source, expect);
}

struct FiftResolveFeature;

impl ResolveFeature for FiftResolveFeature {
    const LANGUAGE_ID: &'static str = "fift";
    const FILE_EXT: &'static str = "fif";

    async fn request(
        backend: &Backend,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<GotoDefinitionResponse> {
        backend
            .handle_fift_goto_definition(goto_definition_params(uri, position))
            .await
    }
}

struct FiftReferencesFeature;

impl ReferencesFeature for FiftReferencesFeature {
    const LANGUAGE_ID: &'static str = "fift";
    const FILE_EXT: &'static str = "fif";
    const INCLUDE_DECLARATION: bool = false;

    async fn request(
        backend: &Backend,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<Vec<Location>> {
        backend
            .handle_fift_references(references_params(uri, position, Self::INCLUDE_DECLARATION))
            .await
    }
}

struct FiftReferencesWithDeclarationFeature;

impl ReferencesFeature for FiftReferencesWithDeclarationFeature {
    const LANGUAGE_ID: &'static str = "fift";
    const FILE_EXT: &'static str = "fif";
    const INCLUDE_DECLARATION: bool = true;

    async fn request(
        backend: &Backend,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<Vec<Location>> {
        backend
            .handle_fift_references(references_params(uri, position, Self::INCLUDE_DECLARATION))
            .await
    }
}

struct FiftHoverFeature;

impl HoverFeature for FiftHoverFeature {
    const LANGUAGE_ID: &'static str = "fift";
    const FILE_EXT: &'static str = "fif";

    async fn request(backend: &Backend, uri: Url, position: lsp_types::Position) -> Option<Hover> {
        backend.handle_fift_hover(hover_params(uri, position)).await
    }
}

struct FiftFoldingFeature;

impl FoldingFeature for FiftFoldingFeature {
    const LANGUAGE_ID: &'static str = "fift";
    const FILE_EXT: &'static str = "fif";

    async fn request(backend: &Backend, uri: Url) -> Option<Vec<FoldingRange>> {
        backend
            .handle_fift_folding_range(folding_range_params(uri))
            .await
    }
}

struct FiftSemanticTokensFeature;

impl SemanticTokensFeature for FiftSemanticTokensFeature {
    const LANGUAGE_ID: &'static str = "fift";
    const FILE_EXT: &'static str = "fif";

    async fn request(backend: &Backend, uri: Url) -> Option<SemanticTokensResult> {
        backend
            .handle_fift_semantic_tokens_full(semantic_tokens_params(uri))
            .await
    }
}
