use crate::rules::ast::{
    field_init_can_be_folded, mutable_variable_can_be_immutable, unused_variable,
};
use rules::diagnostic::Diagnostic;
use std::collections::HashMap;
use std::sync::Arc;
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::{FileId, SymbolId};
use tolk_resolver::resolve_index::FileResolveIndex;
use tolk_syntax::{ObjectLit, SourceFile, Walker, walk_ast};
use tolk_ty::InferenceResult;
use tolk_ty::TypeDb;

mod rules;
pub use rules::*;

pub struct Checker<'a> {
    pub file_db: &'a FileDb,
    pub type_db: &'a mut TypeDb<'a>,
    pub body_types: &'a HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    pub fn resolve_index_for(&self, file_id: FileId) -> Option<Arc<FileResolveIndex>> {
        self.type_db
            .project_index
            .resolved_uses
            .get(&file_id)
            .cloned()
    }

    pub fn process_file(&mut self, file: &SourceFile, file_id: FileId) {
        let mut walker = CheckerWalker {
            checker: self,
            file_id,
        };

        walk_ast(&mut walker, file);
    }
}

struct CheckerWalker<'a, 'b> {
    checker: &'a mut Checker<'b>,
    file_id: FileId,
}

impl<'a, 'b, 'file> Walker<'file> for CheckerWalker<'a, 'b> {
    type Result = ();

    fn walk_source_file(&mut self, source_file: &'file SourceFile) -> Self::Result {
        mutable_variable_can_be_immutable::check_file(self.checker, self.file_id);
        unused_variable::check_file(self.checker, self.file_id);

        for top_level in source_file.top_levels() {
            self.visit_top_level(&top_level);
        }
        self.default_result()
    }

    fn walk_object_lit(&mut self, node: &ObjectLit<'file>) -> Self::Result {
        field_init_can_be_folded::check_struct_literal(self.checker, self.file_id, node);

        if let Some(object_type) = node.typ() {
            self.visit_type(&object_type);
        }
        for arg in node.arguments() {
            self.walk_instance_arg(&arg);
        }
    }

    fn default_result(&self) -> Self::Result {}
}
