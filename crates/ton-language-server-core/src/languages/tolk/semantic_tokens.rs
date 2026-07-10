use super::{TolkResolveSnapshot, TolkWorkspaceEngine, logical_path_for_uri};
use crate::semantic_tokens::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensBuilder,
};
use crate::{DocumentSnapshot, Range};
use tolk_resolver::resolve_index::{FileResolveIndex, LocalDef, LocalDefKind, NameUse, Resolved};
use tolk_resolver::{FileId, Span, Symbol, SymbolKind};

impl TolkWorkspaceEngine {
    pub(super) fn semantic_tokens(&self, document: &DocumentSnapshot) -> Vec<SemanticToken> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        };
        let Some(snapshot) = snapshot else {
            return Vec::new();
        };
        let path = logical_path_for_uri(document.uri());
        let Some(file_id) = snapshot.project_index.get_file_by_path(&path) else {
            return Vec::new();
        };

        snapshot.semantic_tokens(document, file_id)
    }
}

impl TolkResolveSnapshot {
    fn semantic_tokens(&self, document: &DocumentSnapshot, file_id: FileId) -> Vec<SemanticToken> {
        let mut builder = TolkSemanticTokensBuilder::new(document);
        let Some(resolved_uses) = self.project_index.get_resolved_uses(file_id) else {
            return builder.build();
        };

        if let Some(file_index) = self.project_index.get_file_index(file_id) {
            for symbol in &file_index.decls {
                add_symbol_tokens(&mut builder, symbol);
            }
        }

        for local in &resolved_uses.locals {
            let (token_type, modifiers) = semantic_token_of_local(local);
            builder.add_token_at_span(local.def_span, token_type, modifiers);
        }

        for name_use in &resolved_uses.uses {
            self.add_name_use_token(&mut builder, name_use, resolved_uses);
        }

        if let Some(inferences) = self.all_body_types.get(&file_id) {
            for inference in inferences.values() {
                for name_use in &inference.resolved_refs {
                    if resolved_uses
                        .find_use(name_use.span.start())
                        .is_some_and(|resolved| resolved.span == name_use.span)
                    {
                        continue;
                    }

                    self.add_name_use_token(&mut builder, name_use, resolved_uses);
                }
            }
        }

        builder.build()
    }

    fn add_name_use_token(
        &self,
        builder: &mut TolkSemanticTokensBuilder<'_>,
        name_use: &NameUse,
        resolved_uses: &FileResolveIndex,
    ) {
        match name_use.resolved {
            Resolved::Local(local_id) => {
                if let Some(local) = resolved_uses.find_local(local_id) {
                    let (token_type, modifiers) = semantic_token_of_local(local);
                    builder.add_token_at_span(name_use.span, token_type, modifiers);
                }
            }
            Resolved::Global(symbol_id) => {
                if let Some(symbol) = self.project_index.resolve_symbol(symbol_id) {
                    builder.add_token_at_span(name_use.span, semantic_token_of_symbol(symbol), 0);
                }
            }
            Resolved::Unresolved => {}
        }
    }
}

struct TolkSemanticTokensBuilder<'a> {
    inner: SemanticTokensBuilder,
    document: &'a DocumentSnapshot,
}

impl<'a> TolkSemanticTokensBuilder<'a> {
    const fn new(document: &'a DocumentSnapshot) -> Self {
        Self {
            inner: SemanticTokensBuilder::new(),
            document,
        }
    }

    fn add_token_at_span(
        &mut self,
        span: Span,
        token_type: SemanticTokenType,
        token_modifiers_bitset: u32,
    ) {
        self.inner.add_token_at_range(
            self.range_for_span(span),
            token_type,
            token_modifiers_bitset,
        );
    }

    fn range_for_span(&self, span: Span) -> Range {
        self.document
            .text_index()
            .range_for_offsets(self.document.text(), span.start(), span.end())
    }

    fn build(self) -> Vec<SemanticToken> {
        self.inner.build()
    }
}

fn add_symbol_tokens(builder: &mut TolkSemanticTokensBuilder<'_>, symbol: &Symbol) {
    match &symbol.kind {
        SymbolKind::Struct { fields, .. } => {
            for field in fields {
                add_symbol_tokens(builder, field);
            }
        }
        SymbolKind::Enum { members } => {
            for member in members {
                add_symbol_tokens(builder, member);
            }
        }
        SymbolKind::GlobalVariable
        | SymbolKind::StructField
        | SymbolKind::EnumMember
        | SymbolKind::TypeAlias { .. }
        | SymbolKind::Constant
        | SymbolKind::Function { .. }
        | SymbolKind::Method { .. }
        | SymbolKind::GetMethod { .. } => {}
    }
    builder.add_token_at_span(symbol.name_span, semantic_token_of_symbol(symbol), 0);
}

fn semantic_token_of_symbol(symbol: &Symbol) -> SemanticTokenType {
    match symbol.kind {
        SymbolKind::Struct { .. } => {
            if is_special_struct(&symbol.name) {
                SemanticTokenType::Macro
            } else {
                SemanticTokenType::Struct
            }
        }
        SymbolKind::StructField | SymbolKind::Constant => SemanticTokenType::Property,
        SymbolKind::Enum { .. } => SemanticTokenType::Enum,
        SymbolKind::EnumMember => SemanticTokenType::EnumMember,
        SymbolKind::TypeAlias { .. } => SemanticTokenType::Type,
        SymbolKind::GlobalVariable => SemanticTokenType::Variable,
        SymbolKind::Function { .. } | SymbolKind::Method { .. } | SymbolKind::GetMethod { .. } => {
            SemanticTokenType::Function
        }
    }
}

const fn semantic_token_of_local(local: &LocalDef) -> (SemanticTokenType, u32) {
    match local.kind {
        LocalDefKind::Param {
            is_mutable,
            is_self,
            ..
        } => {
            let modifiers = mutable_modifier(is_mutable);
            if is_self {
                (SemanticTokenType::Keyword, modifiers)
            } else {
                (SemanticTokenType::Parameter, modifiers)
            }
        }
        LocalDefKind::Var { is_mutable, .. } => {
            (SemanticTokenType::Variable, mutable_modifier(is_mutable))
        }
        LocalDefKind::Catch => (SemanticTokenType::Variable, 0),
        LocalDefKind::TypeParameter => (SemanticTokenType::TypeParameter, 0),
    }
}

const fn mutable_modifier(is_mutable: bool) -> u32 {
    if is_mutable {
        SemanticTokenModifier::Modification.bitset()
    } else {
        0
    }
}

fn is_special_struct(name: &str) -> bool {
    matches!(name, "contract" | "blockchain" | "random" | "debug")
}
