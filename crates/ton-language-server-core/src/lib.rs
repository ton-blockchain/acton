mod completion;
mod language;
mod logging;
mod profiling;
mod semantic_tokens;
mod service;
mod text;
mod types;

pub mod languages;

pub use completion::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionTrigger, CompletionTriggerKind,
    InsertTextFormat,
};
pub use language::{
    CodeActionRequest, CodeLensRequest, CompletionRequest, DefinitionRequest,
    DocumentHighlightRequest, DocumentSymbolRequest, FeatureSet, FileRenameRequest,
    FoldingRangeRequest, HoverRequest, InlayHintRequest, LanguagePlugin, ParseRequest,
    ParsedDocument, PluginContext, PrepareRenameRequest, RenameRequest, SemanticTokensRequest,
    SignatureHelpRequest, TypeDefinitionRequest, WorkspaceLanguage, WorkspaceSymbolRequest,
};
pub use logging::{
    CORE_TARGET, EDIT_TARGET, FIFT_TARGET, LogLevel, LoggingConfig, ParseLogLevelError,
    SERVICE_TARGET, TASM_TARGET, TLB_TARGET, TOLK_TARGET,
};
pub use profiling::{ProfileEvent, ProfileSummary, Profiler};
pub use semantic_tokens::{
    SEMANTIC_TOKEN_MODIFIER_NAMES, SEMANTIC_TOKEN_TYPE_NAMES, SemanticToken, SemanticTokenModifier,
    SemanticTokenType, SemanticTokens, SemanticTokensBuilder,
};
pub use service::{LanguageService, LanguageServiceConfig};
pub use text::TextIndex;
pub use types::{
    CodeAction, CodeActionKind, CodeLens, Command, DocumentEdits, DocumentHighlight,
    DocumentHighlightKind, DocumentSnapshot, DocumentSymbol, DocumentSymbolKind, DocumentUri,
    FileRename, FoldingRange, Hover, InlayHint, InlayHintKind, LanguageId, Location,
    ParameterInformation, Position, PrepareRename, Range, SignatureHelp, SignatureInformation,
    TextEdit, WorkspaceConfig, WorkspaceEdit, WorkspaceSymbol,
};

#[must_use]
pub fn default_language_service() -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    #[cfg(feature = "tlb")]
    service.register_language(languages::tlb::TlbLanguage::new());
    #[cfg(feature = "tasm")]
    service.register_language(languages::tasm::TasmLanguage::new());
    #[cfg(feature = "fift")]
    service.register_language(languages::fift::FiftLanguage::new());
    #[cfg(feature = "tolk")]
    service.register_language(languages::tolk::TolkLanguage::new());
    service
}
