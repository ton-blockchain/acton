use serde::Serialize;
use std::cell::RefCell;
use std::fmt::Write as _;
#[cfg(feature = "fift")]
use ton_language_server_core::languages::fift::FiftLanguage;
#[cfg(feature = "tasm")]
use ton_language_server_core::languages::tasm::TasmLanguage;
#[cfg(feature = "tolk")]
use ton_language_server_core::languages::tolk::TolkLanguage;
use ton_language_server_core::{
    CORE_TARGET, CodeLens, CompletionItem, CompletionItemKind, CompletionList, CompletionTrigger,
    CompletionTriggerKind, DocumentUri, FoldingRange, Hover, InlayHint, InlayHintKind,
    InsertTextFormat, LanguageId, LanguageService, Location, LogLevel, Position, ProfileSummary,
    Range, SEMANTIC_TOKEN_MODIFIER_NAMES, SEMANTIC_TOKEN_TYPE_NAMES, SemanticToken, SemanticTokens,
    WorkspaceConfig,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct TonLanguageServer {
    service: RefCell<LanguageService>,
}

#[wasm_bindgen]
impl TonLanguageServer {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        install_tree_sitter_allocator();
        install_logging();
        console_error_panic_hook::set_once();
        wasm_language_server(None).expect("default language server construction should not fail")
    }

    #[wasm_bindgen(js_name = withTasmSpec)]
    pub fn with_tasm_spec(spec_json: String) -> Result<Self, JsValue> {
        install_tree_sitter_allocator();
        install_logging();
        console_error_panic_hook::set_once();
        wasm_language_server(Some(&spec_json))
    }

    #[wasm_bindgen(js_name = addSourceFile)]
    pub fn add_source_file(&self, uri: String, text: String) -> Result<(), JsValue> {
        self.add_source_file_for_language("tolk".to_owned(), uri, text)
    }

    #[wasm_bindgen(js_name = addSourceFileForLanguage)]
    pub fn add_source_file_for_language(
        &self,
        language_id: String,
        uri: String,
        text: String,
    ) -> Result<(), JsValue> {
        self.service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .add_source_file(LanguageId::from(language_id), DocumentUri::from(uri), text)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = setWorkspaceConfigForLanguage)]
    pub fn set_workspace_config_for_language(
        &self,
        language_id: String,
        root_uri: String,
        manifest_uri: String,
        manifest_text: String,
    ) -> Result<(), JsValue> {
        let manifest_uri = if manifest_uri.is_empty() {
            None
        } else {
            Some(DocumentUri::from(manifest_uri))
        };
        self.service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .set_workspace_config(
                LanguageId::from(language_id),
                WorkspaceConfig::new(DocumentUri::from(root_uri), manifest_uri, manifest_text),
            )
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = setLogLevel)]
    pub fn set_log_level(&self, level: String) -> Result<(), JsValue> {
        let level = level.parse::<LogLevel>().map_err(js_error)?;
        wasm_logs().set_level(level);
        tracing::info!(
            target: CORE_TARGET,
            operation = "logging.set_level",
            level = level.as_str(),
            "log level updated"
        );
        Ok(())
    }

    #[wasm_bindgen(js_name = logs)]
    #[must_use]
    pub fn logs(&self) -> String {
        wasm_logs().render()
    }

    #[wasm_bindgen(js_name = clearLogs)]
    pub fn clear_logs(&self) {
        wasm_logs().clear();
    }

    #[wasm_bindgen(js_name = profileSummary)]
    pub fn profile_summary(&self) -> Result<String, JsValue> {
        let summary = self
            .service
            .try_borrow()
            .map_err(|_| language_server_busy())?
            .profiler()
            .summary()
            .clone();
        Ok(render_profile_summary(&summary))
    }

    #[wasm_bindgen(js_name = semanticTokenTypes)]
    pub fn semantic_token_types(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(SEMANTIC_TOKEN_TYPE_NAMES).map_err(js_error)
    }

    #[wasm_bindgen(js_name = semanticTokenModifiers)]
    pub fn semantic_token_modifiers(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(SEMANTIC_TOKEN_MODIFIER_NAMES).map_err(js_error)
    }

    #[wasm_bindgen(js_name = openDocument)]
    pub fn open_document(
        &self,
        uri: String,
        language_id: String,
        version: i32,
        text: String,
    ) -> Result<(), JsValue> {
        self.service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .open_document(
                DocumentUri::from(uri),
                LanguageId::from(language_id),
                version,
                text,
            )
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = changeDocument)]
    pub fn change_document(&self, uri: String, version: i32, text: String) -> Result<(), JsValue> {
        self.service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .change_document(&DocumentUri::from(uri), version, text)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = definition)]
    pub fn definition(&self, uri: String, line: u32, character: u32) -> Result<JsValue, JsValue> {
        let locations = self
            .service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .definition(&DocumentUri::from(uri), Position::new(line, character))
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&locations_to_lsp(locations)).map_err(js_error)
    }

    #[wasm_bindgen(js_name = references)]
    pub fn references(
        &self,
        uri: String,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<JsValue, JsValue> {
        let locations = self
            .service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .references(
                &DocumentUri::from(uri),
                Position::new(line, character),
                include_declaration,
            )
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&locations_to_lsp(locations)).map_err(js_error)
    }

    #[wasm_bindgen(js_name = hover)]
    pub fn hover(&self, uri: String, line: u32, character: u32) -> Result<JsValue, JsValue> {
        let hover = self
            .service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .hover(&DocumentUri::from(uri), Position::new(line, character))
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&hover.map(hover_to_lsp)).map_err(js_error)
    }

    #[wasm_bindgen(js_name = completion)]
    pub fn completion(
        &self,
        uri: String,
        line: u32,
        character: u32,
        trigger_kind: u32,
        trigger_character: String,
    ) -> Result<JsValue, JsValue> {
        let trigger = CompletionTrigger {
            kind: match trigger_kind {
                2 => CompletionTriggerKind::TriggerCharacter,
                3 => CompletionTriggerKind::TriggerForIncompleteCompletions,
                _ => CompletionTriggerKind::Invoked,
            },
            character: (!trigger_character.is_empty()).then_some(trigger_character),
        };
        let completion = self
            .service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .completion(
                &DocumentUri::from(uri),
                Position::new(line, character),
                trigger,
            )
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&completion_list_to_lsp(completion)).map_err(js_error)
    }

    #[wasm_bindgen(js_name = semanticTokens)]
    pub fn semantic_tokens(&self, uri: String) -> Result<JsValue, JsValue> {
        let tokens = self
            .service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .semantic_tokens(&DocumentUri::from(uri))
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&semantic_tokens_to_lsp(tokens)).map_err(js_error)
    }

    #[wasm_bindgen(js_name = inlayHints)]
    pub fn inlay_hints(
        &self,
        uri: String,
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    ) -> Result<JsValue, JsValue> {
        let hints = self
            .service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .inlay_hints(
                &DocumentUri::from(uri),
                Range::new(
                    Position::new(start_line, start_character),
                    Position::new(end_line, end_character),
                ),
            )
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&inlay_hints_to_lsp(hints)).map_err(js_error)
    }

    #[wasm_bindgen(js_name = codeLens)]
    pub fn code_lens(&self, uri: String) -> Result<JsValue, JsValue> {
        let lenses = self
            .service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .code_lens(&DocumentUri::from(uri))
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&code_lenses_to_lsp(lenses)).map_err(js_error)
    }

    #[wasm_bindgen(js_name = foldingRanges)]
    pub fn folding_ranges(&self, uri: String) -> Result<JsValue, JsValue> {
        let ranges = self
            .service
            .try_borrow_mut()
            .map_err(|_| language_server_busy())?
            .folding_ranges(&DocumentUri::from(uri))
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&folding_ranges_to_lsp(ranges)).map_err(js_error)
    }
}

