use std::collections::BTreeMap;
use std::sync::Arc;

use dashmap::DashMap;
use expect_test::Expect;
use lsp_types::{
    CodeLens, CompletionItem, CompletionResponse, CompletionTextEdit, DidOpenTextDocumentParams,
    FoldingRange, GotoDefinitionResponse, Hover, InitializeParams, InitializeResult, Location,
    Position, SemanticTokensResult, TextDocumentItem, Url,
};
use tolk_resolver::file_db::FileDb;
use ton_ls::{Backend, SelfContainedLanguageRegistry};
use tower_lsp::{LanguageServer, LspService};

use crate::self_contained::harness::case::parse_source;
use crate::self_contained::harness::lsp::{extract_semantic_legend, uri_for_case};
use crate::self_contained::harness::render::{
    render_code_lenses, render_completion_items, render_folding_ranges, render_hover,
    render_references, render_resolve, render_semantic_tokens,
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

pub(crate) trait CodeLensFeature {
    const LANGUAGE_ID: &'static str;
    const FILE_EXT: &'static str;

    async fn request(backend: &Backend, uri: Url) -> Option<Vec<CodeLens>>;
}

pub(crate) trait CompletionFeature {
    const LANGUAGE_ID: &'static str;
    const FILE_EXT: &'static str;

    async fn request(backend: &Backend, uri: Url, position: Position)
    -> Option<CompletionResponse>;
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

pub(crate) fn case_code_lens<F: CodeLensFeature>(case_name: &str, source: &str, expect: Expect) {
    let parsed = parse_source(source).expect("failed to parse source snippet");
    assert!(
        parsed.carets.is_empty(),
        "code lens case must not contain carets"
    );

    let actual = run_async(async move {
        let server = TestServer::new();
        let uri = uri_for_case(case_name, F::FILE_EXT);
        server
            .open_document(&uri, F::LANGUAGE_ID, &parsed.source)
            .await;

        let response = F::request(server.backend(), uri).await;
        render_code_lenses(response)
    });

    expect.assert_eq(&actual);
}

pub(crate) fn case_completion<F: CompletionFeature>(case_name: &str, source: &str, expect: Expect) {
    let parsed = parse_source(source).expect("failed to parse source snippet");
    assert_eq!(
        parsed.carets.len(),
        1,
        "completion case must contain exactly one caret marker"
    );
    let caret = parsed.carets[0].position;

    let actual = run_async(async move {
        let server = TestServer::new();
        let uri = uri_for_case(case_name, F::FILE_EXT);
        server
            .open_document(&uri, F::LANGUAGE_ID, &parsed.source)
            .await;

        let response = F::request(server.backend(), uri, caret).await;
        render_completion_items(response)
    });

    expect.assert_eq(&actual);
}

pub(crate) fn case_completion_apply<F: CompletionFeature>(
    case_name: &str,
    source: &str,
    expected_labels: &[&str],
    completion_index: usize,
    expected_after_apply: &str,
) {
    let parsed_input = parse_source(source).expect("failed to parse completion input snippet");
    assert_eq!(
        parsed_input.carets.len(),
        1,
        "completion apply input must contain exactly one caret marker"
    );
    let parsed_expected =
        parse_source(expected_after_apply).expect("failed to parse completion expected snippet");
    assert_eq!(
        parsed_expected.carets.len(),
        1,
        "completion apply expected snippet must contain exactly one caret marker"
    );

    let input_caret = parsed_input.carets[0].position;
    let expected_caret = parsed_expected.carets[0].position;

    let expected_labels = expected_labels
        .iter()
        .map(|label| (*label).to_owned())
        .collect::<Vec<_>>();

    let (actual_source, actual_caret) = run_async(async move {
        let server = TestServer::new();
        let uri = uri_for_case(case_name, F::FILE_EXT);
        server
            .open_document(&uri, F::LANGUAGE_ID, &parsed_input.source)
            .await;

        let response = F::request(server.backend(), uri, input_caret).await;
        let items = completion_items_from_response(response);
        let labels = items
            .iter()
            .map(|item| item.label.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            expected_labels,
            "completion labels mismatch before apply\nactual:\n{}\nexpected:\n{}",
            render_label_list(&labels),
            render_label_list(&expected_labels)
        );

        let item = items.get(completion_index).unwrap_or_else(|| {
            panic!(
                "completion index {} out of bounds, available items: {}\n{}",
                completion_index,
                items.len(),
                render_label_list(&labels)
            )
        });

        apply_completion_item(&parsed_input.source, input_caret, item)
    });

    assert_eq!(
        actual_source, parsed_expected.source,
        "completion apply text mismatch\nactual:\n{}\nexpected:\n{}",
        actual_source, parsed_expected.source
    );
    assert_eq!(
        actual_caret,
        expected_caret,
        "completion apply caret mismatch\nactual:\n{}\nexpected:\n{}",
        insert_caret_marker(&actual_source, actual_caret),
        insert_caret_marker(&parsed_expected.source, expected_caret)
    );
}

fn render_label_list(labels: &[String]) -> String {
    if labels.is_empty() {
        return "<empty>".to_owned();
    }

    labels
        .iter()
        .enumerate()
        .map(|(idx, label)| format!("{idx}: {label}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn completion_items_from_response(response: Option<CompletionResponse>) -> Vec<CompletionItem> {
    match response {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        None => Vec::new(),
    }
}

fn apply_completion_item(
    source: &str,
    cursor: Position,
    item: &CompletionItem,
) -> (String, Position) {
    let mut updated = source.to_owned();

    let (start, end, insert_text, is_snippet) = if let Some(text_edit) = item.text_edit.as_ref() {
        let (range, new_text) = match text_edit {
            CompletionTextEdit::Edit(edit) => (edit.range, edit.new_text.as_str()),
            CompletionTextEdit::InsertAndReplace(edit) => (edit.replace, edit.new_text.as_str()),
        };
        let start = position_to_offset_utf16(&updated, range.start);
        let end = position_to_offset_utf16(&updated, range.end);
        (
            start,
            end,
            new_text.to_owned(),
            item.insert_text_format == Some(lsp_types::InsertTextFormat::SNIPPET),
        )
    } else {
        let text = item
            .insert_text
            .clone()
            .unwrap_or_else(|| item.label.clone());
        let cursor_offset = position_to_offset_utf16(&updated, cursor);
        let (start, end) = infer_default_replace_range(&updated, cursor_offset);
        (
            start,
            end,
            text,
            item.insert_text_format == Some(lsp_types::InsertTextFormat::SNIPPET),
        )
    };

    let (inserted_text, caret_rel_offset) = if is_snippet {
        apply_snippet(&insert_text)
    } else {
        let end_offset = insert_text.len();
        (insert_text, end_offset)
    };

    updated.replace_range(start..end, &inserted_text);
    let caret_offset = start + caret_rel_offset;
    let caret_position = offset_to_position_utf16(&updated, caret_offset);

    (updated, caret_position)
}

fn infer_default_replace_range(text: &str, cursor_offset: usize) -> (usize, usize) {
    let mut start = cursor_offset.min(text.len());
    while start > 0 {
        let ch = text[..start].chars().next_back();
        let Some(ch) = ch else {
            break;
        };
        if !is_completion_word_char(ch) {
            break;
        }
        start -= ch.len_utf8();
    }

    let mut end = cursor_offset.min(text.len());
    while end < text.len() {
        let ch = text[end..].chars().next();
        let Some(ch) = ch else {
            break;
        };
        if !is_completion_word_char(ch) {
            break;
        }
        end += ch.len_utf8();
    }

    (start, end)
}

fn is_completion_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.'
}

fn apply_snippet(snippet: &str) -> (String, usize) {
    let bytes = snippet.as_bytes();
    let mut i = 0usize;
    let mut out = String::new();
    let mut tabstops: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();

    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                if i + 1 < bytes.len() {
                    out.push(bytes[i + 1] as char);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'$' => {
                if i + 1 >= bytes.len() {
                    out.push('$');
                    i += 1;
                    continue;
                }

                let next = bytes[i + 1];
                if next.is_ascii_digit() {
                    let (idx, next_i) = parse_number(bytes, i + 1);
                    tabstops.entry(idx).or_insert(out.len());
                    i = next_i;
                    continue;
                }

                if next == b'{' {
                    if let Some(close) = bytes[i + 2..].iter().position(|b| *b == b'}') {
                        let close_i = i + 2 + close;
                        let inner = &snippet[i + 2..close_i];
                        let (tab_idx, default_text) = parse_braced_snippet(inner);
                        tabstops.entry(tab_idx).or_insert(out.len());
                        out.push_str(&default_text);
                        i = close_i + 1;
                        continue;
                    }
                    out.push('$');
                    i += 1;
                    continue;
                }

                out.push('$');
                i += 1;
            }
            _ => {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
    }

    let caret = tabstops
        .iter()
        .find_map(|(idx, offset)| (*idx > 0).then_some(*offset))
        .or_else(|| tabstops.get(&0).copied())
        .unwrap_or(out.len());

    (out, caret)
}

fn parse_number(bytes: &[u8], mut i: usize) -> (u32, usize) {
    let mut value: u32 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        value = value
            .saturating_mul(10)
            .saturating_add((bytes[i] - b'0') as u32);
        i += 1;
    }
    (value, i)
}

fn parse_braced_snippet(inner: &str) -> (u32, String) {
    let bytes = inner.as_bytes();
    let (idx, i) = parse_number(bytes, 0);
    if i >= bytes.len() {
        return (idx, String::new());
    }

    match bytes[i] {
        b':' => (idx, unescape_snippet_text(&inner[i + 1..])),
        b'|' => {
            let rest = &inner[i + 1..];
            let content = rest.strip_suffix('|').unwrap_or(rest);
            let first = content.split(',').next().unwrap_or_default();
            (idx, unescape_snippet_text(first))
        }
        _ => (idx, String::new()),
    }
}

fn unescape_snippet_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            out.push(bytes[i + 1] as char);
            i += 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn position_to_offset_utf16(text: &str, position: Position) -> usize {
    let target_line = position.line as usize;
    let target_col = position.character;

    let mut line = 0usize;
    let mut col_utf16 = 0u32;

    for (byte_idx, ch) in text.char_indices() {
        if line == target_line && col_utf16 >= target_col {
            return byte_idx;
        }

        if ch == '\n' {
            if line == target_line {
                return byte_idx;
            }
            line += 1;
            col_utf16 = 0;
            continue;
        }

        if line == target_line {
            col_utf16 += ch.len_utf16() as u32;
            if col_utf16 >= target_col {
                return byte_idx + ch.len_utf8();
            }
        }
    }

    text.len()
}

fn offset_to_position_utf16(text: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut character = 0u32;

    for (byte_idx, ch) in text.char_indices() {
        if byte_idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }

    Position::new(line, character)
}

fn insert_caret_marker(text: &str, position: Position) -> String {
    let offset = position_to_offset_utf16(text, position).min(text.len());
    let mut out = String::with_capacity(text.len() + "<caret>".len());
    out.push_str(&text[..offset]);
    out.push_str("<caret>");
    out.push_str(&text[offset..]);
    out
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
            acton_config: None,
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
