mod context;
mod contract;
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
use rustc_hash::FxHashMap;
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};
use tolk_resolver::{FileId, resolve_files};
use tolk_ty::{FileBodyTypes, TypeDb, infer};

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

        let speculative_snapshot = completion_snapshot(
            &snapshot,
            file_id,
            context.source_file(),
            context.offset,
            profiler,
        );
        let snapshot = speculative_snapshot
            .as_ref()
            .unwrap_or_else(|| snapshot.as_ref());

        Ok(providers::collect(
            snapshot,
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

/// Builds query-local semantics for the source rewritten with the completion identifier.
///
/// A trailing dot is not a valid expression in the live document. Resolving the rewritten
/// syntax against the live snapshot would therefore mix nodes from one tree with inference
/// from another. The speculative branch preserves all stable IDs and only resolves and infers
/// the declaration that contains the cursor.
fn completion_snapshot(
    base: &TolkResolveSnapshot,
    file_id: FileId,
    source_file: &tolk_syntax::SourceFile,
    offset: usize,
    profiler: &mut Profiler,
) -> Option<TolkResolveSnapshot> {
    let original_file = base.file_db.get_by_id(file_id)?;

    let fork_started_at = profiler.start();
    let file_db = Arc::new(base.file_db.fork());
    let temporary_file =
        file_db.process_source_file(original_file.path().clone(), source_file.clone());
    profiler.finish("tolk.completion.snapshot.fork", fork_started_at);

    if temporary_file.id() != file_id {
        profiler.increment("tolk.completion.snapshot.fallback");
        return None;
    }

    let changed_file_ids = BTreeSet::from([file_id]);
    let index_started_at = profiler.start();
    let Some(mut project_index) = base
        .project_index
        .with_updated_files(&file_db, &changed_file_ids)
    else {
        profiler.finish("tolk.completion.snapshot.index", index_started_at);
        profiler.increment("tolk.completion.snapshot.fallback");
        return None;
    };
    project_index.reuse_resolved_uses_from(&base.project_index, &changed_file_ids);
    profiler.finish("tolk.completion.snapshot.index", index_started_at);

    let resolve_started_at = profiler.start();
    let mut unresolved_files = project_index
        .files()
        .keys()
        .filter(|file_id| !project_index.resolved_uses().contains_key(file_id))
        .copied()
        .collect::<Vec<_>>();
    unresolved_files.sort_unstable();
    resolve_files(&file_db, &mut project_index, unresolved_files);
    profiler.finish("tolk.completion.snapshot.resolve", resolve_started_at);

    let inference_started_at = profiler.start();
    let mut type_interner = base.type_interner.as_ref().clone();
    let mut type_db = TypeDb::new_for_query(
        &mut type_interner,
        &file_db,
        &project_index,
        &base.type_db_cache,
    );
    let mut body_types = FileBodyTypes::default();

    if let Some(symbol) = temporary_file.find_symbol_at(offset)
        && let Some(declaration) = temporary_file.find_syntax_declaration(symbol.id)
    {
        let inference = infer(&mut type_db, file_id, symbol.id, &declaration);
        body_types.insert(symbol.id, inference);
        profiler.increment("tolk.completion.type_inference.declaration");
    }

    let type_db_cache = type_db.into_cache();
    profiler.finish(
        "tolk.completion.snapshot.type_inference",
        inference_started_at,
    );

    Some(TolkResolveSnapshot {
        generation: base.generation,
        file_db,
        project_index: Arc::new(project_index),
        all_body_types: base.all_body_types.clone(),
        body_types_override: Some((file_id, Arc::new(body_types))),
        use_facts: RwLock::new(FxHashMap::default()),
        type_interner: Arc::new(type_interner),
        type_db_cache: Arc::new(type_db_cache),
        file_uris: base.file_uris.clone(),
    })
}

pub(super) struct TolkCompletionProviderContext<'a> {
    snapshot: &'a TolkResolveSnapshot,
    file_id: FileId,
    visible_globals: tolk_resolver::symbol_resolver::GlobalEnv,
    document: &'a DocumentSnapshot,
    syntax: &'a TolkCompletionContext,
    workspace: WorkspaceCompletionData<'a>,
    profiler: &'a BufferedProfiler,
}
