use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SchemaPathSegment {
    Key(String),
    Index(usize),
}

impl SchemaPathSegment {
    #[must_use]
    pub fn as_token(&self) -> String {
        match self {
            SchemaPathSegment::Key(value) => value.clone(),
            SchemaPathSegment::Index(value) => value.to_string(),
        }
    }
}

impl From<&str> for SchemaPathSegment {
    fn from(value: &str) -> Self {
        Self::Key(value.to_string())
    }
}

impl From<String> for SchemaPathSegment {
    fn from(value: String) -> Self {
        Self::Key(value)
    }
}

impl From<usize> for SchemaPathSegment {
    fn from(value: usize) -> Self {
        Self::Index(value)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SchemaDoc {
    pub title: Option<String>,
    pub description: Option<String>,
    pub schema_type: Option<String>,
    pub const_value: Option<Value>,
    pub default_value: Option<Value>,
    pub enum_values: Vec<Value>,
    pub examples: Vec<Value>,
}

impl SchemaDoc {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.schema_type.is_none()
            && self.const_value.is_none()
            && self.default_value.is_none()
            && self.enum_values.is_empty()
            && self.examples.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionProperty {
    pub name: String,
    pub required: bool,
    pub doc: SchemaDoc,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompletionInfo {
    pub properties: Vec<CompletionProperty>,
    pub has_pattern_properties: bool,
    pub allows_additional_properties: bool,
}
