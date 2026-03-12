use expect_test::Expect;
use lsp_types::{CodeLens, FoldingRange, Hover, Url};
use ton_ls::Backend;

use crate::self_contained::harness::case::normalize_case_name;
use crate::self_contained::harness::lsp::{code_lens_params, folding_range_params, hover_params};
use crate::self_contained::harness::runner::{
    CodeLensFeature, FoldingFeature, HoverFeature, case_code_lens, case_folding, case_hover,
};

pub(crate) fn case_tasm_hover(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_hover_");
    case_hover::<TasmHoverFeature>(&case_name, source, expect);
}

pub(crate) fn case_tasm_folding(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_folding_");
    case_folding::<TasmFoldingFeature>(&case_name, source, expect);
}

pub(crate) fn case_tasm_code_lens(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_code_lens_");
    case_code_lens::<TasmCodeLensFeature>(&case_name, source, expect);
}

struct TasmHoverFeature;

impl HoverFeature for TasmHoverFeature {
    const LANGUAGE_ID: &'static str = "tasm";
    const FILE_EXT: &'static str = "tasm";

    async fn request(backend: &Backend, uri: Url, position: lsp_types::Position) -> Option<Hover> {
        backend.handle_tasm_hover(hover_params(uri, position)).await
    }
}

struct TasmFoldingFeature;

impl FoldingFeature for TasmFoldingFeature {
    const LANGUAGE_ID: &'static str = "tasm";
    const FILE_EXT: &'static str = "tasm";

    async fn request(backend: &Backend, uri: Url) -> Option<Vec<FoldingRange>> {
        backend
            .handle_tasm_folding_range(folding_range_params(uri))
            .await
    }
}

struct TasmCodeLensFeature;

impl CodeLensFeature for TasmCodeLensFeature {
    const LANGUAGE_ID: &'static str = "tasm";
    const FILE_EXT: &'static str = "tasm";

    async fn request(backend: &Backend, uri: Url) -> Option<Vec<CodeLens>> {
        backend.handle_tasm_code_lens(code_lens_params(uri)).await
    }
}
