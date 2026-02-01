use lsp_types::*;
use tower_lsp::jsonrpc::Result as LspResult;
use crate::backend::Backend;
use crate::backend::analysis::AnalysisResult;
use crate::backend::utils::FileInfoExt;

impl Backend {
    pub async fn handle_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let now = std::time::Instant::now();
        let uri = params.text_document_position_params.text_document.uri;
        log::info!("Request: goto_definition for {}", uri);

        let position = params.text_document_position_params.position;

        let result = (|| {
            let analysis = self.analysis.get(&uri)?;
            let path = uri.to_file_path().ok()?;
            let file_info = self.file_db.get_by_path(&path)?;
            let file_id = file_info.id();

            let offsets = file_info.line_offsets();
            let offset = (*offsets.get(position.line as usize)?) + position.character as usize;

            if let Some(body_types) = analysis.all_body_types.get(&file_id) {
                for results in body_types.values() {
                    if let Ok(idx) = results.resolved_refs.binary_search_by(|u| {
                        if (offset as u32) < u.span.start {
                            std::cmp::Ordering::Greater
                        } else if (offset as u32) >= u.span.end {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    }) {
                        return self.resolve_to_location(&results.resolved_refs[idx], &analysis);
                    }
                }
            }

            // Search in project index (binary search inside find_use)
            if let Some(name_use) = analysis.project_index.find_use(file_id, offset)
                && let Some(res) = self.resolve_to_location(name_use, &analysis)
            {
                return Some(res);
            }

            None
        })();

        log::info!("Response: goto_definition took {:?}", now.elapsed());
        Ok(result)
    }

    pub fn resolve_to_location(
        &self,
        name_use: &tolk_resolver::resolve_index::NameUse,
        analysis: &AnalysisResult,
    ) -> Option<GotoDefinitionResponse> {
        match name_use.resolved {
            tolk_resolver::resolve_index::Resolved::Local(def_id) => {
                let target_info = self.file_db.get_by_id(def_id.file_id)?;
                let target_uri = self
                    .file_urls
                    .entry(def_id.file_id)
                    .or_insert_with(|| target_info.url().unwrap())
                    .clone();
                let range = crate::backend::utils::offset_to_range(&target_info, def_id.local as usize);
                Some(GotoDefinitionResponse::Scalar(Location::new(
                    target_uri, range,
                )))
            }
            tolk_resolver::resolve_index::Resolved::Global(sym_id) => {
                let symbol = analysis.project_index.resolve_symbol(sym_id)?;
                let target_info = self.file_db.get_by_id(sym_id.file_id)?;
                let target_uri = self
                    .file_urls
                    .entry(sym_id.file_id)
                    .or_insert_with(|| target_info.url().unwrap())
                    .clone();
                let range = crate::backend::utils::offset_to_range(&target_info, symbol.name_span.start());
                Some(GotoDefinitionResponse::Scalar(Location::new(
                    target_uri, range,
                )))
            }
            _ => None,
        }
    }
}
