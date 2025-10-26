use serde::Deserialize;

/// Source map data structure for Tolk compiler output
#[derive(Debug, Clone, Deserialize)]
pub struct SourceMap {
    pub version: String,
    pub language: Option<String>,
    pub compiler_version: Option<String>,
    pub files: Vec<SourceFile>,
    pub globals: Vec<GlobalVariable>,
    pub locations: Vec<DebugLocation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub is_stdlib: bool,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlobalVariable {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: i64,
    pub column: i64,
    pub end_line: i64,
    pub end_column: i64,
    pub length: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DebugLocation {
    pub idx: i64,
    pub loc: SourceLocation,
    pub variables: Vec<Variable>,
    pub context: EntryContext,
    pub debug: Option<DebugInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Variable {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub is_temporary: Option<bool>,
    pub constant_value: Option<String>,
    pub possible_qualifier_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryContext {
    pub description: EntryContextDescription,
    pub inlining: InliningInfo,
    pub event: Option<String>,
    pub containing_function: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum EntryContextDescription {
    Basic {
        ast_kind: String,
    },
    Assert {
        ast_kind: String,
        is_assert_throw: bool,
        condition: String,
    },
    BinaryOperator {
        ast_kind: String,
        description: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct InliningInfo {
    pub inlined_to_func: Option<String>,
    pub containing_func_inline_mode: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DebugInfo {
    pub opcode: String,
    pub line_str: String,
    pub line_off: String,
}