impl Default for TonLanguageServer {
    fn default() -> Self {
        Self::new()
    }
}

fn wasm_language_server(tasm_spec_json: Option<&str>) -> Result<TonLanguageServer, JsValue> {
    let mut service = LanguageService::new(ton_language_server_core::LanguageServiceConfig {
        enable_profiling: true,
    });

    #[cfg(feature = "tlb")]
    service.register_language(ton_language_server_core::languages::tlb::TlbLanguage::new());

    #[cfg(feature = "tasm")]
    service.register_language(if let Some(spec_json) = tasm_spec_json {
        TasmLanguage::with_spec_json(spec_json).map_err(js_error)?
    } else {
        TasmLanguage::new()
    });
    #[cfg(not(feature = "tasm"))]
    let _ = tasm_spec_json;

    #[cfg(feature = "fift")]
    service.register_language(FiftLanguage::new());

    #[cfg(feature = "tolk")]
    service.register_language(TolkLanguage::new());

    Ok(TonLanguageServer {
        service: RefCell::new(service),
    })
}

#[derive(Serialize)]
struct LspHover {
    contents: LspMarkupContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<LspRange>,
}

#[derive(Serialize)]
struct LspMarkupContent {
    kind: &'static str,
    value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LspCompletionList {
    is_incomplete: bool,
    items: Vec<LspCompletionItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LspCompletionItem {
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    documentation: Option<LspMarkupContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    insert_text: Option<String>,
    insert_text_format: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_edit: Option<LspTextEdit>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    additional_text_edits: Vec<LspTextEdit>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LspTextEdit {
    range: LspRange,
    new_text: String,
}

#[derive(Serialize)]
struct LspLocation {
    uri: String,
    range: LspRange,
}

#[derive(Serialize)]
struct LspCodeLens {
    range: LspRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<LspCommand>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LspSemanticTokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    result_id: Option<String>,
    data: Vec<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LspInlayHint {
    position: LspPosition,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tooltip: Option<String>,
    padding_left: bool,
    padding_right: bool,
}

#[derive(Serialize)]
struct LspCommand {
    title: String,
    command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LspFoldingRange {
    start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_character: Option<u32>,
    end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_character: Option<u32>,
}

#[derive(Serialize)]
struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

#[derive(Serialize)]
struct LspPosition {
    line: u32,
    character: u32,
}

fn locations_to_lsp(locations: Vec<Location>) -> Vec<LspLocation> {
    locations
        .into_iter()
        .map(|location| LspLocation {
            uri: location.uri.as_str().to_owned(),
            range: LspRange {
                start: position_to_lsp(location.range.start),
                end: position_to_lsp(location.range.end),
            },
        })
        .collect()
}

fn hover_to_lsp(hover: Hover) -> LspHover {
    LspHover {
        contents: LspMarkupContent {
            kind: "markdown",
            value: hover.contents,
        },
        range: hover.range.map(range_to_lsp),
    }
}

fn completion_list_to_lsp(completion: CompletionList) -> LspCompletionList {
    LspCompletionList {
        is_incomplete: completion.is_incomplete,
        items: completion
            .items
            .into_iter()
            .map(completion_item_to_lsp)
            .collect(),
    }
}

fn completion_item_to_lsp(item: CompletionItem) -> LspCompletionItem {
    LspCompletionItem {
        label: item.label,
        kind: item.kind.map(completion_item_kind_to_lsp),
        detail: item.detail,
        documentation: item.documentation.map(|value| LspMarkupContent {
            kind: "markdown",
            value,
        }),
        deprecated: item.deprecated.then_some(true),
        sort_text: item.sort_text,
        filter_text: item.filter_text,
        insert_text: item.insert_text,
        insert_text_format: match item.insert_text_format {
            InsertTextFormat::PlainText => 1,
            InsertTextFormat::Snippet => 2,
        },
        text_edit: item.text_edit.map(|edit| LspTextEdit {
            range: range_to_lsp(edit.range),
            new_text: edit.new_text,
        }),
        additional_text_edits: item
            .additional_text_edits
            .into_iter()
            .map(|edit| LspTextEdit {
                range: range_to_lsp(edit.range),
                new_text: edit.new_text,
            })
            .collect(),
    }
}

const fn completion_item_kind_to_lsp(kind: CompletionItemKind) -> u8 {
    match kind {
        CompletionItemKind::Text => 1,
        CompletionItemKind::Method => 2,
        CompletionItemKind::Function => 3,
        CompletionItemKind::Constructor => 4,
        CompletionItemKind::Field => 5,
        CompletionItemKind::Variable => 6,
        CompletionItemKind::Class => 7,
        CompletionItemKind::Interface => 8,
        CompletionItemKind::Module => 9,
        CompletionItemKind::Property => 10,
        CompletionItemKind::Unit => 11,
        CompletionItemKind::Value => 12,
        CompletionItemKind::Enum => 13,
        CompletionItemKind::Keyword => 14,
        CompletionItemKind::Snippet => 15,
        CompletionItemKind::Color => 16,
        CompletionItemKind::File => 17,
        CompletionItemKind::Reference => 18,
        CompletionItemKind::Folder => 19,
        CompletionItemKind::EnumMember => 20,
        CompletionItemKind::Constant => 21,
        CompletionItemKind::Struct => 22,
        CompletionItemKind::Event => 23,
        CompletionItemKind::Operator => 24,
        CompletionItemKind::TypeParameter => 25,
    }
}

fn code_lenses_to_lsp(lenses: Vec<CodeLens>) -> Vec<LspCodeLens> {
    lenses.into_iter().map(code_lens_to_lsp).collect()
}

fn semantic_tokens_to_lsp(tokens: SemanticTokens) -> LspSemanticTokens {
    LspSemanticTokens {
        result_id: tokens.result_id,
        data: flatten_semantic_tokens(tokens.data),
    }
}

fn inlay_hints_to_lsp(hints: Vec<InlayHint>) -> Vec<LspInlayHint> {
    hints
        .into_iter()
        .map(|hint| LspInlayHint {
            position: position_to_lsp(hint.position),
            label: hint.label,
            kind: hint.kind.map(|kind| match kind {
                InlayHintKind::Type => 1,
                InlayHintKind::Parameter => 2,
            }),
            tooltip: hint.tooltip,
            padding_left: hint.padding_left,
            padding_right: hint.padding_right,
        })
        .collect()
}

fn flatten_semantic_tokens(tokens: Vec<SemanticToken>) -> Vec<u32> {
    let mut data = Vec::with_capacity(tokens.len() * 5);
    for token in tokens {
        data.push(token.delta_line);
        data.push(token.delta_start);
        data.push(token.length);
        data.push(token.token_type);
        data.push(token.token_modifiers_bitset);
    }
    data
}

fn code_lens_to_lsp(lens: CodeLens) -> LspCodeLens {
    LspCodeLens {
        range: range_to_lsp(lens.range),
        command: lens.command.map(|command| LspCommand {
            title: command.title,
            command: command.command,
            arguments: command.arguments,
        }),
    }
}

fn folding_ranges_to_lsp(ranges: Vec<FoldingRange>) -> Vec<LspFoldingRange> {
    ranges
        .into_iter()
        .map(|range| LspFoldingRange {
            start_line: range.start_line,
            start_character: range.start_character,
            end_line: range.end_line,
            end_character: range.end_character,
        })
        .collect()
}

const fn range_to_lsp(range: Range) -> LspRange {
    LspRange {
        start: position_to_lsp(range.start),
        end: position_to_lsp(range.end),
    }
}

const fn position_to_lsp(position: Position) -> LspPosition {
    LspPosition {
        line: position.line,
        character: position.character,
    }
}

fn js_error(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn language_server_busy() -> JsValue {
    JsValue::from_str("language server is busy")
}

fn render_profile_summary(summary: &ProfileSummary) -> String {
    if summary.events.is_empty() && summary.counters.is_empty() {
        return "No profiling data".to_owned();
    }

    let mut output = String::new();
    if !summary.counters.is_empty() {
        output.push_str("Counters\n");
        for (name, count) in &summary.counters {
            output.push_str("  ");
            output.push_str(name);
            output.push_str(": ");
            output.push_str(&count.to_string());
            output.push('\n');
        }
    }

    if !summary.events.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("Spans\n");
        let mut spans = std::collections::BTreeMap::<&'static str, (usize, f64)>::new();
        for event in &summary.events {
            let entry = spans.entry(event.name).or_default();
            entry.0 += 1;
            entry.1 += event.elapsed.as_secs_f64() * 1000.0;
        }
        for (name, (count, total_ms)) in spans {
            let average_ms = total_ms / count as f64;
            output.push_str("  ");
            output.push_str(name);
            output.push_str(": count=");
            output.push_str(&count.to_string());
            output.push_str(" total=");
            push_ms(&mut output, total_ms);
            output.push_str(" avg=");
            push_ms(&mut output, average_ms);
            output.push('\n');
        }
    }

    output
}

fn push_ms(output: &mut String, milliseconds: f64) {
    let _ = write!(output, "{milliseconds:.3}ms");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn completion_serializes_as_lsp_json() {
        let mut item = CompletionItem::new("save", CompletionItemKind::Method)
            .with_detail("fun Storage.save(self)")
            .with_documentation("Saves storage.")
            .with_filter_text("save")
            .with_snippet_replacement(
                Range::new(Position::new(3, 8), Position::new(3, 10)),
                "save(${1:value})$0",
            );
        item.deprecated = true;
        item.sort_text = Some("001-save".to_owned());

        let value = serde_json::to_value(completion_list_to_lsp(CompletionList {
            is_incomplete: true,
            items: vec![item],
        }))
        .expect("LSP completion payload should be serializable");

        assert_eq!(
            value,
            json!({
                "isIncomplete": true,
                "items": [{
                    "label": "save",
                    "kind": 2,
                    "detail": "fun Storage.save(self)",
                    "documentation": {
                        "kind": "markdown",
                        "value": "Saves storage."
                    },
                    "deprecated": true,
                    "sortText": "001-save",
                    "filterText": "save",
                    "insertText": "save(${1:value})$0",
                    "insertTextFormat": 2,
                    "textEdit": {
                        "range": {
                            "start": { "line": 3, "character": 8 },
                            "end": { "line": 3, "character": 10 }
                        },
                        "newText": "save(${1:value})$0"
                    }
                }]
            })
        );
    }
}

fn install_logging() {
    let _ = tracing::subscriber::set_global_default(WasmLogSubscriber::new(wasm_logs().clone()));
}

fn wasm_logs() -> &'static std::sync::Arc<WasmLogState> {
    static LOGS: std::sync::OnceLock<std::sync::Arc<WasmLogState>> = std::sync::OnceLock::new();
    LOGS.get_or_init(|| std::sync::Arc::new(WasmLogState::default()))
}

#[derive(Debug)]
struct WasmLogState {
    level: std::sync::atomic::AtomicUsize,
    next_event_id: std::sync::atomic::AtomicU64,
    lines: std::sync::Mutex<Vec<String>>,
}

impl Default for WasmLogState {
    fn default() -> Self {
        Self {
            level: std::sync::atomic::AtomicUsize::new(encode_log_level(LogLevel::Info)),
            next_event_id: std::sync::atomic::AtomicU64::new(0),
            lines: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl WasmLogState {
    fn set_level(&self, level: LogLevel) {
        self.level.store(
            encode_log_level(level),
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    fn enabled(&self, level: tracing::Level) -> bool {
        let configured = self.level.load(std::sync::atomic::Ordering::Relaxed);
        configured != 0 && encode_tracing_level(level) <= configured
    }

    fn push(&self, line: String) {
        const MAX_LOG_LINES: usize = 2_000;

        let mut lines = self
            .lines
            .lock()
            .expect("WASM log buffer should not be poisoned");
        lines.push(line);
        let overflow = lines.len().saturating_sub(MAX_LOG_LINES);
        if overflow > 0 {
            lines.drain(..overflow);
        }
    }

    fn next_event_id(&self) -> u64 {
        self.next_event_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    fn render(&self) -> String {
        self.lines
            .lock()
            .expect("WASM log buffer should not be poisoned")
            .join("\n")
    }

    fn clear(&self) {
        self.lines
            .lock()
            .expect("WASM log buffer should not be poisoned")
            .clear();
    }
}

struct WasmLogSubscriber {
    logs: std::sync::Arc<WasmLogState>,
    next_span_id: std::sync::atomic::AtomicU64,
}

impl WasmLogSubscriber {
    const fn new(logs: std::sync::Arc<WasmLogState>) -> Self {
        Self {
            logs,
            next_span_id: std::sync::atomic::AtomicU64::new(1),
        }
    }
}

impl tracing::Subscriber for WasmLogSubscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target().starts_with(CORE_TARGET) && self.logs.enabled(*metadata.level())
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(
            self.next_span_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        )
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        if !self.enabled(event.metadata()) {
            return;
        }

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.logs.push(render_log_event(
            self.logs.next_event_id(),
            event.metadata(),
            &visitor.fields,
        ));
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}

    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(tracing::level_filters::LevelFilter::TRACE)
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: std::collections::BTreeMap<String, String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}

fn render_log_event(
    event_id: u64,
    metadata: &tracing::Metadata<'_>,
    fields: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut output = format!(
        "{event_id:04} {} {}",
        render_tracing_level(*metadata.level()),
        metadata.target()
    );
    if let Some(operation) = fields.get("operation") {
        output.push(' ');
        output.push_str(operation);
    }
    if let Some(message) = fields.get("message") {
        output.push(' ');
        output.push_str(message.trim_matches('"'));
    }
    for (name, value) in fields {
        if name == "message" || name == "operation" {
            continue;
        }
        output.push('\n');
        output.push_str("  ");
        output.push_str(name);
        output.push_str(": ");
        output.push_str(&redact_field(name, value));
    }
    output
}

fn redact_field(name: &str, value: &str) -> String {
    if is_path_like_field(name) {
        redact_path_like_value(value)
    } else {
        value.to_owned()
    }
}

fn is_path_like_field(name: &str) -> bool {
    matches!(name, "uri" | "path" | "root" | "workspace_root")
        || name.ends_with("_uri")
        || name.ends_with("_path")
        || name.ends_with("_root")
}

fn redact_path_like_value(value: &str) -> String {
    if let Some(path) = value.strip_prefix("file://") {
        let file_name = path.rsplit('/').next().unwrap_or("<unknown>");
        format!("file://<redacted>/{file_name}")
    } else if value.starts_with('/') {
        let file_name = value.rsplit('/').next().unwrap_or("<unknown>");
        format!("<redacted>/{file_name}")
    } else {
        value.to_owned()
    }
}

const fn encode_log_level(level: LogLevel) -> usize {
    match level {
        LogLevel::Off => 0,
        LogLevel::Error => 1,
        LogLevel::Warn => 2,
        LogLevel::Info => 3,
        LogLevel::Debug => 4,
        LogLevel::Trace => 5,
    }
}

const fn encode_tracing_level(level: tracing::Level) -> usize {
    match level {
        tracing::Level::ERROR => 1,
        tracing::Level::WARN => 2,
        tracing::Level::INFO => 3,
        tracing::Level::DEBUG => 4,
        tracing::Level::TRACE => 5,
    }
}

const fn render_tracing_level(level: tracing::Level) -> &'static str {
    match level {
        tracing::Level::ERROR => "error",
        tracing::Level::WARN => "warn",
        tracing::Level::INFO => "info",
        tracing::Level::DEBUG => "debug",
        tracing::Level::TRACE => "trace",
    }
}

#[cfg(target_arch = "wasm32")]
fn install_tree_sitter_allocator() {
    tree_sitter_allocator::install();
}

#[cfg(not(target_arch = "wasm32"))]
const fn install_tree_sitter_allocator() {}

#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
mod tree_sitter_allocator {
    use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr;
    use std::sync::Once;

    const ALLOCATION_ALIGN: usize = 16;
    const HEADER_SIZE: usize = size_of::<AllocationHeader>();

    static INSTALL: Once = Once::new();

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct AllocationHeader {
        layout_size: usize,
        offset: usize,
    }

    pub(crate) fn install() {
        INSTALL.call_once(|| {
            // SAFETY: Tree-sitter stores these process-global callbacks and calls them with
            // ordinary C allocator semantics. The functions below return pointers allocated by
            // Rust's global allocator and retain enough header metadata to free them with the
            // same allocator.
            unsafe {
                tree_sitter::set_allocator(Some(malloc), Some(calloc), Some(realloc), Some(free));
            }
        });
    }

    unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
        // SAFETY: `allocate` implements C `malloc` semantics for arbitrary sizes.
        unsafe { allocate(size, false) }
    }

    unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
        let Some(total_size) = count.checked_mul(size) else {
            return ptr::null_mut();
        };

        // SAFETY: `allocate` implements C `calloc` semantics when `zeroed` is true.
        unsafe { allocate(total_size, true) }
    }

    unsafe extern "C" fn realloc(ptr: *mut c_void, new_size: usize) -> *mut c_void {
        if ptr.is_null() {
            // SAFETY: null `realloc` is equivalent to `malloc`.
            return unsafe { allocate(new_size, false) };
        }
        if new_size == 0 {
            // SAFETY: zero-sized `realloc` frees the original pointer.
            unsafe { free(ptr) };
            return ptr::null_mut();
        }

        // SAFETY: `ptr` was allocated by this allocator because Tree-sitter receives these
        // callbacks as a matched allocator set.
        let header = unsafe { read_header(ptr) };
        // SAFETY: Allocate a new block before freeing the old one, preserving C `realloc`
        // behavior on allocation failure.
        let new_ptr = unsafe { allocate(new_size, false) };
        if new_ptr.is_null() {
            return ptr::null_mut();
        }

        // SAFETY: Both blocks are valid and non-overlapping allocations from this allocator.
        unsafe {
            ptr::copy_nonoverlapping(
                ptr.cast::<u8>(),
                new_ptr.cast::<u8>(),
                header.requested_size().min(new_size),
            );
            free(ptr);
        }

        new_ptr
    }

    unsafe extern "C" fn free(ptr: *mut c_void) {
        if ptr.is_null() {
            return;
        }

        // SAFETY: `ptr` was returned by `allocate`; the header records the original allocation.
        let header = unsafe { read_header(ptr) };
        let user_ptr = ptr.cast::<u8>();
        // SAFETY: `offset` was computed from `base` to `user_ptr` in `allocate`.
        let base_ptr = unsafe { user_ptr.sub(header.offset) };
        // SAFETY: the layout size was accepted by `allocate` and persisted in the header.
        let layout = unsafe { layout_from_size_unchecked(header.layout_size) };
        // SAFETY: `base_ptr` and `layout` match the allocation created in `allocate`.
        unsafe { dealloc(base_ptr, layout) };
    }

    unsafe fn allocate(size: usize, zeroed: bool) -> *mut c_void {
        if size == 0 {
            return ptr::null_mut();
        }

        let Some(layout_size) = size
            .checked_add(HEADER_SIZE)
            .and_then(|size| size.checked_add(ALLOCATION_ALIGN - 1))
        else {
            return ptr::null_mut();
        };

        let Ok(layout) = Layout::from_size_align(layout_size, ALLOCATION_ALIGN) else {
            return ptr::null_mut();
        };
        let base_ptr = if zeroed {
            // SAFETY: `layout` is valid and non-zero sized.
            unsafe { alloc_zeroed(layout) }
        } else {
            // SAFETY: `layout` is valid and non-zero sized.
            unsafe { alloc(layout) }
        };
        if base_ptr.is_null() {
            return ptr::null_mut();
        }

        let unaligned_user_addr = base_ptr as usize + HEADER_SIZE;
        let user_addr = (unaligned_user_addr + ALLOCATION_ALIGN - 1) & !(ALLOCATION_ALIGN - 1);
        let user_ptr = user_addr as *mut u8;
        let offset = user_addr - base_ptr as usize;
        let header_ptr = user_ptr
            .wrapping_sub(HEADER_SIZE)
            .cast::<AllocationHeader>();

        // SAFETY: `header_ptr` points into the allocation immediately before `user_ptr`.
        unsafe {
            header_ptr.write(AllocationHeader {
                layout_size,
                offset,
            });
        }

        user_ptr.cast()
    }

    const unsafe fn read_header(ptr: *mut c_void) -> AllocationHeader {
        let header_ptr = ptr
            .cast::<u8>()
            .wrapping_sub(HEADER_SIZE)
            .cast::<AllocationHeader>();
        // SAFETY: All allocator entry points only receive pointers previously returned by
        // `allocate`, so the header immediately before the user pointer is initialized.
        unsafe { header_ptr.read() }
    }

    const unsafe fn layout_from_size_unchecked(size: usize) -> Layout {
        // SAFETY: callers only pass sizes that were returned by `Layout::from_size_align`.
        unsafe { Layout::from_size_align_unchecked(size, ALLOCATION_ALIGN) }
    }

    impl AllocationHeader {
        const fn requested_size(self) -> usize {
            self.layout_size - HEADER_SIZE - (ALLOCATION_ALIGN - 1)
        }
    }
}
