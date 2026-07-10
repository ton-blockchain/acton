use super::{
    TolkResolveSnapshot, TolkWorkspaceEngine, fallback_uri_for_path, logical_path_for_uri,
};
use crate::{
    DocumentEdits, DocumentSnapshot, Position, PrepareRename, TextEdit, TextIndex, WorkspaceEdit,
};
use std::collections::BTreeMap;
use tolk_resolver::{FileId, Resolved, Span, SymbolKind, resolve_index::LocalDefId};
use tolk_syntax::{AstNode, HasName, InstanceArg, TryFromNode};
use tolk_ty::GlobalUsages;

impl TolkWorkspaceEngine {
    pub(super) fn prepare_rename(
        &self,
        document: &DocumentSnapshot,
        position: Position,
    ) -> anyhow::Result<Option<PrepareRename>> {
        let Some((snapshot, file_id, offset)) = self.rename_context(document, position) else {
            return Ok(None);
        };
        let Some(target) = snapshot.rename_target(file_id, offset) else {
            return Ok(None);
        };
        snapshot.ensure_renameable(&target.resolved)?;
        let Some(file) = snapshot.file_db.get_by_id(file_id) else {
            return Ok(None);
        };
        let source = file.source().source.as_ref();
        let placeholder = source
            .get(target.span.start()..target.span.end())
            .unwrap_or_default();
        let range = TextIndex::new(source).range_for_offsets(
            source,
            target.span.start(),
            target.span.end(),
        );

        Ok(Some(PrepareRename::new(range, placeholder)))
    }

    pub(super) fn rename(
        &self,
        document: &DocumentSnapshot,
        position: Position,
        new_name: &str,
    ) -> anyhow::Result<Option<WorkspaceEdit>> {
        let Some((snapshot, file_id, offset)) = self.rename_context(document, position) else {
            return Ok(None);
        };
        let Some(target) = snapshot.rename_target(file_id, offset) else {
            return Ok(None);
        };
        snapshot.ensure_renameable(&target.resolved)?;

        let replacement = rename_identifier(new_name);
        let edit = snapshot.workspace_edit(&target.resolved, &replacement);
        Ok((!edit.documents.is_empty()).then_some(edit))
    }

    fn rename_context(
        &self,
        document: &DocumentSnapshot,
        position: Position,
    ) -> Option<(std::sync::Arc<TolkResolveSnapshot>, FileId, usize)> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        }?;
        let path = logical_path_for_uri(document.uri());
        let file_id = snapshot.project_index.get_file_by_path(&path)?;
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);

        Some((snapshot, file_id, offset))
    }
}

impl TolkResolveSnapshot {
    fn rename_target(&self, file_id: FileId, offset: usize) -> Option<RenameTarget> {
        [offset, offset.saturating_sub(1)]
            .into_iter()
            .find_map(|offset| self.rename_target_at(file_id, offset))
    }

    fn rename_target_at(&self, file_id: FileId, offset: usize) -> Option<RenameTarget> {
        if let Some(name_use) = self.project_index.find_use(file_id, offset)
            && !matches!(name_use.resolved, Resolved::Unresolved)
        {
            return Some(RenameTarget {
                resolved: name_use.resolved.clone(),
                span: name_use.span,
            });
        }

        if let Some(symbol) = self.project_index.find_symbol_at(file_id, offset) {
            return Some(RenameTarget {
                resolved: Resolved::Global(symbol.id),
                span: symbol.name_span,
            });
        }

        if let Some(local) = self
            .project_index
            .get_resolved_uses(file_id)?
            .find_local_at(offset)
        {
            return Some(RenameTarget {
                resolved: Resolved::Local(local.id),
                span: local.def_span,
            });
        }

        None
    }

    fn ensure_renameable(&self, resolved: &Resolved) -> anyhow::Result<()> {
        if let Resolved::Global(symbol_id) = resolved
            && self.file_db.is_stdlib_file(symbol_id.file_id)
        {
            anyhow::bail!("cannot rename an element from the Tolk standard library");
        }

        Ok(())
    }

