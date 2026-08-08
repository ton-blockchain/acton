use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::hash::{Hash, Hasher};
use tolk_resolver::{FileId, FileInfo, ProjectIndex, SymbolId};
use tolk_syntax::{FunctionLike, TopLevel};
use tree_sitter::Node;

pub(super) type FileDeclarationStamps = FxHashMap<SymbolId, DeclarationStamp>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DeclarationStamp {
    text_hash: u64,
    signature_hash: u64,
    start: u32,
    inferred_signature: bool,
}

#[derive(Debug)]
pub(super) struct DeclarationChanges {
    pub changed: FxHashSet<SymbolId>,
    pub relocated: FxHashMap<SymbolId, i64>,
    pub potential_signature_changes: FxHashSet<SymbolId>,
    pub signature_changed: bool,
}

impl DeclarationChanges {
    pub(super) fn between(
        current: &FileDeclarationStamps,
        previous: Option<&FileDeclarationStamps>,
    ) -> Self {
        let Some(previous) = previous else {
            return Self {
                changed: current.keys().copied().collect(),
                relocated: FxHashMap::default(),
                potential_signature_changes: FxHashSet::default(),
                signature_changed: true,
            };
        };

        let mut changed = FxHashSet::default();
        let mut relocated = FxHashMap::default();
        let mut potential_signature_changes = FxHashSet::default();
        let mut signature_changed = current.len() != previous.len();

        for (&symbol_id, current) in current {
            let Some(previous) = previous.get(&symbol_id) else {
                changed.insert(symbol_id);
                signature_changed = true;
                continue;
            };

            if current.text_hash != previous.text_hash {
                changed.insert(symbol_id);
                if current.inferred_signature {
                    potential_signature_changes.insert(symbol_id);
                }
            } else if current.start != previous.start {
                relocated.insert(
                    symbol_id,
                    i64::from(current.start) - i64::from(previous.start),
                );
            }

            signature_changed |= current.signature_hash != previous.signature_hash;
            signature_changed |= current.inferred_signature != previous.inferred_signature;
        }

        Self {
            changed,
            relocated,
            potential_signature_changes,
            signature_changed,
        }
    }
}

pub(super) fn collect_declaration_stamps(file: &FileInfo) -> FileDeclarationStamps {
    let source_file = file.source();
    let source = source_file.source.as_bytes();

    source_file
        .top_levels()
        .filter_map(|declaration| {
            let symbol = file.find_declaration(&declaration)?;
            let syntax = declaration.syntax();
            let text = source.get(syntax.byte_range())?;
            let (excluded_body, inferred_signature) = function_signature(&declaration);

            Some((
                symbol.id,
                DeclarationStamp {
                    text_hash: hash(text),
                    signature_hash: syntax_hash(syntax, source, excluded_body),
                    start: syntax.start_byte() as u32,
                    inferred_signature,
                },
            ))
        })
        .collect()
}

pub(super) fn imports_changed(
    current: &ProjectIndex,
    previous: Option<&ProjectIndex>,
    file_id: FileId,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    let current = current
        .imports()
        .get(&file_id)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let previous = previous
        .imports()
        .get(&file_id)
        .map(Vec::as_slice)
        .unwrap_or_default();

    !current
        .iter()
        .map(|import| (import.path(), import.target()))
        .eq(previous
            .iter()
            .map(|import| (import.path(), import.target())))
}

fn function_signature<'tree>(declaration: &TopLevel<'tree>) -> (Option<Node<'tree>>, bool) {
    match declaration {
        TopLevel::Func(function) => (
            function.body().map(|body| body.syntax()),
            function.return_type().is_none(),
        ),
        TopLevel::Method(method) => (
            method.body().map(|body| body.syntax()),
            method.return_type().is_none(),
        ),
        TopLevel::GetMethod(method) => (
            method.body().map(|body| body.syntax()),
            method.return_type().is_none(),
        ),
        _ => (None, false),
    }
}

fn syntax_hash(root: Node<'_>, source: &[u8], excluded: Option<Node<'_>>) -> u64 {
    let mut hasher = FxHasher::default();
    hash_syntax(root, source, excluded, &mut hasher);
    hasher.finish()
}

fn hash_syntax(node: Node<'_>, source: &[u8], excluded: Option<Node<'_>>, hasher: &mut FxHasher) {
    if Some(node) == excluded || node.kind() == "comment" {
        return;
    }

    0_u8.hash(hasher);
    node.kind_id().hash(hasher);
    node.is_missing().hash(hasher);

    if node.child_count() == 0 {
        source
            .get(node.byte_range())
            .unwrap_or_default()
            .hash(hasher);
    } else {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            hash_syntax(child, source, excluded, hasher);
        }
    }

    1_u8.hash(hasher);
}

fn hash(value: impl Hash) -> u64 {
    let mut hasher = FxHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}
