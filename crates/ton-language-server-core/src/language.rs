use crate::completion::{CompletionList, CompletionTrigger};
use crate::custom::TypeAtPosition;
use crate::profiling::Profiler;
use crate::semantic_tokens::SemanticToken;
use crate::types::{
    CodeAction, CodeLens, DocumentHighlight, DocumentSnapshot, DocumentSymbol, DocumentUri,
    FileRename, FoldingRange, Hover, InlayHint, Location, Position, PrepareRename, Range,
    SignatureHelp, WorkspaceConfig, WorkspaceEdit, WorkspaceSymbol,
};
use std::any::Any;
use std::sync::Arc;
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
    pub inlay_hints: bool,
    pub signature_help: bool,
    pub rename: bool,
    pub type_definition: bool,
    pub document_highlight: bool,
    pub workspace_symbols: bool,
    pub code_actions: bool,
    pub file_rename: bool,
    pub type_at_position: bool,
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

    fn add_source_file(&self, _uri: DocumentUri, _text: Arc<str>) -> anyhow::Result<()> {
        anyhow::bail!("workspace source files are not supported by this language")
    }

    fn remove_source_file(&self, _uri: &DocumentUri) -> anyhow::Result<()> {
        anyhow::bail!("workspace source files are not supported by this language")
    }

    fn set_workspace_config(&self, _config: WorkspaceConfig) -> anyhow::Result<()> {
        anyhow::bail!("workspace configuration is not supported by this language")
    }
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

pub struct ReferenceRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
    pub include_declaration: bool,
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

pub struct DocumentSymbolRequest<'a> {
    pub context: PluginContext<'a>,
}

pub struct SignatureHelpRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
}

pub struct PrepareRenameRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
}

pub struct RenameRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
    pub new_name: &'a str,
}

pub struct TypeDefinitionRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
}

pub struct DocumentHighlightRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
}

pub struct WorkspaceSymbolRequest<'a> {
    pub query: &'a str,
    pub profiler: &'a mut Profiler,
}

pub struct CodeActionRequest<'a> {
    pub context: PluginContext<'a>,
    pub range: Range,
}

pub struct FileRenameRequest<'a> {
    pub files: &'a [FileRename],
    pub profiler: &'a mut Profiler,
}

pub struct SemanticTokensRequest<'a> {
    pub context: PluginContext<'a>,
}

pub struct InlayHintRequest<'a> {
    pub context: PluginContext<'a>,
    pub range: Range,
}

pub struct CompletionRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
    pub trigger: CompletionTrigger,
}

pub struct TypeAtPositionRequest<'a> {
    pub context: PluginContext<'a>,
    pub position: Position,
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

    fn references(&self, _request: ReferenceRequest<'_>) -> anyhow::Result<Vec<Location>> {
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

    fn document_symbols(
        &self,
        _request: DocumentSymbolRequest<'_>,
    ) -> anyhow::Result<Vec<DocumentSymbol>> {
        Ok(Vec::new())
    }

    fn signature_help(
        &self,
        _request: SignatureHelpRequest<'_>,
    ) -> anyhow::Result<Option<SignatureHelp>> {
        Ok(None)
    }

    fn prepare_rename(
        &self,
        _request: PrepareRenameRequest<'_>,
    ) -> anyhow::Result<Option<PrepareRename>> {
        Ok(None)
    }

    fn rename(&self, _request: RenameRequest<'_>) -> anyhow::Result<Option<WorkspaceEdit>> {
        Ok(None)
    }

    fn type_definition(
        &self,
        _request: TypeDefinitionRequest<'_>,
    ) -> anyhow::Result<Vec<Location>> {
        Ok(Vec::new())
    }

    fn document_highlights(
        &self,
        _request: DocumentHighlightRequest<'_>,
    ) -> anyhow::Result<Vec<DocumentHighlight>> {
        Ok(Vec::new())
    }

    fn workspace_symbols(
        &self,
        _request: WorkspaceSymbolRequest<'_>,
    ) -> anyhow::Result<Vec<WorkspaceSymbol>> {
        Ok(Vec::new())
    }

    fn code_actions(&self, _request: CodeActionRequest<'_>) -> anyhow::Result<Vec<CodeAction>> {
        Ok(Vec::new())
    }

    fn will_rename_files(
        &self,
        _request: FileRenameRequest<'_>,
    ) -> anyhow::Result<Option<WorkspaceEdit>> {
        Ok(None)
    }

    fn did_rename_files(&self, _files: &[FileRename]) -> anyhow::Result<()> {
        Ok(())
    }

    fn semantic_tokens(
        &self,
        _request: SemanticTokensRequest<'_>,
    ) -> anyhow::Result<Vec<SemanticToken>> {
        Ok(Vec::new())
    }

    fn inlay_hints(&self, _request: InlayHintRequest<'_>) -> anyhow::Result<Vec<InlayHint>> {
        Ok(Vec::new())
    }

    fn completion(&self, _request: CompletionRequest<'_>) -> anyhow::Result<CompletionList> {
        Ok(CompletionList::default())
    }

    fn type_at_position(
        &self,
        _request: TypeAtPositionRequest<'_>,
    ) -> anyhow::Result<Option<TypeAtPosition>> {
        Ok(None)
    }
}