    fn workspace_edit(&self, resolved: &Resolved, replacement: &str) -> WorkspaceEdit {
        let is_field = matches!(
            resolved,
            Resolved::Global(symbol_id)
                if self
                    .project_index
                    .resolve_symbol(*symbol_id)
                    .is_some_and(|symbol| matches!(symbol.kind, SymbolKind::StructField))
        );
        let mut documents = BTreeMap::<String, DocumentEdits>::new();

        for (file_id, span) in self.rename_occurrences(resolved) {
            let Some(file) = self.file_db.get_by_id(file_id) else {
                continue;
            };
            let uri = self
                .path_to_uri
                .get(file.path())
                .cloned()
                .unwrap_or_else(|| fallback_uri_for_path(file.path()));
            let source = file.source().source.as_ref();
            let range = TextIndex::new(source).range_for_offsets(source, span.start(), span.end());
            let new_text = shorthand_replacement(file.source(), span, replacement, is_field)
                .unwrap_or_else(|| replacement.to_owned());
            let key = uri.as_str().to_owned();
            documents
                .entry(key)
                .or_insert_with(|| DocumentEdits::new(uri, Vec::new()))
                .edits
                .push(TextEdit::new(range, new_text));
        }

        let mut documents = documents.into_values().collect::<Vec<_>>();
        for document in &mut documents {
            document.edits.sort_by_key(|edit| edit.range.start);
        }
        WorkspaceEdit::new(documents)
    }

    fn rename_occurrences(&self, resolved: &Resolved) -> Vec<(FileId, Span)> {
        match resolved {
            Resolved::Global(symbol_id) => {
                let mut occurrences =
                    GlobalUsages::new(self.project_index.as_ref(), &self.all_body_types)
                        .for_symbol(*symbol_id)
                        .map(|reference| (reference.file_id, reference.usage.span))
                        .collect::<Vec<_>>();
                if let Some(symbol) = self.project_index.resolve_symbol(*symbol_id) {
                    occurrences.push((symbol.id.file_id, symbol.name_span));
                }
                occurrences
            }
            Resolved::Local(local_id) => self.local_rename_occurrences(*local_id),
            Resolved::Unresolved => Vec::new(),
        }
    }

    fn local_rename_occurrences(&self, local_id: LocalDefId) -> Vec<(FileId, Span)> {
        let Some(resolve_index) = self.project_index.get_resolved_uses(local_id.file_id) else {
            return Vec::new();
        };
        let mut occurrences = resolve_index
            .local_usages_of(local_id)
            .map(|usage| (local_id.file_id, usage.span))
            .collect::<Vec<_>>();
        if let Some(local) = resolve_index.find_local(local_id) {
            occurrences.push((local_id.file_id, local.def_span));
        }
        occurrences
    }
}

struct RenameTarget {
    resolved: Resolved,
    span: Span,
}

fn shorthand_replacement(
    source_file: &tolk_syntax::SourceFile,
    span: Span,
    replacement: &str,
    is_field: bool,
) -> Option<String> {
    let identifier = source_file
        .tree
        .root_node()
        .descendant_for_byte_range(span.start(), span.end())?;
    let argument = InstanceArg::try_from_node(identifier.parent()?).ok()?;
    if argument.has_value_separator() {
        return None;
    }
    let original = argument.name()?.text(source_file.source.as_ref());

    Some(if is_field {
        format!("{replacement}: {original}")
    } else {
        format!("{original}: {replacement}")
    })
}

fn rename_identifier(name: &str) -> String {
    if is_valid_identifier(name) || name.starts_with('`') && name.ends_with('`') {
        name.to_owned()
    } else {
        format!("`{name}`")
    }
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if !chars.all(|character| character.is_ascii_alphanumeric() || character == '_') {
        return false;
    }

    !TOLK_KEYWORDS.contains(&name)
}

const TOLK_KEYWORDS: &[&str] = &[
    "tolk", "import", "global", "const", "type", "struct", "fun", "get", "mutate", "asm",
    "builtin", "var", "val", "return", "repeat", "if", "else", "do", "while", "break", "continue",
    "throw", "assert", "try", "catch", "lazy", "is", "!is", "match", "true", "false", "null",
];
