use crate::ast::deprecated_symbol_use;
use crate::rules::ast::{
    field_init_can_be_folded, mutable_variable_can_be_immutable, unused_import, unused_variable,
    write_only_variable,
};
use rules::diagnostic::Diagnostic;
pub use rules::*;
use std::collections::HashMap;
use std::sync::Arc;
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::{FileId, SymbolId};
use tolk_resolver::resolve_index::FileResolveIndex;
use tolk_resolver::{AstNodeSpanExt, Resolved};
use tolk_syntax::{Ident, ObjectLit, SourceFile, TypeIdent, Walker, walk_ast};
use tolk_ty::InferenceResult;
use tolk_ty::TypeDb;
use tree_sitter::Node;

#[cfg(feature = "profile_rules")]
mod profiling;
mod rules;

#[cfg(feature = "profile_rules")]
pub use profiling::Profiler;
use tolk_analysis::{AnalysisDb, FileUseFacts};

pub struct Checker<'a> {
    pub file_db: &'a FileDb,
    pub type_db: &'a mut TypeDb<'a>,
    pub body_types: &'a HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    pub analysis_db: AnalysisDb,
    pub diagnostics: Vec<Diagnostic>,

    #[cfg(feature = "profile_rules")]
    pub profiler: Profiler,
}

impl<'a> Checker<'a> {
    pub fn new(
        file_db: &'a FileDb,
        type_db: &'a mut TypeDb<'a>,
        body_types: &'a HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    ) -> Self {
        Self {
            file_db,
            type_db,
            body_types,
            analysis_db: AnalysisDb::new(),
            diagnostics: Vec::new(),
            #[cfg(feature = "profile_rules")]
            profiler: Profiler::default(),
        }
    }

    pub fn resolve_index_for(&self, file_id: FileId) -> Option<Arc<FileResolveIndex>> {
        self.type_db
            .project_index
            .resolved_uses
            .get(&file_id)
            .cloned()
    }

    pub fn use_facts(&mut self, file_id: FileId) -> Option<Arc<FileUseFacts>> {
        self.analysis_db
            .use_facts(self.type_db, self.body_types, file_id)
    }

    pub fn process_file(&mut self, file: &SourceFile, file_id: FileId) {
        self.use_facts(file_id);
        let resolve_index = self.resolve_index_for(file_id);
        let mut walker = CheckerWalker {
            checker: self,
            file_id,
            resolve_index,
        };

        walk_ast(&mut walker, file);
    }

    #[cfg(feature = "profile_rules")]
    pub fn print_profiling_results(&self) {
        let mut rules: Vec<_> = self.profiler.rules.iter().collect();
        rules.sort_by_key(|(_, stats)| std::cmp::Reverse(stats.total));

        println!("\nRule profiling results:");
        println!("{:<40} {:>10} {:>15}", "Rule", "Calls", "Total Time");
        println!("{:-<67}", "");
        for (rule, stats) in rules {
            println!(
                "{:<40} {:>10} {:>15?}",
                format!("{:?}", rule),
                stats.calls,
                stats.total
            );
        }
        println!();
    }
}

#[cfg(feature = "profile_rules")]
macro_rules! run_rule {
    ($checker:expr, $rule:expr, $body:expr) => {{
        let start = std::time::Instant::now();
        let result = $body;
        $checker.profiler.record($rule, start.elapsed());
        result
    }};
}

#[cfg(not(feature = "profile_rules"))]
macro_rules! run_rule {
    ($checker:expr, $rule:expr, $body:expr) => {{
        let _ = $rule; // avoid unused warnings
        $body
    }};
}

struct CheckerWalker<'a, 'b> {
    checker: &'a mut Checker<'b>,
    file_id: FileId,
    resolve_index: Option<Arc<FileResolveIndex>>,
}

impl<'a, 'b, 'file> Walker<'file> for CheckerWalker<'a, 'b> {
    type Result = ();

    fn walk_source_file(&mut self, source_file: &'file SourceFile) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::MutableVariableCanBeImmutable,
            mutable_variable_can_be_immutable::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::UnusedImport,
            unused_import::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::UnusedVariable,
            unused_variable::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::WriteOnlyVariable,
            write_only_variable::check_file(self.checker, self.file_id)
        );

        for top_level in source_file.top_levels() {
            self.visit_top_level(&top_level);
        }
        self.default_result()
    }

    fn walk_object_lit(&mut self, node: &ObjectLit<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::FieldInitCanBeFolded,
            field_init_can_be_folded::check_struct_literal(self.checker, self.file_id, node)
        );

        if let Some(object_type) = node.typ() {
            self.visit_type(&object_type);
        }
        for arg in node.arguments() {
            self.walk_instance_arg(&arg);
        }
    }

    fn walk_ident(&mut self, node: &Ident<'file>) -> Self::Result {
        self.resolve_ident_and_run_inspections(&node.0)
    }

    fn walk_type_ident(&mut self, node: &TypeIdent<'file>) -> Self::Result {
        self.resolve_ident_and_run_inspections(&node.0)
    }

    fn default_result(&self) -> Self::Result {}
}

impl<'a, 'b> CheckerWalker<'a, 'b> {
    fn resolve_ident_and_run_inspections(&mut self, node: &Node) {
        let Some(resolve_index) = &self.resolve_index else {
            return;
        };

        let Some(usage) = resolve_index.find_use(node.span().start()) else {
            return;
        };

        if let Resolved::Global(resolved) = usage.resolved
            && let Some(symbol) = self.checker.type_db.project_index.resolve_symbol(resolved)
        {
            run_rule!(
                self.checker,
                Rule::DeprecatedSymbolUse,
                deprecated_symbol_use::check_resolved_reference(
                    self.checker,
                    self.file_id,
                    node,
                    &symbol,
                )
            );
        }
    }
}
