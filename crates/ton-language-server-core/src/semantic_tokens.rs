use crate::Range;
use web_time::{SystemTime, UNIX_EPOCH};

pub const SEMANTIC_TOKEN_TYPE_NAMES: &[&str] = &[
    "struct",
    "property",
    "enum",
    "enumMember",
    "type",
    "variable",
    "function",
    "typeParameter",
    "parameter",
    "keyword",
    "macro",
];

pub const SEMANTIC_TOKEN_MODIFIER_NAMES: &[&str] = &["modification"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SemanticTokenType {
    Struct = 0,
    Property = 1,
    Enum = 2,
    EnumMember = 3,
    Type = 4,
    Variable = 5,
    Function = 6,
    TypeParameter = 7,
    Parameter = 8,
    Keyword = 9,
    Macro = 10,
}

impl SemanticTokenType {
    #[must_use]
    pub const fn index(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SemanticTokenModifier {
    Modification = 0,
}

impl SemanticTokenModifier {
    #[must_use]
    pub const fn bitset(self) -> u32 {
        1 << (self as u32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticToken {
    pub delta_line: u32,
    pub delta_start: u32,
    pub length: u32,
    pub token_type: u32,
    pub token_modifiers_bitset: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTokens {
    pub result_id: Option<String>,
    pub data: Vec<SemanticToken>,
}

impl SemanticTokens {
    #[must_use]
    pub fn new(data: Vec<SemanticToken>) -> Self {
        Self {
            result_id: Some(semantic_tokens_result_id()),
            data,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RawSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
}

#[derive(Debug, Default)]
pub struct SemanticTokensBuilder {
    tokens: Vec<RawSemanticToken>,
}

impl SemanticTokensBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    pub fn add_token(
        &mut self,
        line: u32,
        start: u32,
        length: u32,
        token_type: SemanticTokenType,
        token_modifiers_bitset: u32,
    ) {
        if length == 0 {
            return;
        }

        self.tokens.push(RawSemanticToken {
            line,
            start,
            length,
            token_type: token_type.index(),
            token_modifiers_bitset,
        });
    }

    pub fn add_token_at_range(
        &mut self,
        range: Range,
        token_type: SemanticTokenType,
        token_modifiers_bitset: u32,
    ) {
        if range.start.line != range.end.line {
            return;
        }
        let length = range.end.character.saturating_sub(range.start.character);
        self.add_token(
            range.start.line,
            range.start.character,
            length,
            token_type,
            token_modifiers_bitset,
        );
    }

    #[must_use]
    pub fn build(mut self) -> Vec<SemanticToken> {
        if self.tokens.is_empty() {
            return Vec::new();
        }

        self.tokens.sort_by_key(|token| (token.line, token.start));

        let mut result = Vec::with_capacity(self.tokens.len());
        let mut last_line = 0;
        let mut last_start = 0;

        for token in self.tokens {
            let delta_line = token.line - last_line;
            let delta_start = if delta_line == 0 {
                token.start - last_start
            } else {
                token.start
            };

            result.push(SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.token_modifiers_bitset,
            });

            last_line = token.line;
            last_start = token.start;
        }

        result
    }
}

#[must_use]
fn semantic_tokens_result_id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
