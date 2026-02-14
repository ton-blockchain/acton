use crate::AnalysisResult;
use crate::backend::Backend;
use crate::backend::utils::{FileInfoExt, SpanExt};
use dashmap::mapref::one::Ref;
use lsp_types::*;
use std::sync::Arc;
use tolk_resolver::resolve_index::LocalDefId;
use tolk_resolver::{FileInfo, Resolved, SymbolId};
use tower_lsp::jsonrpc::Result as LspResult;

impl Backend {
    pub async fn handle_references(
        &self,
        params: ReferenceParams,
    ) -> LspResult<Option<Vec<Location>>> {
        crate::profile!(self, "references");
        let now = std::time::Instant::now();
        let uri = params.text_document_position.text_document.uri.clone();
        log::info!("Request: goto_references for {}", uri);

        let position = params.text_document_position.position;
        let result = self.references(&uri, position);

        log::info!(
            "Response: goto_references took {:?}, found {} references",
            now.elapsed(),
            result.as_ref().map(|v| v.len()).unwrap_or(0)
        );
        Ok(result)
    }

    fn references(&self, uri: &Url, position: Position) -> Option<Vec<Location>> {
        let analysis = self.analysis.get(uri)?;
        let path = uri.to_file_path().ok()?;
        let file = self.file_db.get_by_path(&path)?;

        let offset = file.position_to_offset(position)?;
        let resolved = self.resolve_symbol_at(&analysis, &file, offset)?;

        match resolved {
            Resolved::Global(symbol_id) => self.global_references(analysis, symbol_id),
            Resolved::Local(local_id) => self.local_refences(analysis, uri, &file, local_id),
            Resolved::Unresolved => None,
        }
    }

    fn local_refences(
        &self,
        analysis: Ref<Url, Arc<AnalysisResult>>,
        uri: &Url,
        file: &Arc<FileInfo>,
        local_id: LocalDefId,
    ) -> Option<Vec<Location>> {
        let resolved_uses = analysis.project_index.get_resolved_uses(file.id())?;
        let locations = resolved_uses
            .local_usages_of(local_id)
            .map(|usage| Location::new(uri.clone(), usage.span.range(file)))
            .collect::<Vec<_>>();
        Some(locations)
    }

    fn global_references(
        &self,
        analysis: Ref<Url, Arc<AnalysisResult>>,
        symbol_id: SymbolId,
    ) -> Option<Vec<Location>> {
        // Typically, a project can have dozens or even hundreds of files (the Tolk standard library,
        // the Acton standard library, test files, scripts, contract files), but a global symbol is typically
        // used across few files.
        //
        // We find files that directly import the file with the definition and search only
        // within them, which speeds up the search by orders of magnitude.
        let dependents = analysis.project_index.direct_dependents(symbol_id.file_id);

        let locations = dependents
            .into_iter()
            .filter_map(|file_id| {
                let index = analysis.project_index.get_resolved_uses(file_id)?;
                let file = self.file_db.get_by_id(file_id)?;
                let url = self.get_file_url(&file)?;

                let locations = index
                    .global_usages_of(symbol_id)
                    .map(|usage| Location::new(url.clone(), usage.span.range(&file)))
                    .collect::<Vec<_>>();
                Some(locations)
            })
            .flatten()
            .collect::<Vec<_>>();
        Some(locations)
    }
}
