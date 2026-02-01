use crate::backend::Backend;
use crate::backend::analysis::AnalysisResult;
use tolk_resolver::{FileInfo, Resolved, Span};

impl Backend {
    pub fn resolve_symbol_at(
        &self,
        analysis: &AnalysisResult,
        file_info: &FileInfo,
        offset: usize,
    ) -> Option<Resolved> {
        crate::profile!(self, "resolve_symbol_at");
        let file_id = file_info.id();
        let resolved_uses = analysis.project_index.get_resolved_uses(file_id)?;

        // Maybe we point to usage of some symbol
        if let Some(usage) = resolved_uses.find_use(offset) {
            return Some(usage.resolved.clone());
        }

        // Maybe we point to name of some global declaration
        if let Some(global_symbol) = analysis.project_index.find_symbol_at(file_id, offset) {
            return Some(Resolved::Global(global_symbol.id));
        }

        // Maybe we point to name of some local variable
        if let Some(local_def) = resolved_uses.find_local_at(offset) {
            return Some(Resolved::Local(local_def.id));
        }

        let symbol = file_info.find_symbol_at(offset)?;
        let inferences = analysis.all_body_types.get(&file_info.id())?;

        let inference = inferences.get(&symbol.id)?;
        if let Some(resolved) = inference.resolve(Span::from_offset(offset)) {
            return Some(resolved.resolved.clone());
        }

        None
    }
}
