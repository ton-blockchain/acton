use super::file_info::FileInfoExt;
use super::resolution::ResolvedTarget;
use super::{TolkResolveSnapshot, TolkWorkspaceEngine};
use crate::{DocumentEdits, DocumentSnapshot, Position, PrepareRename, TextEdit, WorkspaceEdit};
use std::collections::BTreeMap;
use std::sync::Arc;
use tolk_resolver::{FileId, Resolved, Span, SymbolKind, resolve_index::LocalDefId};
use tolk_syntax::{HasName, InstanceArg, TryFromNode};
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

        let placeholder = file.text_at(target.span);
        let range = file.range_for_span(target.span);

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
    ) -> Option<(Arc<TolkResolveSnapshot>, FileId, usize)> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        }?;
        let file_id = snapshot.find_document_file(document)?;
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);

        Some((snapshot, file_id, offset))
    }
}

impl TolkResolveSnapshot {
    fn rename_target(&self, file_id: FileId, offset: usize) -> Option<ResolvedTarget> {
        [offset, offset.saturating_sub(1)]
            .into_iter()
            .find_map(|offset| self.resolved_target_at(file_id, offset))
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

        let mut documents = BTreeMap::<FileId, DocumentEdits>::new();

        for (file_id, span) in self.rename_occurrences(resolved) {
            let Some(file) = self.file_db.get_by_id(file_id) else {
                continue;
            };
            let Some(uri) = self.file_uri(file_id).cloned() else {
                continue;
            };

            let range = file.range_for_span(span);
            let new_text = shorthand_replacement(file.as_ref(), span, replacement, is_field)
                .unwrap_or_else(|| replacement.to_owned());

            documents
                .entry(file_id)
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

fn shorthand_replacement(
    file: &tolk_resolver::FileInfo,
    span: Span,
    replacement: &str,
    is_field: bool,
) -> Option<String> {
    let identifier = file.find_node_at_span(span)?;
    let argument = InstanceArg::try_from_node(identifier.parent()?).ok()?;
    if argument.has_value_separator() {
        return None;
    }
    let original = file.text(&argument.name()?);

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
