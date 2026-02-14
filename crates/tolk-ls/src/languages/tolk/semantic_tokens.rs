use crate::backend::Backend;
use crate::backend::utils::SpanExt;
use lsp_types::*;
use std::sync::Arc;
use std::time::SystemTime;
use tolk_resolver::resolve_index::{FileResolveIndex, LocalDef, LocalDefKind, NameUse, Resolved};
use tolk_resolver::{AstNodeSpanExt, FileInfo, Span, Symbol, SymbolKind};
use tolk_syntax::AstNode;
use tower_lsp::jsonrpc::Result as LspResult;

use crate::AnalysisResult;

pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::STRUCT,         // 0
    SemanticTokenType::PROPERTY,       // 1
    SemanticTokenType::ENUM,           // 2
    SemanticTokenType::ENUM_MEMBER,    // 3
    SemanticTokenType::TYPE,           // 4
    SemanticTokenType::VARIABLE,       // 5
    SemanticTokenType::FUNCTION,       // 6
    SemanticTokenType::TYPE_PARAMETER, // 7
    SemanticTokenType::PARAMETER,      // 8
    SemanticTokenType::KEYWORD,        // 9
    SemanticTokenType::MACRO,          // 10
];

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum TokenType {
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

pub const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::MODIFICATION, // 0
];

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum TokenModifier {
    Modification = 0,
}

pub struct SemanticTokensBuilder {
    tokens: Vec<RawSemanticToken>,
    file: Arc<FileInfo>,
}

struct RawSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers: u32,
}

impl SemanticTokensBuilder {
    pub fn new(file: Arc<FileInfo>) -> Self {
        Self {
            tokens: Vec::new(),
            file,
        }
    }

    pub fn add_token_at_span(&mut self, span: Span, token_type: TokenType, token_modifiers: u32) {
        let range = span.range(&self.file);
        self.tokens.push(RawSemanticToken {
            line: range.start.line,
            start: range.start.character,
            length: range.end.character - range.start.character,
            token_type: token_type as u32,
            token_modifiers,
        });
    }

    pub fn build(mut self) -> Vec<SemanticToken> {
        if self.tokens.is_empty() {
            return Vec::new();
        }

        // Sort tokens as required by the LSP specification
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

        for tok in self.tokens {
            let delta_line = tok.line - last_line;
            let delta_start = if delta_line == 0 {
                tok.start - last_start
            } else {
                tok.start
            };

            result.push(SemanticToken {
                delta_line,
                delta_start,
                length: tok.length,
                token_type: tok.token_type,
                token_modifiers_bitset: tok.token_modifiers,
            });

            last_line = tok.line;
            last_start = tok.start;
        }

        result
    }
}

impl Backend {
    pub async fn handle_semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        crate::profile!(self, "semantic_tokens");
        let now = std::time::Instant::now();
        let uri = params.text_document.uri;
        log::info!("Request: semantic_tokens_full for {}", uri);

        let Some(data) = self.semantic_tokens(&uri) else {
            return Ok(None);
        };

