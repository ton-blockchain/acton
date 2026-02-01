use crate::backend::Backend;
use crate::backend::utils::SpanExt;
use lsp_types::*;
use std::sync::Arc;
use std::time::SystemTime;
use tolk_resolver::resolve_index::LocalDefKind;
use tolk_resolver::{AstNodeSpanExt, FileInfo, Resolved, SymbolKind};
use tolk_syntax::ast::{
    Constant, Enum, EnumMember, Func, GetMethod, GlobalVar, Method, NodeTraversalExt, Parameter,
    Struct, StructField, TypeAlias, TypeParameter,
};
use tolk_syntax::{AstNode, AstNodeBytesKind, HasName, TryFromNode};
use tower_lsp::jsonrpc::Result as LspResult;

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

    pub fn add_token<'a, Node: AstNode<'a>>(
        &mut self,
        node: Node,
        token_type: TokenType,
        token_modifiers: u32,
    ) {
        let range = node.span().range(&self.file);
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
        let source = &file_info.source().source;
        let root = file_info.source().root_node();

        for node in root.traverse() {
            if let Ok(decl) = Struct::try_from_node(node) {
                if let Some(name) = decl.name() {
                    let token_type = if is_special_struct(name.text(source)) {
                        TokenType::Macro
                    } else {
                        TokenType::Struct
                    };
                    builder.add_token(name, token_type, 0);
                }
                continue;
            }
            if let Ok(decl) = StructField::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::Property, 0);
                continue;
            }
            if let Ok(decl) = Enum::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::Enum, 0);
                continue;
            }
            if let Ok(decl) = EnumMember::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::EnumMember, 0);
                continue;
            }
            if let Ok(decl) = TypeAlias::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::Type, 0);
                continue;
            }
            if let Ok(decl) = Constant::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::Property, 0);
                continue;
            }
            if let Ok(decl) = GlobalVar::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::Variable, 0);
                continue;
            }
            if let Ok(decl) = Func::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::Function, 0);
                continue;
            }
            if let Ok(decl) = Method::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::Function, 0);
                continue;
            }
            if let Ok(decl) = GetMethod::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::Function, 0);
                continue;
            }
            if let Ok(decl) = TypeParameter::try_from_node(node)
                && let Some(name) = decl.name()
            {
                builder.add_token(name, TokenType::TypeParameter, 0);
                continue;
            }

            if let Ok(decl) = Parameter::try_from_node(node)
                && let Some(name) = decl.name()
            {
                let is_mutable = decl.mutate();
                let text = name.text(source);
                let (token_type, modifiers) = if text == "self" {
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
                };
                builder.add_token(name, token_type, modifiers);
                continue;
            }

            let kind = node.kind_bytes();
            if (kind == b"identifier" || kind == b"type_identifier")
                && let Some(resolved) =
                    self.resolve_symbol_at(&analysis, &file_info, node.start_byte())
            {
                match resolved {
                    Resolved::Local(local_id) => {
                        if let Some(resolved_uses) =
                            analysis.project_index.get_resolved_uses(file_info.id())
                            && let Some(local_def) = resolved_uses.find_local(local_id)
                        {
                            let is_mutable = matches!(local_def.kind, LocalDefKind::Param { is_mutable, .. } if is_mutable);
                            let (token_type, modifiers) = if node.text_matches(source, "self") {
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
                            };
                            builder.add_token(node, token_type, modifiers);
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
                            builder.add_token(node, token_type, 0);
                        }
                    }
                    Resolved::Unresolved => {}
                }
            }
        }

        Some(builder.build())
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
