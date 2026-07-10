use super::{
    TolkWorkspaceEngine, fallback_uri_for_path, import_edits, logical_path_for_uri, range_for_span,
};
use crate::{DocumentEdits, FileRename, TextEdit, WorkspaceEdit};
use std::collections::BTreeMap;
use std::path::Path;
use tolk_syntax::{AstNode, TopLevel};

impl TolkWorkspaceEngine {
    pub(super) fn will_rename_files(&self, files: &[FileRename]) -> Option<WorkspaceEdit> {
        let state = self.state.read().expect("Tolk workspace lock poisoned");
        let snapshot = state.latest_snapshot.as_ref()?;
        let renames = files
            .iter()
            .filter_map(|rename| {
                let old_path = logical_path_for_uri(&rename.old_uri);
                let new_path = logical_path_for_uri(&rename.new_uri);
                is_tolk_rename(&old_path, &new_path).then_some((old_path, new_path))
            })
            .collect::<BTreeMap<_, _>>();
        if renames.is_empty() {
            return None;
        }
        let mut documents = BTreeMap::<String, DocumentEdits>::new();

        for (&file_id, imports) in snapshot.project_index.imports() {
            let Some(importer) = snapshot.project_index.files().get(&file_id) else {
                continue;
            };
            let importer_path = renames.get(&importer.path).unwrap_or(&importer.path);
            let importer_moved = importer_path != &importer.path;
            let Some(source_file) = snapshot.file_db.get_by_id(file_id) else {
                continue;
            };

            for resolved_import in imports {
                let Some(target_id) = resolved_import.target() else {
                    continue;
                };
                let Some(target) = snapshot.project_index.files().get(&target_id) else {
                    continue;
                };
                let target_path = renames.get(&target.path).unwrap_or(&target.path);
                if !importer_moved && target_path == &target.path {
                    continue;
                }
                let Some(import_path) = import_edits::import_path_from(
                    importer_path,
                    target_path,
                    &state.project_config.stdlib_path,
                    state.project_config.import_mappings.as_ref(),
                ) else {
                    continue;
                };
                if import_path == resolved_import.import().path.as_ref() {
                    continue;
                }
                let Some(path_node) =
                    import_path_node(source_file.source(), resolved_import.import().span.start())
                else {
                    continue;
                };
                let uri = snapshot
                    .path_to_uri
                    .get(&importer.path)
                    .cloned()
                    .unwrap_or_else(|| fallback_uri_for_path(&importer.path));
                let range = range_for_span(
                    source_file.source().source.as_ref(),
                    tolk_resolver::Span::from_syntax(&path_node),
                );
                let key = uri.as_str().to_owned();
                documents
                    .entry(key)
                    .or_insert_with(|| DocumentEdits::new(uri, Vec::new()))
                    .edits
                    .push(TextEdit::new(range, format!("\"{import_path}\"")));
            }
        }

        let documents = documents.into_values().collect::<Vec<_>>();
        (!documents.is_empty()).then(|| WorkspaceEdit::new(documents))
    }

    pub(super) fn did_rename_files(&self, files: &[FileRename]) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("Tolk workspace lock poisoned");

        for rename in files {
            let old_path = logical_path_for_uri(&rename.old_uri);
            let new_path = logical_path_for_uri(&rename.new_uri);
            if !is_tolk_rename(&old_path, &new_path) {
                continue;
            }
            let Some(mut file) = state.files.remove(&old_path) else {
                continue;
            };
            if file.base_uri.is_some() {
                file.base_uri = Some(rename.new_uri.clone());
            }
            if let Some(open) = &mut file.open {
                open.uri = rename.new_uri.clone();
            }
            file.dirty = true;
            state.file_db.remove_path(&old_path);
            state.files.insert(new_path.clone(), file);
            if state.roots.remove(&old_path) {
                state.roots.insert(new_path);
            }
        }

        let mut profiler = crate::Profiler::disabled();
        state.rebuild_snapshot(&mut profiler)
    }
}

fn is_tolk_rename(old_path: &Path, new_path: &Path) -> bool {
    old_path
        .extension()
        .is_some_and(|extension| extension == "tolk")
        && new_path
            .extension()
            .is_some_and(|extension| extension == "tolk")
}

fn import_path_node(
    source_file: &tolk_syntax::SourceFile,
    import_start: usize,
) -> Option<tree_sitter::Node<'_>> {
    source_file.top_levels().find_map(|top_level| {
        let TopLevel::Import(import) = top_level else {
            return None;
        };
        if import.syntax().start_byte() != import_start {
            return None;
        }
        import.path().map(|path| path.syntax())
    })
}
