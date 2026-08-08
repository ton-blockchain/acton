use std::sync::Arc;
use tolk_resolver::project_index::ProjectIndex;
use tolk_ty::{TypeInterner, WorkspaceBodyTypes};

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub project_index: Arc<ProjectIndex>,
    pub type_interner: Arc<TypeInterner>,
    pub all_body_types: WorkspaceBodyTypes,
    pub diagnostics: Vec<tolk_linter::diagnostic::Diagnostic>,
}
