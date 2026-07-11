mod context;
mod imports;
mod items;
mod providers;
mod semantics;

use self::context::TolkCompletionContext;
use self::imports::WorkspaceCompletionData;
use super::{
    TOLK_STDLIB_DIR, TolkResolveSnapshot, TolkWorkspaceEngine, collect_embedded_stdlib_paths,
};
use crate::profiling::BufferedProfiler;
use crate::{CompletionList, DocumentSnapshot, Position, Profiler};
use std::collections::BTreeSet;

impl TolkWorkspaceEngine {
    pub(super) fn completion(
        &self,
        document: &DocumentSnapshot,
        source_file: &tolk_syntax::SourceFile,
        position: Position,
        profiler: &mut Profiler,
    ) -> anyhow::Result<CompletionList> {
        let context_started_at = profiler.start();
        let context = TolkCompletionContext::new(document, source_file, position)?;
        profiler.finish("tolk.completion.context", context_started_at);

        let workspace_started_at = profiler.start();
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
        profiler.finish("tolk.completion.workspace", workspace_started_at);

        let Some(file_id) = snapshot.find_document_file(document) else {
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
            profiler,
        ))
    }
}

pub(super) struct TolkCompletionProviderContext<'a> {
    snapshot: &'a TolkResolveSnapshot,
    file_id: tolk_resolver::FileId,
    visible_globals: tolk_resolver::symbol_resolver::GlobalEnv,
    document: &'a DocumentSnapshot,
    syntax: &'a TolkCompletionContext,
    workspace: WorkspaceCompletionData<'a>,
    profiler: &'a BufferedProfiler,
}
