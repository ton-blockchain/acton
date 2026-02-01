use crate::AnalysisResult;
use crate::backend::Backend;
use crate::backend::utils::{FileInfoExt, offset_to_range};
use dashmap::mapref::one::Ref;
use lsp_types::*;
use std::sync::Arc;
use tolk_resolver::resolve_index::LocalDefId;
use tolk_resolver::{FileInfo, FileResolveIndex, Resolved, SymbolId};
use tower_lsp::jsonrpc::Result as LspResult;

impl Backend {
    pub async fn handle_references(
        &self,
        params: ReferenceParams,
    ) -> LspResult<Option<Vec<Location>>> {
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
        let resolved_uses = analysis.project_index.get_resolved_uses(file.id())?;

        match resolved {
            Resolved::Global(symbol_id) => self.global_references(analysis, symbol_id),
            Resolved::Local(local_id) => self.local_refences(uri, &file, resolved_uses, local_id),
            Resolved::Unresolved => None,
        }
    }

    fn local_refences(
        &self,
        uri: &Url,
        file: &Arc<FileInfo>,
        resolved_uses: &Arc<FileResolveIndex>,
        local_id: LocalDefId,
    ) -> Option<Vec<Location>> {
        let locations = resolved_uses
            .local_usages_of(local_id)
            .map(|usage| {
                let range = offset_to_range(file, usage.span.start());
                Location::new(uri.clone(), range)
            })
            .collect::<Vec<_>>();
        Some(locations)
    }

    fn global_references(
        &self,
        analysis: Ref<Url, Arc<AnalysisResult>>,
        symbol_id: SymbolId,
    ) -> Option<Vec<Location>> {
        let usages = analysis
            .project_index
            .resolved_uses
            .iter()
            .map(|(file_id, index)| (file_id, index.global_usages_of(symbol_id)));

        let locations = usages
            .flat_map(|(&file_id, usages)| {
                let file_info = self.file_db.get_by_id(file_id)?;
                let url = self.get_file_url(file_id, &file_info)?;
                let locations = usages
                    .map(|usage| {
                        let range = offset_to_range(&file_info, usage.span.start());
                        Location::new(url.clone(), range)
                    })
                    .collect::<Vec<_>>();
                Some(locations)
            })
            .flatten()
            .collect::<Vec<_>>();
        Some(locations)
    }
}
