use lsp_types::{Range, SemanticToken};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug)]
struct RawSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers: u32,
}

pub struct SemanticTokensBuilder {
    tokens: Vec<RawSemanticToken>,
}

impl SemanticTokensBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    pub fn add_token(
        &mut self,
        line: u32,
        start: u32,
        length: u32,
        token_type: u32,
        token_modifiers: u32,
    ) {
        if length == 0 {
            return;
        }

        self.tokens.push(RawSemanticToken {
            line,
            start,
            length,
            token_type,
            token_modifiers,
        });
    }

    pub fn add_token_at_range(&mut self, range: Range, token_type: u32, token_modifiers: u32) {
        let length = range.end.character.saturating_sub(range.start.character);
        self.add_token(
            range.start.line,
            range.start.character,
            length,
            token_type,
            token_modifiers,
        );
    }

    #[must_use]
    pub fn build(mut self) -> Vec<SemanticToken> {
        if self.tokens.is_empty() {
            return Vec::new();
        }

        self.tokens.sort_by(|a, b| {
            if a.line != b.line {
                a.line.cmp(&b.line)
            } else {
                a.start.cmp(&b.start)
            }
        });

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
                token_modifiers_bitset: token.token_modifiers,
            });

            last_line = token.line;
            last_start = token.start;
        }

        result
    }
}

#[must_use]
pub fn semantic_tokens_result_id() -> String {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
