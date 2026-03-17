use expect_test::Expect;
use lsp_types::{CompletionResponse, Hover, Url};
use ton_ls::Backend;

use crate::self_contained::harness::case::normalize_case_name;
use crate::self_contained::harness::lsp::{completion_params, hover_params};
use crate::self_contained::harness::runner::{
    CompletionFeature, HoverFeature, case_completion, case_completion_apply, case_hover,
};

pub(crate) fn case_toml_hover(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_hover_");
    case_hover::<TomlHoverFeature>(&case_name, source, expect);
}

pub(crate) fn case_toml_completion(test_fn_name: &str, source: &str, expect: Expect) {
    let case_name = normalize_case_name(test_fn_name, "test_completion_");
    case_completion::<TomlCompletionFeature>(&case_name, source, expect);
}

pub(crate) fn case_toml_completion_apply(
    test_fn_name: &str,
    source: &str,
    expected_labels: &[&str],
    completion_index: usize,
    expected_after_apply: &str,
) {
    let case_name = normalize_case_name(test_fn_name, "test_apply_completion_");
    case_completion_apply::<TomlCompletionFeature>(
        &case_name,
        source,
        expected_labels,
        completion_index,
        expected_after_apply,
    );
}

struct TomlHoverFeature;

impl HoverFeature for TomlHoverFeature {
    const LANGUAGE_ID: &'static str = "toml";
    const FILE_EXT: &'static str = "toml";

    async fn request(backend: &Backend, uri: Url, position: lsp_types::Position) -> Option<Hover> {
        backend.handle_toml_hover(hover_params(uri, position)).await
    }
}

struct TomlCompletionFeature;

impl CompletionFeature for TomlCompletionFeature {
    const LANGUAGE_ID: &'static str = "toml";
    const FILE_EXT: &'static str = "toml";

    async fn request(
        backend: &Backend,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<CompletionResponse> {
        backend
            .handle_toml_completion(completion_params(uri, position))
            .await
    }
}
