use std::collections::BTreeMap;
use std::sync::Arc;

use dashmap::DashMap;
use expect_test::Expect;
use lsp_types::{
    DidOpenTextDocumentParams, FoldingRange, GotoDefinitionResponse, Hover, InitializeParams,
    InitializeResult, Location, SemanticTokensResult, TextDocumentItem, Url,
};
use tolk_resolver::file_db::FileDb;
use ton_ls::{Backend, SelfContainedLanguageRegistry};
use tower_lsp::{LanguageServer, LspService};

use crate::self_contained::harness::case::parse_source;
use crate::self_contained::harness::lsp::{extract_semantic_legend, uri_for_case};
use crate::self_contained::harness::render::{
    render_folding_ranges, render_hover, render_references, render_resolve, render_semantic_tokens,
};

pub(crate) trait ResolveFeature {
    const LANGUAGE_ID: &'static str;
    const FILE_EXT: &'static str;

    async fn request(
        backend: &Backend,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<GotoDefinitionResponse>;
}

pub(crate) trait SemanticTokensFeature {
    const LANGUAGE_ID: &'static str;
    const FILE_EXT: &'static str;

    async fn request(backend: &Backend, uri: Url) -> Option<SemanticTokensResult>;
}

pub(crate) trait HoverFeature {
    const LANGUAGE_ID: &'static str;
    const FILE_EXT: &'static str;

    async fn request(backend: &Backend, uri: Url, position: lsp_types::Position) -> Option<Hover>;
}

pub(crate) trait ReferencesFeature {
    const LANGUAGE_ID: &'static str;
    const FILE_EXT: &'static str;
    const INCLUDE_DECLARATION: bool;

    async fn request(
        backend: &Backend,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<Vec<Location>>;
}

pub(crate) trait FoldingFeature {
    const LANGUAGE_ID: &'static str;
    const FILE_EXT: &'static str;

    async fn request(backend: &Backend, uri: Url) -> Option<Vec<FoldingRange>>;
}

pub(crate) fn case_resolve<F: ResolveFeature>(case_name: &str, source: &str, expect: Expect) {
    let parsed = parse_source(source).expect("failed to parse source snippet");
    assert!(
        !parsed.carets.is_empty(),
        "resolve case must contain at least one caret marker"
    );

    let actual = run_async(async move {
        let server = TestServer::new();
        let uri = uri_for_case(case_name, F::FILE_EXT);
        server
            .open_document(&uri, F::LANGUAGE_ID, &parsed.source)
            .await;

        let mut lines = Vec::new();
        for caret in &parsed.carets {
            let response = F::request(server.backend(), uri.clone(), caret.position).await;
            lines.extend(render_resolve(caret.position, response));
        }
        lines.join("\n")
    });

    expect.assert_eq(&actual);
}

pub(crate) fn case_semantic_tokens<F: SemanticTokensFeature>(
    case_name: &str,
    source: &str,
    expect: Expect,
) {
    let parsed = parse_source(source).expect("failed to parse source snippet");
    assert!(
        parsed.carets.is_empty(),
        "semantic tokens case must not contain carets"
    );

    let actual = run_async(async move {
        let server = TestServer::new();
        let uri = uri_for_case(case_name, F::FILE_EXT);
        server
            .open_document(&uri, F::LANGUAGE_ID, &parsed.source)
            .await;

        let init = server
            .initialize()
            .await
            .expect("initialize should succeed for semantic tokens tests");
        let legend = extract_semantic_legend(&init).expect("semantic legend should be available");

        let response = F::request(server.backend(), uri).await;
        let tokens = match response {
            Some(SemanticTokensResult::Tokens(tokens)) => tokens.data,
            Some(SemanticTokensResult::Partial(partial)) => partial.data,
            None => Vec::new(),
        };

        if tokens.is_empty() {
            return "<none>".to_owned();
        }

        render_semantic_tokens(&parsed.source, &tokens, &legend).join("\n")
    });

    expect.assert_eq(&actual);
}

pub(crate) fn case_hover<F: HoverFeature>(case_name: &str, source: &str, expect: Expect) {
    let parsed = parse_source(source).expect("failed to parse source snippet");
    assert!(
        !parsed.carets.is_empty(),
        "hover case must contain at least one caret marker"
    );

    let actual = run_async(async move {
        let server = TestServer::new();
        let uri = uri_for_case(case_name, F::FILE_EXT);
        server
            .open_document(&uri, F::LANGUAGE_ID, &parsed.source)
            .await;

        let mut lines = Vec::new();
        for caret in &parsed.carets {
            let response = F::request(server.backend(), uri.clone(), caret.position).await;
            lines.push(render_hover(response));
        }
        lines.join("\n\n---\n\n")
    });

    expect.assert_eq(&actual);
}

pub(crate) fn case_references<F: ReferencesFeature>(case_name: &str, source: &str, expect: Expect) {
    let parsed = parse_source(source).expect("failed to parse source snippet");
    assert!(
        !parsed.carets.is_empty(),
        "references case must contain at least one caret marker"
    );

    let actual = run_async(async move {
        let server = TestServer::new();
        let uri = uri_for_case(case_name, F::FILE_EXT);
        server
            .open_document(&uri, F::LANGUAGE_ID, &parsed.source)
            .await;

        let mut lines = Vec::new();
        for caret in &parsed.carets {
            let response = F::request(server.backend(), uri.clone(), caret.position).await;
            lines.push(render_references(caret.position, response));
        }
        lines.join("\n")
    });

    expect.assert_eq(&actual);
}

pub(crate) fn case_folding<F: FoldingFeature>(case_name: &str, source: &str, expect: Expect) {
    let parsed = parse_source(source).expect("failed to parse source snippet");
    assert!(
        parsed.carets.is_empty(),
        "folding case must not contain carets"
    );

    let actual = run_async(async move {
        let server = TestServer::new();
        let uri = uri_for_case(case_name, F::FILE_EXT);
        server
            .open_document(&uri, F::LANGUAGE_ID, &parsed.source)
            .await;

        let response = F::request(server.backend(), uri).await;
        render_folding_ranges(response)
    });

    expect.assert_eq(&actual);
}

struct TestServer {
    service: LspService<Backend>,
}

impl TestServer {
    fn new() -> Self {
        let project_root = std::env::temp_dir().join("ton-ls-self-contained-tests");
        let stdlib_root = project_root.join("stdlib");
        let acton_root = project_root.join("acton");

        let (service, _socket) = LspService::new(move |client| Backend {
            client,
            file_db: Arc::new(FileDb::new(stdlib_root.clone(), Some(acton_root.clone()))),
            project_root: project_root.clone(),
            mappings: Option::<BTreeMap<String, String>>::None,
            documents: DashMap::new(),
            analysis: DashMap::new(),
            file_urls: DashMap::new(),
            registry: SelfContainedLanguageRegistry::new(),
            #[cfg(feature = "profiling")]
            profiling: Arc::new(ton_ls::ProfilingContext::new()),
        });

        Self { service }
    }

    fn backend(&self) -> &Backend {
        self.service.inner()
    }

    async fn initialize(&self) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        LanguageServer::initialize(self.backend(), InitializeParams::default()).await
    }

    async fn open_document(&self, uri: &Url, language_id: &str, text: &str) {
        LanguageServer::did_open(
            self.backend(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: language_id.to_owned(),
                    version: 1,
                    text: text.to_owned(),
                },
            },
        )
        .await;
    }
}

fn run_async<T>(future: impl std::future::Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime for self-contained tests");
    runtime.block_on(future)
}