        log::info!("Response: semantic_tokens_full took {:?}", now.elapsed());
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: Some(Self::result_id()),
            data,
        })))
    }

    fn semantic_tokens(&self, uri: &Url) -> Option<Vec<SemanticToken>> {
        let analysis = self.analysis.get(uri)?;
        let path = uri.to_file_path().ok()?;
        let file_info = self.file_db.get_by_path(&path)?;

        let mut builder = SemanticTokensBuilder::new(file_info.clone());

        let resolved_uses = analysis.project_index.get_resolved_uses(file_info.id())?;

        // 1. Process all declarations
        if let Some(file_index) = analysis.project_index.get_file_index(file_info.id()) {
            for decl in &file_index.decls {
                Self::add_symbol_tokens(&mut builder, decl);
            }
        }

        // 2. Process all local definitions
        for local in &resolved_uses.locals {
            let (token_type, modifiers) = Self::semantic_token_of_local(local);
            builder.add_token_at_span(local.def_span, token_type, modifiers);
        }

        // 3. Process all name usages
        for name_use in &resolved_uses.uses {
            Self::add_name_use_token(&mut builder, name_use, &analysis, resolved_uses);
        }

        // 4. Process all name usages from inference results
        if let Some(inferences) = analysis.all_body_types.get(&file_info.id()) {
            for inference in inferences.values() {
                for name_use in &inference.resolved_refs {
                    Self::add_name_use_token(&mut builder, name_use, &analysis, resolved_uses);
                }
            }
        }

        Some(builder.build())
    }

    fn add_name_use_token(
        builder: &mut SemanticTokensBuilder,
        name_use: &NameUse,
        analysis: &AnalysisResult,
        resolved_uses: &FileResolveIndex,
    ) {
        match name_use.resolved {
            Resolved::Local(local_id) => {
                if let Some(local) = resolved_uses.find_local(local_id) {
                    let (token_type, modifiers) = Self::semantic_token_of_local(local);
                    builder.add_token_at_span(name_use.span, token_type, modifiers);
                }
            }
            Resolved::Global(symbol_id) => {
                if let Some(symbol) = analysis.project_index.resolve_symbol(symbol_id) {
                    let token_type = match symbol.kind {
                        SymbolKind::Struct { .. } => {
                            if is_special_struct(&symbol.name) {
                                TokenType::Macro
                            } else {
                                TokenType::Struct
                            }
                        }
                        SymbolKind::StructField => TokenType::Property,
                        SymbolKind::Enum { .. } => TokenType::Enum,
                        SymbolKind::EnumMember => TokenType::EnumMember,
                        SymbolKind::TypeAlias { .. } => TokenType::Type,
                        SymbolKind::Constant => TokenType::Property,
                        SymbolKind::GlobalVariable => TokenType::Variable,
                        SymbolKind::Function { .. }
                        | SymbolKind::Method { .. }
                        | SymbolKind::GetMethod { .. } => TokenType::Function,
                    };
                    builder.add_token_at_span(name_use.span, token_type, 0);
                }
            }
            Resolved::Unresolved => {}
        }
    }

    fn semantic_token_of_local(local: &LocalDef) -> (TokenType, u32) {
        let (token_type, modifiers) = match local.kind {
            LocalDefKind::Param {
                is_mutable,
                is_self,
                ..
            } => {
                if is_self {
                    (
                        TokenType::Keyword,
                        if is_mutable {
                            1 << TokenModifier::Modification as u32
                        } else {
                            0
                        },
                    )
                } else {
                    (
                        TokenType::Parameter,
                        if is_mutable {
                            1 << TokenModifier::Modification as u32
                        } else {
                            0
                        },
                    )
                }
            }
            LocalDefKind::Var { is_mutable, .. } => (
                TokenType::Variable,
                if is_mutable {
                    1 << TokenModifier::Modification as u32
                } else {
                    0
                },
            ),
            LocalDefKind::Catch => (TokenType::Variable, 0),
            LocalDefKind::TypeParameter => (TokenType::TypeParameter, 0),
        };
        (token_type, modifiers)
    }

    fn add_symbol_tokens(builder: &mut SemanticTokensBuilder, symbol: &Symbol) {
        let token_type = match symbol.kind {
            SymbolKind::Struct { ref fields, .. } => {
                for field in fields {
                    Self::add_symbol_tokens(builder, field);
                }
                if is_special_struct(&symbol.name) {
                    TokenType::Macro
                } else {
                    TokenType::Struct
                }
            }
            SymbolKind::StructField => TokenType::Property,
            SymbolKind::Enum { ref members } => {
                for member in members {
                    Self::add_symbol_tokens(builder, member);
                }
                TokenType::Enum
            }
            SymbolKind::EnumMember => TokenType::EnumMember,
            SymbolKind::TypeAlias { .. } => TokenType::Type,
            SymbolKind::Constant => TokenType::Property,
            SymbolKind::GlobalVariable => TokenType::Variable,
            SymbolKind::Function { .. }
            | SymbolKind::Method { .. }
            | SymbolKind::GetMethod { .. } => TokenType::Function,
        };
        builder.add_token_at_span(symbol.name_span, token_type, 0);
    }

    fn result_id() -> String {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string()
    }
}

fn is_special_struct(name: &str) -> bool {
    name == "contract" || name == "blockchain" || name == "random" || name == "debug"
}
