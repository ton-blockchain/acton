mod completion;
mod hover;
mod schema;

use crate::language::{
    CompletionRequest, FeatureSet, HoverRequest, LanguagePlugin, ParseRequest, ParsedDocument,
};
use crate::{LanguageId, logging};
use anyhow::Context;
use std::any::Any;
use tree_sitter::Tree;

pub const LANGUAGE_ID: &str = "toml";

#[derive(Clone, Copy, Debug, Default)]
pub struct TomlLanguage;

impl TomlLanguage {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl LanguagePlugin for TomlLanguage {
    fn language_id(&self) -> LanguageId {
        LanguageId::from(LANGUAGE_ID)
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }

    fn capabilities(&self) -> FeatureSet {
        FeatureSet {
            completion: true,
            hover: true,
            ..FeatureSet::default()
        }
    }

    fn parse(&self, request: ParseRequest<'_>) -> anyhow::Result<Box<dyn ParsedDocument>> {
        tracing::debug!(
            target: logging::TOML_TARGET,
            operation = "toml.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            text_len = request.document.text().len(),
            "parsing TOML document"
        );
        let started_at = request.profiler.start();
        let source_file =
            toml_syntax::parse_with_old_tree(request.document.text(), request.old_tree)?;
        request.profiler.finish("toml.parse", started_at);

        Ok(Box::new(TomlParsedDocument { source_file }))
    }

    fn hover(&self, request: HoverRequest<'_>) -> anyhow::Result<Option<crate::Hover>> {
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TomlParsedDocument>()
            .context("TOML parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let result = hover::hover(request.context.document, parsed, request.position);
        request.context.profiler.finish("toml.hover", started_at);

        Ok(result)
    }

    fn completion(&self, request: CompletionRequest<'_>) -> anyhow::Result<crate::CompletionList> {
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TomlParsedDocument>()
            .context("TOML parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let result = completion::completion(request.context.document, parsed, request.position);
        request
            .context
            .profiler
            .finish("toml.completion", started_at);

        Ok(result)
    }
}

#[derive(Debug)]
struct TomlParsedDocument {
    source_file: toml_syntax::SourceFile,
}

impl ParsedDocument for TomlParsedDocument {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn tree(&self) -> &Tree {
        &self.source_file.tree
    }
}
