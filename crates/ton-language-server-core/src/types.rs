use crate::text::TextIndex;
use std::fmt;
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
