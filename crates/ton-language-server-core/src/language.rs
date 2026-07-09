use crate::profiling::Profiler;
use crate::types::{DocumentSnapshot, Location, Position};
use std::any::Any;
use tree_sitter::Tree;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeatureSet {
    pub definition: bool,
    pub document_symbols: bool,
    pub diagnostics: bool,
    pub references: bool,
    pub hover: bool,
    pub completion: bool,
    pub semantic_tokens: bool,
}

pub trait ParsedDocument: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn tree(&self) -> &Tree;
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

pub trait LanguagePlugin: Send + Sync {
    fn language_id(&self) -> crate::LanguageId;

    fn file_extensions(&self) -> &'static [&'static str];

    fn capabilities(&self) -> FeatureSet;

    fn parse(&self, request: ParseRequest<'_>) -> anyhow::Result<Box<dyn ParsedDocument>>;

    fn definition(&self, _request: DefinitionRequest<'_>) -> anyhow::Result<Vec<Location>> {
        Ok(Vec::new())
    }
}
