mod context;
mod imports;
mod items;
mod providers;
mod semantics;

use self::context::TolkCompletionContext;
use self::imports::WorkspaceCompletionData;
use super::{
    TOLK_STDLIB_DIR, TolkResolveSnapshot, TolkWorkspaceEngine, collect_embedded_stdlib_paths,
    logical_path_for_uri,
};
use crate::{CompletionList, DocumentSnapshot, Position};
use std::collections::BTreeSet;

impl TolkWorkspaceEngine {
    pub(super) fn completion(
        &self,
        document: &DocumentSnapshot,
        position: Position,
    ) -> anyhow::Result<CompletionList> {
        let context = TolkCompletionContext::new(document, position)?;
        let (snapshot, paths, stdlib_path, mappings, contract_ids, wallet_names) = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            let Some(snapshot) = state.latest_snapshot.clone() else {
                return Ok(CompletionList::default());
            };
            let mut paths = state.files.keys().cloned().collect::<BTreeSet<_>>();
            paths.extend(
                snapshot
                    .project_index
                    .files()
                    .values()
                    .map(|file| file.path.clone()),
            );
            if state.project_config.use_embedded_stdlib {
                collect_embedded_stdlib_paths(&TOLK_STDLIB_DIR, &mut paths);
            }
            (
                snapshot,
                paths.into_iter().collect::<Vec<_>>(),
                state.project_config.stdlib_path.clone(),
                state.project_config.import_mappings.clone(),
                state.project_config.contract_ids.clone(),
                state.project_config.wallet_names.clone(),
            )
        };
        let path = logical_path_for_uri(document.uri());
        let Some(file_id) = snapshot.project_index.get_file_by_path(&path) else {
            return Ok(CompletionList::default());
        };

        Ok(providers::collect(
            &snapshot,
            file_id,
            document,
            &context,
            WorkspaceCompletionData {
                paths: &paths,
                stdlib_path: &stdlib_path,
                mappings: mappings.as_ref(),
                contract_ids: &contract_ids,
                wallet_names: &wallet_names,
            },
        ))
    }
}

pub(super) struct TolkCompletionProviderContext<'a> {
    snapshot: &'a TolkResolveSnapshot,
    file_id: tolk_resolver::FileId,
    document: &'a DocumentSnapshot,
    syntax: &'a TolkCompletionContext,
    workspace: WorkspaceCompletionData<'a>,
}
