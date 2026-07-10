use crate::text::TextIndex;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentUri(Arc<str>);

impl DocumentUri {
    #[must_use]
    pub fn new(uri: impl Into<Arc<str>>) -> Self {
        Self(uri.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this URI into the normalized logical path used as a workspace
    /// file key.
    ///
    /// `file://` URIs use their file path, while other URI schemes use the
    /// portion after `://`. Values without a scheme are treated as paths. The
    /// result is rooted at `/` and `.`/`..` components are resolved lexically;
    /// this method does not access the filesystem or percent-decode the URI.
    #[must_use]
    pub fn logical_path(&self) -> PathBuf {
        let path = if let Some(file_path) = self.0.strip_prefix("file://") {
            PathBuf::from(format!("/{}", file_path.trim_start_matches('/')))
        } else if let Some((_, rest)) = self.0.split_once("://") {
            PathBuf::from(format!("/{}", rest.trim_start_matches('/')))
        } else {
            PathBuf::from(self.0.as_ref())
        };
        normalize_logical_path(&path)
    }
}

/// Normalizes a workspace path lexically and ensures that it is rooted at `/`.
///
/// This helper deliberately does not canonicalize through the filesystem, so
/// it also works for virtual documents and provider-backed source files.
pub(crate) fn normalize_logical_path(path: &Path) -> PathBuf {
    let normalized = normalize_path(path);
    if normalized.is_absolute() {
        return normalized;
    }
    if normalized.as_os_str().is_empty() || normalized == Path::new(".") {
        return PathBuf::from("/");
    }
    Path::new("/").join(normalized)
}

/// Resolves `.` and `..` components without accessing the filesystem.
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

impl From<&str> for DocumentUri {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DocumentUri {
    fn from(value: String) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

impl fmt::Display for DocumentUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct WorkspaceConfig {
    root_uri: DocumentUri,
    manifest_uri: Option<DocumentUri>,
    manifest_text: Arc<str>,
    tolk_stdlib_root_uri: Option<DocumentUri>,
}

impl WorkspaceConfig {
    #[must_use]
    pub fn new(
        root_uri: impl Into<DocumentUri>,
        manifest_uri: Option<DocumentUri>,
        manifest_text: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            root_uri: root_uri.into(),
            manifest_uri,
            manifest_text: manifest_text.into(),
            tolk_stdlib_root_uri: None,
        }
    }

    #[must_use]
    pub fn with_tolk_stdlib_root_uri(mut self, uri: impl Into<DocumentUri>) -> Self {
        self.tolk_stdlib_root_uri = Some(uri.into());
        self
    }

    #[must_use]
    pub const fn root_uri(&self) -> &DocumentUri {
        &self.root_uri
    }

    #[must_use]
    pub const fn manifest_uri(&self) -> Option<&DocumentUri> {
        self.manifest_uri.as_ref()
    }

    #[must_use]
    pub const fn manifest_text(&self) -> &Arc<str> {
        &self.manifest_text
    }

    #[must_use]
    pub const fn tolk_stdlib_root_uri(&self) -> Option<&DocumentUri> {
        self.tolk_stdlib_root_uri.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LanguageId(Arc<str>);

impl LanguageId {
    #[must_use]
    pub fn new(language_id: impl Into<Arc<str>>) -> Self {
        Self(language_id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for LanguageId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for LanguageId {
    fn from(value: String) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

impl fmt::Display for LanguageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    uri: DocumentUri,
    language_id: LanguageId,
    version: i32,
    text: Arc<str>,
    text_index: Arc<TextIndex>,
}

impl DocumentSnapshot {
    #[must_use]
    pub fn new(
        uri: DocumentUri,
        language_id: LanguageId,
        version: i32,
        text: impl Into<Arc<str>>,
    ) -> Self {
        let text = text.into();
        let text_index = Arc::new(TextIndex::new(&text));
        Self {
            uri,
            language_id,
            version,
            text,
            text_index,
        }
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    #[must_use]
    pub const fn language_id(&self) -> &LanguageId {
        &self.language_id
    }

    #[must_use]
    pub const fn version(&self) -> i32 {
        self.version
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn text_index(&self) -> &TextIndex {
        &self.text_index
    }

    #[must_use]
    pub fn text_of<'tree, N>(&self, node: N) -> &str
    where
        N: ton_syntax::ast::AstNode<'tree>,
    {
        self.text.get(node.syntax().byte_range()).unwrap_or("")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    #[must_use]
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentEdits {
    pub uri: DocumentUri,
    pub edits: Vec<TextEdit>,
}

impl DocumentEdits {
    #[must_use]
    pub const fn new(uri: DocumentUri, edits: Vec<TextEdit>) -> Self {
        Self { uri, edits }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceEdit {
    pub documents: Vec<DocumentEdits>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeActionKind {
    QuickFix,
    Refactor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeAction {
    pub title: String,
    pub kind: CodeActionKind,
    pub edit: WorkspaceEdit,
}

impl CodeAction {
    #[must_use]
    pub fn new(title: impl Into<String>, kind: CodeActionKind, edit: WorkspaceEdit) -> Self {
        Self {
            title: title.into(),
            kind,
            edit,
        }
    }
}

impl WorkspaceEdit {
    #[must_use]
    pub const fn new(documents: Vec<DocumentEdits>) -> Self {
        Self { documents }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrepareRename {
    pub range: Range,
    pub placeholder: String,
}

impl PrepareRename {
    #[must_use]
    pub fn new(range: Range, placeholder: impl Into<String>) -> Self {
        Self {
            range,
            placeholder: placeholder.into(),
        }
    }
}

impl TextEdit {
    #[must_use]
    pub fn new(range: Range, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub uri: DocumentUri,
    pub range: Range,
}

impl Location {
    #[must_use]
    pub const fn new(uri: DocumentUri, range: Range) -> Self {
        Self { uri, range }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hover {
    pub contents: String,
    pub range: Option<Range>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentHighlightKind {
    Text,
    Read,
    Write,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentHighlight {
    pub range: Range,
    pub kind: Option<DocumentHighlightKind>,
}

impl DocumentHighlight {
    #[must_use]
    pub const fn new(range: Range, kind: DocumentHighlightKind) -> Self {
        Self {
            range,
            kind: Some(kind),
        }
    }
}

impl Hover {
    #[must_use]
    pub fn new(contents: impl Into<String>, range: Option<Range>) -> Self {
        Self {
            contents: contents.into(),
            range,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub title: String,
    pub command: String,
    pub arguments: Vec<String>,
}

impl Command {
    #[must_use]
    pub fn new(title: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            command: command.into(),
            arguments: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeLens {
    pub range: Range,
    pub command: Option<Command>,
}

impl CodeLens {
    #[must_use]
    pub const fn new(range: Range, command: Option<Command>) -> Self {
        Self { range, command }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InlayHintKind {
    Type,
    Parameter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlayHint {
    pub position: Position,
    pub label: String,
    pub kind: Option<InlayHintKind>,
    pub tooltip: Option<String>,
    pub text_edits: Vec<TextEdit>,
    pub padding_left: bool,
    pub padding_right: bool,
}

impl InlayHint {
    #[must_use]
    pub fn new(position: Position, label: impl Into<String>, kind: InlayHintKind) -> Self {
        Self {
            position,
            label: label.into(),
            kind: Some(kind),
            tooltip: None,
            text_edits: Vec::new(),
            padding_left: false,
            padding_right: false,
        }
    }

    #[must_use]
    pub fn plain(position: Position, label: impl Into<String>) -> Self {
        Self {
            position,
            label: label.into(),
            kind: None,
            tooltip: None,
            text_edits: Vec::new(),
            padding_left: false,
            padding_right: false,
        }
    }

    #[must_use]
    pub fn with_text_edit(mut self, edit: TextEdit) -> Self {
        self.text_edits.push(edit);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FoldingRange {
    pub start_line: u32,
    pub start_character: Option<u32>,
    pub end_line: u32,
    pub end_character: Option<u32>,
}

impl FoldingRange {
    #[must_use]
    pub const fn new(
        start_line: u32,
        start_character: Option<u32>,
        end_line: u32,
        end_character: Option<u32>,
    ) -> Self {
        Self {
            start_line,
            start_character,
            end_line,
            end_character,
        }
    }

    #[must_use]
    pub const fn line_range(start_line: u32, end_line: u32) -> Self {
        Self::new(start_line, None, end_line, None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocumentSymbolKind {
    File,
    Module,
    Namespace,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: DocumentSymbolKind,
    pub range: Range,
    pub selection_range: Range,
    pub children: Vec<DocumentSymbol>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: DocumentSymbolKind,
    pub location: Location,
    pub container_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRename {
    pub old_uri: DocumentUri,
    pub new_uri: DocumentUri,
}

impl FileRename {
    #[must_use]
    pub const fn new(old_uri: DocumentUri, new_uri: DocumentUri) -> Self {
        Self { old_uri, new_uri }
    }
}

impl WorkspaceSymbol {
    #[must_use]
    pub fn new(name: impl Into<String>, kind: DocumentSymbolKind, location: Location) -> Self {
        Self {
            name: name.into(),
            kind,
            location,
            container_name: None,
        }
    }
}

impl DocumentSymbol {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        kind: DocumentSymbolKind,
        range: Range,
        selection_range: Range,
    ) -> Self {
        Self {
            name: name.into(),
            detail: None,
            kind,
            range,
            selection_range,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_children(mut self, children: Vec<DocumentSymbol>) -> Self {
        self.children = children;
        self
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignatureInformation {
    pub label: String,
    pub documentation: Option<String>,
    /// Parameter labels in declaration order.
    ///
    /// Tolk parameters cannot carry their own documentation, so labels are
    /// stored directly instead of wrapping each one in a metadata object.
    pub parameters: Vec<String>,
    pub active_parameter: Option<u32>,
}

impl SignatureInformation {
    #[must_use]
    pub fn new(label: impl Into<String>, parameters: Vec<String>) -> Self {
        Self {
            label: label.into(),
            documentation: None,
            parameters,
            active_parameter: None,
        }
    }

    #[must_use]
    pub const fn with_active_parameter(mut self, active_parameter: u32) -> Self {
        self.active_parameter = Some(active_parameter);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: Option<u32>,
    pub active_parameter: Option<u32>,
}

impl SignatureHelp {
    #[must_use]
    pub fn new(signature: SignatureInformation) -> Self {
        Self {
            signatures: vec![signature],
            active_signature: Some(0),
            active_parameter: None,
        }
    }
}
