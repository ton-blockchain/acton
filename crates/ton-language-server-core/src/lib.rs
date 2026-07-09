mod language;
mod logging;
mod profiling;
mod service;
mod text;
mod types;

pub mod languages;

pub use language::{
    CodeLensRequest, DefinitionRequest, FeatureSet, FoldingRangeRequest, HoverRequest,
    LanguagePlugin, ParseRequest, ParsedDocument, PluginContext, WorkspaceLanguage,
};
pub use logging::{
    CORE_TARGET, EDIT_TARGET, FIFT_TARGET, LogLevel, LoggingConfig, ParseLogLevelError,
    SERVICE_TARGET, TASM_TARGET, TLB_TARGET, TOLK_TARGET,
};
pub use profiling::{ProfileEvent, ProfileSummary, Profiler};
pub use service::{LanguageService, LanguageServiceConfig};
pub use text::TextIndex;
pub use types::{
    CodeLens, Command, DocumentSnapshot, DocumentUri, FoldingRange, Hover, LanguageId, Location,
    Position, Range, TextEdit,
};

#[must_use]
pub fn default_language_service() -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(languages::tlb::TlbLanguage::new());
    service.register_language(languages::tasm::TasmLanguage::new());
    service.register_language(languages::fift::FiftLanguage::new());
    service.register_language(languages::tolk::TolkLanguage::new());
    service
}
