use crate::profiling::Profiler;
use crate::types::{
    CodeLens, DocumentSnapshot, DocumentUri, FoldingRange, Hover, Location, Position,
};
use std::any::Any;
use tree_sitter::Tree;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeatureSet {
    pub definition: bool,
    pub document_symbols: bool,
    pub diagnostics: bool,
    pub references: bool,
    pub hover: bool,
    pub code_lens: bool,
    pub folding_ranges: bool,
    pub completion: bool,
    pub semantic_tokens: bool,
}

pub trait ParsedDocument: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn tree(&self) -> &Tree;
}

pub trait WorkspaceLanguage: Send + Sync {
    fn did_open(
        &self,
        document: &DocumentSnapshot,
        parsed: &dyn ParsedDocument,
        profiler: &mut Profiler,
    ) -> anyhow::Result<()>;

    fn did_change(
        &self,
        document: &DocumentSnapshot,
        parsed: &dyn ParsedDocument,
        profiler: &mut Profiler,
    ) -> anyhow::Result<()>;

    fn did_close(&self, uri: &DocumentUri);
}

pub struct PluginContext<'a> {
    pub document: &'a DocumentSnapshot,
    pub parsed: &'a dyn ParsedDocument,
    pub profiler: &'a mut Profiler,
}

pub struct ParseRequest<'a> {
    pub document: &'a DocumentSnapshot,
    pub old_tree: Option<&'a Tree>,
    pub profiler: &'a mut Profiler,
}

pub struct DefinitionRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
}

pub struct HoverRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
}

pub struct CodeLensRequest<'a> {
    pub context: PluginContext<'a>,
}

pub struct FoldingRangeRequest<'a> {
    pub context: PluginContext<'a>,
}

pub trait LanguagePlugin: Send + Sync {
    fn language_id(&self) -> crate::LanguageId;

    fn file_extensions(&self) -> &'static [&'static str];

    fn capabilities(&self) -> FeatureSet;

    fn workspace(&self) -> Option<&dyn WorkspaceLanguage> {
        None
    }

    fn parse(&self, request: ParseRequest<'_>) -> anyhow::Result<Box<dyn ParsedDocument>>;

    fn definition(&self, _request: DefinitionRequest<'_>) -> anyhow::Result<Vec<Location>> {
        Ok(Vec::new())
    }

    fn hover(&self, _request: HoverRequest<'_>) -> anyhow::Result<Option<Hover>> {
        Ok(None)
    }

    fn code_lens(&self, _request: CodeLensRequest<'_>) -> anyhow::Result<Vec<CodeLens>> {
        Ok(Vec::new())
    }

    fn folding_ranges(
        &self,
        _request: FoldingRangeRequest<'_>,
    ) -> anyhow::Result<Vec<FoldingRange>> {
        Ok(Vec::new())
    }
}
