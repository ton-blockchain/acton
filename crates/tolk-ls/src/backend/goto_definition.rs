use crate::backend::Backend;
use crate::backend::utils::{FileInfoExt, SpanExt};
use lsp_types::*;
use tolk_resolver::Resolved;
use tower_lsp::jsonrpc::Result as LspResult;

impl Backend {
    pub async fn handle_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let now = std::time::Instant::now();
        let uri = params.text_document_position_params.text_document.uri;
        log::info!("Request: goto_definition for {}", uri);

        let position = params.text_document_position_params.position;

        let result = self.definition(&uri, position);

        log::info!("Response: goto_definition took {:?}", now.elapsed());
        Ok(result)
    }

    fn definition(&self, uri: &Url, position: Position) -> Option<GotoDefinitionResponse> {
        let analysis = self.analysis.get(uri)?;
        let path = uri.to_file_path().ok()?;
        let file_info = self.file_db.get_by_path(&path)?;

        let offset = file_info.position_to_offset(position)?;
        let resolved = self.resolve_symbol_at(&analysis, &file_info, offset)?;

        match resolved {
            Resolved::Global(symbol_id) => {
                let symbol = analysis.project_index.resolve_symbol(symbol_id)?;
                let target_info = self.file_db.get_by_id(symbol_id.file_id)?;
                let target_uri = self.get_file_url(symbol_id.file_id, &target_info)?;
                let range = symbol.name_span.start_range(&target_info);
                Some(GotoDefinitionResponse::Scalar(Location::new(
                    target_uri, range,
                )))
            }
            Resolved::Local(local_id) => {
                let resolved_uses = analysis.project_index.get_resolved_uses(file_info.id())?;
                let local = resolved_uses.find_local(local_id)?;
                let target_info = self.file_db.get_by_id(local_id.file_id)?;
                let target_uri = self.get_file_url(local_id.file_id, &target_info)?;
                let range = local.def_span.start_range(&target_info);
                Some(GotoDefinitionResponse::Scalar(Location::new(
                    target_uri, range,
                )))
            }
            Resolved::Unresolved => None,
        }
    }
}
