mod completion;
mod index;
mod psi;
mod reference;
mod semantic_tokens;

use crate::language::{
    CompletionRequest, DefinitionRequest, FeatureSet, LanguagePlugin, ParseRequest, ParsedDocument,
    SemanticTokensRequest,
};
use crate::logging;
use crate::{LanguageId, Location};
use anyhow::Context;
use index::TlbSymbolIndex;
use std::any::Any;
use tree_sitter::Tree;

pub const LANGUAGE_ID: &str = "tlb";

#[derive(Debug, Default)]
pub struct TlbLanguage;

impl TlbLanguage {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl LanguagePlugin for TlbLanguage {
    fn language_id(&self) -> LanguageId {
        LanguageId::from(LANGUAGE_ID)
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["tlb"]
    }

    fn capabilities(&self) -> FeatureSet {
        FeatureSet {
            definition: true,
            completion: true,
            semantic_tokens: true,
            ..FeatureSet::default()
        }
    }

    fn parse(&self, request: ParseRequest<'_>) -> anyhow::Result<Box<dyn ParsedDocument>> {
        tracing::debug!(
            target: logging::TLB_TARGET,
            operation = "tlb.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            text_len = request.document.text().len(),
            "parsing TL-B document"
        );
        let parse_started_at = request.profiler.start();
        let source_file =
            match tlb_syntax::parse_with_old_tree(request.document.text(), request.old_tree) {
                Ok(source_file) => source_file,
                Err(error) => {
                    tracing::debug!(
                        target: logging::TLB_TARGET,
                        operation = "tlb.parse",
                        uri = request.document.uri().as_str(),
                        version = request.document.version(),
                        incremental = request.old_tree.is_some(),
                        error = %error,
                        "TL-B parse failed"
                    );
                    return Err(error);
                }
            };
        request.profiler.finish("tlb.parse", parse_started_at);
        tracing::debug!(
            target: logging::TLB_TARGET,
            operation = "tlb.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            has_error = source_file.tree.root_node().has_error(),
            "parsed TL-B document"
        );

        let index_started_at = request.profiler.start();
        let index = TlbSymbolIndex::build(&source_file);
        request.profiler.finish("tlb.index", index_started_at);
        tracing::debug!(
            target: logging::TLB_TARGET,
            operation = "tlb.index.rebuilt",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            declaration_count = index.declaration_count(),
            "rebuilt TL-B symbol index"
        );

        Ok(Box::new(TlbParsedDocument { source_file, index }))
    }

    fn definition(&self, request: DefinitionRequest<'_>) -> anyhow::Result<Vec<Location>> {
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TlbParsedDocument>()
            .context("TL-B parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let psi_file = psi::TlbPsiFile::new(request.context.document, parsed);
        let ranges = psi_file.definition_ranges_at(request.position);
        request
            .context
            .profiler
            .finish("tlb.definition.resolve", started_at);
        tracing::debug!(
            target: logging::TLB_TARGET,
            operation = "tlb.definition.resolve",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            line = request.position.line,
            character = request.position.character,
            result_count = ranges.len(),
            "resolved TL-B definition"
        );

        Ok(ranges
            .into_iter()
            .map(|range| Location::new(request.context.document.uri().clone(), range))
            .collect())
    }

    fn semantic_tokens(
        &self,
        request: SemanticTokensRequest<'_>,
    ) -> anyhow::Result<Vec<crate::SemanticToken>> {
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TlbParsedDocument>()
            .context("TL-B parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let tokens = semantic_tokens::semantic_tokens(request.context.document, parsed);
        request
            .context
            .profiler
            .finish("tlb.semantic_tokens", started_at);
        tracing::debug!(
            target: logging::TLB_TARGET,
            operation = "tlb.semantic_tokens",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            result_count = tokens.len(),
            "resolved TL-B semantic tokens"
        );
        Ok(tokens)
    }

    fn completion(&self, request: CompletionRequest<'_>) -> anyhow::Result<crate::CompletionList> {
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TlbParsedDocument>()
            .context("TL-B parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let completion =
            completion::completion(request.context.document, parsed, request.position)?;
        request
            .context
            .profiler
            .finish("tlb.completion", started_at);
        Ok(completion)
    }
}

#[derive(Debug)]
pub struct TlbParsedDocument {
    source_file: tlb_syntax::SourceFile,
    index: TlbSymbolIndex,
}

impl ParsedDocument for TlbParsedDocument {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn tree(&self) -> &Tree {
        &self.source_file.tree
    }
}
