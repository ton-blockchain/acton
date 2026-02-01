use crate::backend::Backend;
use crate::backend::utils::{FileInfoExt, offset_to_range};
use lsp_types::*;
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
        let file_info = self.file_db.get_by_path(&path)?;
        let file_id = file_info.id();

        let offsets = file_info.line_offsets();
        let offset = (*offsets.get(position.line as usize)?) + position.character as usize;

        let mut locations = Vec::new();

        let global_symbol = analysis
            .project_index
            .files()
            .get(&file_id)
            .and_then(|f| f.decls.iter().find(|decl| decl.name_span.contains(offset)));

        if let Some(global_symbol) = global_symbol {
            for (fid, file_resolve_index) in analysis.project_index.resolved_uses().iter() {
                for usage in &file_resolve_index.uses {
                    if let tolk_resolver::resolve_index::Resolved::Global(usage_symbol_id) =
                        &usage.resolved
                        && usage_symbol_id == &global_symbol.id
                        && let Some(file_info) = self.file_db.get_by_id(*fid)
                        && let Some(url) = file_info.url()
                    {
                        let range = offset_to_range(&file_info, usage.span.start());
                        locations.push(Location::new(url, range));
                    }
                }
            }
        } else {
            let local_symbol_info = analysis
                .project_index
                .resolved_uses()
                .get(&file_id)?
                .locals
                .iter()
                .find(|local| local.def_span.contains(offset));

            if let Some(local_def) = local_symbol_info {
                for (fid, file_resolve_index) in analysis.project_index.resolved_uses().iter() {
                    for usage in &file_resolve_index.uses {
                        if let tolk_resolver::resolve_index::Resolved::Local(usage_symbol_id) =
                            &usage.resolved
                            && usage_symbol_id == &local_def.id
                            && let Some(file_info) = self.file_db.get_by_id(*fid)
                            && let Some(url) = file_info.url()
                        {
                            let range = offset_to_range(&file_info, usage.span.start());
                            locations.push(Location::new(url, range));
                        }
                    }
                }
            }
        }

        Some(locations)
    }
}
