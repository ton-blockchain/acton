extern crate core;

use crate::ast::name_case_checker::check_name_cases;
use crate::ast::{
    acton_import_in_contract, bless_call_missing_safety_comment,
    dangerous_send_mode_missing_safety_comment, deprecated_symbol_use,
    negated_is_type_can_use_not_is, no_bounce_handler, several_not_null_assertions,
};
use crate::rules::ast::{
    asm_function_missing_safety_comment, field_init_can_be_folded, import_path_can_use_mappings,
    message_entity_naming, method_can_be_static, mutable_parameter_can_be_immutable,
    mutable_variable_can_be_immutable, pure_function_call_unused, reserve_mode_literal,
    send_mode_literal, unused_import, unused_variable, used_ignored_identifier,
    write_only_variable,
};
use acton_config::config::{LintEntry, LintLevel};
use rules::diagnostic::{Diagnostic, Severity};
pub use rules::*;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::{FileId, SymbolId};
use tolk_resolver::resolve_index::FileResolveIndex;
use tolk_resolver::{AstNodeSpanExt, NameUse, Resolved};
use tolk_syntax::{
    Call, Expr, ExprStmt, Ident, InstanceArg, NotNull, SourceFile, TopLevel, TypeIdent, Unary,
    Walker, walk_ast,
};
use tolk_ty::InferenceResult;
use tolk_ty::TypeDb;
use tree_sitter::Node;

#[cfg(feature = "profile_rules")]
mod profiling;
mod rules;

use crate::dfa::unauthorized_access;
#[cfg(feature = "profile_rules")]
pub use profiling::Profiler;
use tolk_analysis::{AnalysisDb, FileUseFacts};

#[cfg(feature = "profile_rules")]
macro_rules! run_rule {
    ($checker:expr, $rule:expr, $body:expr) => {{
        if $checker.should_run($rule) {
            let start = std::time::Instant::now();
            let _ = $body;
            $checker.profiler.record($rule, start.elapsed());
        }
    }};
}

#[cfg(not(feature = "profile_rules"))]
macro_rules! run_rule {
    ($checker:expr, $rule:expr, $body:expr) => {{
        if $checker.should_run($rule) {
            let _ = $body;
        }
    }};
}

pub struct Checker<'a> {
    pub file_db: &'a FileDb,
    pub type_db: &'a mut TypeDb<'a>,
    pub body_types: &'a HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    pub analysis_db: AnalysisDb,
    pub diagnostics: Vec<Diagnostic>,
    pub settings: HashMap<Rule, LintLevel>,

    /// Map from file ID to a map of line number to list of suppressed rule names/codes
    pub file_suppressions: FxHashMap<FileId, FxHashMap<usize, Vec<String>>>,
    /// Map from file ID to a list of line start byte offsets
    pub line_starts: FxHashMap<FileId, Vec<u32>>,

    #[cfg(feature = "profile_rules")]
    pub profiler: Profiler,
}

const SUPPRESSION_MARKER: &str = "acton-disable-next-line";

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
            settings: HashMap::new(),
            file_suppressions: FxHashMap::default(),
            line_starts: FxHashMap::default(),
            #[cfg(feature = "profile_rules")]
            profiler: Profiler::default(),
        }
    }

    pub fn scan_for_suppressions(&mut self, file_id: FileId, text: &str) {
        if memchr::memmem::find(text.as_bytes(), SUPPRESSION_MARKER.as_bytes()).is_none() {
            // fast path for most of the files
            return;
        }

        let mut line_starts = Vec::with_capacity(text.len() / 80);
        line_starts.push(0);
        for (i, &b) in text.as_bytes().iter().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        self.line_starts.insert(file_id, line_starts);

        let mut suppressions = FxHashMap::default();
        for (line_idx, line) in text.lines().enumerate() {
            if let Some(pos) = line.find("//") {
                let comment = &line[pos + 2..].trim();
                if let Some(marker_pos) = comment.find(SUPPRESSION_MARKER) {
                    let rules_part = comment[marker_pos + SUPPRESSION_MARKER.len()..].trim();

                    let mut rules = Vec::new();
                    for rule in rules_part.split(',') {
                        let rule = rule.trim();
                        if !rule.is_empty() {
                            rules.push(rule.to_string());
                        }
                    }

                    // we need to store suppression for the **next** line
                    suppressions.insert(line_idx + 1, rules);
                }
            }
        }

        if !suppressions.is_empty() {
            self.file_suppressions.insert(file_id, suppressions);
        }
    }

    pub fn with_settings(mut self, settings: HashMap<Rule, LintLevel>) -> Self {
        self.settings = settings;
        self
    }

    pub fn should_run(&self, rule: Rule) -> bool {
        self.settings
            .get(&rule)
            .map(|level| *level != LintLevel::Allow)
            .unwrap_or(true) // default to run
    }

    pub fn run_once(&mut self) {
        run_rule!(self, Rule::NameCaseChecker, check_name_cases(self));
    }

    pub fn emit_diagnostic(&mut self, mut diagnostic: Diagnostic) {
        if let Some(level) = self.settings.get(&diagnostic.rule) {
            match level {
                LintLevel::Allow => return,
                LintLevel::Warn => {
                    diagnostic.severity = Severity::Warning;
                }
                LintLevel::Deny => {
                    diagnostic.severity = Severity::Error;
                }
            }
        }
        self.diagnostics.push(diagnostic);
    }

    pub fn build_settings(
        config: &acton_config::config::ActonConfig,
        contract_name: Option<&str>,
    ) -> HashMap<Rule, LintLevel> {
        let mut settings = HashMap::new();

        settings.insert(Rule::UnauthorizedAccess, LintLevel::Allow); // disabled by default for now

        let Some(lint) = config.lint.as_ref().and_then(|lint| lint.rules.as_ref()) else {
            return settings;
        };

        // 1. Apply global settings
        for (name, entry) in &lint.entries {
            if let LintEntry::Level(level) = entry
                && let Some(rule) = find_rule_by_name(name)
            {
                settings.insert(rule, level.clone());
            }
        }

        // 2. Apply contract overrides
        if let Some(contract_name) = contract_name
            && let Some(LintEntry::Config(override_settings)) = lint.entries.get(contract_name)
        {
            for (name, level) in override_settings {
                if let Some(rule) = find_rule_by_name(name) {
                    settings.insert(rule, level.clone());
                }
            }
        }

        settings
    }

    pub fn resolve_index_for(&self, file_id: FileId) -> Option<Arc<FileResolveIndex>> {
        self.type_db
            .project_index
            .resolved_uses
            .get(&file_id)
            .cloned()
    }

    pub fn global_usages_of(&self, symbol_id: SymbolId) -> impl Iterator<Item = &NameUse> {
        self.type_db
            .project_index
            .resolved_uses
            .values()
            .flat_map(move |v| v.global_usages_of(symbol_id))
    }

    pub fn use_facts(&mut self, file_id: FileId) -> Option<Arc<FileUseFacts>> {
        self.analysis_db
            .use_facts(self.type_db, self.body_types, file_id)
    }

    pub fn apply_suppressions(&mut self) {
        self.diagnostics.retain(|diag| {
            let Some(file_suppressions) = self.file_suppressions.get(&diag.file_id) else {
                // fast path for most of the files
                return true;
            };

            let Some(line_starts) = self.line_starts.get(&diag.file_id) else {
                return true;
            };

            let span = if let Some(primary) = diag.annotations.iter().find(|a| a.is_primary) {
                primary.span
            } else if let Some(first) = diag.annotations.first() {
                first.span
            } else {
                return true;
            };

            let line_idx = line_starts
                .binary_search(&span.start)
                .unwrap_or_else(|idx| idx - 1);

            if let Some(suppressed_rules) = file_suppressions.get(&line_idx) {
                if suppressed_rules.iter().any(|r| r == "all") {
                    return false;
                }

                if suppressed_rules.iter().any(|r| r == diag.name) {
                    return false;
                }
            }

            true
        });
    }

    pub fn process_file(&mut self, file: &SourceFile, file_id: FileId) {
        self.scan_for_suppressions(file_id, file.source.as_ref());
        self.use_facts(file_id);
        let resolve_index = self.resolve_index_for(file_id);
        let mut walker = CheckerWalker {
            checker: self,
            file_id,
            resolve_index,
            current_inference: None,
            current_decl: None,
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

fn find_rule_by_name(name: &str) -> Option<Rule> {
    Linter::Tolk.all_rules().find(|r| r.name() == name)
}

#[cfg(feature = "profile_rules")]
macro_rules! run_rule {
    ($checker:expr, $rule:expr, $body:expr) => {{
        if $checker.should_run($rule) {
            let start = std::time::Instant::now();
            let _ = $body;
            $checker.profiler.record($rule, start.elapsed());
        }
    }};
}

#[cfg(not(feature = "profile_rules"))]
macro_rules! run_rule {
    ($checker:expr, $rule:expr, $body:expr) => {{
        if $checker.should_run($rule) {
            let _ = $body;
        }
    }};
}

struct CheckerWalker<'a, 'b> {
    checker: &'a mut Checker<'b>,
    file_id: FileId,
    resolve_index: Option<Arc<FileResolveIndex>>,
    current_inference: Option<&'b InferenceResult>,
    current_decl: Option<SymbolId>,
}

impl<'a, 'b, 'file> Walker<'file> for CheckerWalker<'a, 'b> {
    type Result = ();

    fn visit_top_level(&mut self, top_level: &TopLevel<'file>) -> Self::Result {
        let prev_inference = self.current_inference;
        let prev_decl = self.current_decl;

        if let Some(file_info) = self.checker.file_db.get_by_id(self.file_id)
            && let Some(symbol) = file_info.find_declaration(top_level)
        {
            self.current_decl = Some(symbol.id);

            if let Some(file_body_types) = self.checker.body_types.get(&self.file_id)
                && let Some(inference) = file_body_types.get(&symbol.id)
            {
                self.current_inference = Some(inference);
            }
        }

        self.walk_top_level(top_level);

        self.current_inference = prev_inference;
        self.current_decl = prev_decl;
    }

    fn walk_source_file(&mut self, source_file: &'file SourceFile) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::MutableVariableCanBeImmutable,
            mutable_variable_can_be_immutable::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::MutableParameterCanBeImmutable,
            mutable_parameter_can_be_immutable::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::UnusedImport,
            unused_import::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::ImportPathCanUseMappings,
            import_path_can_use_mappings::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::ActonImportInContract,
            acton_import_in_contract::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::AsmFunctionMissingSafetyComment,
            asm_function_missing_safety_comment::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::UsedIgnoredIdentifier,
            used_ignored_identifier::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::MessageShouldBeNamed,
            message_entity_naming::check_file_for_message_name(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::UnauthorizedAccess,
            unauthorized_access::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::MethodCanBeStatic,
            method_can_be_static::check_file(self.checker, self.file_id)
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

    fn walk_expr_stmt(&mut self, node: &ExprStmt<'file>) -> Self::Result {
        if let Some(expr) = node.expr() {
            if let Expr::Call(call) = &expr {
                run_rule!(
                    self.checker,
                    Rule::PureFunctionCallUnused,
                    pure_function_call_unused::check_expr_stmt_call(
                        self.checker,
                        self.file_id,
                        call,
                        self.current_inference
                    )
                );
            }
            self.visit_expr(&expr);
        }
    }

    fn walk_ident(&mut self, node: &Ident<'file>) -> Self::Result {
        self.resolve_ident_and_run_inspections(&node.0)
    }

    fn walk_type_ident(&mut self, node: &TypeIdent<'file>) -> Self::Result {
        self.resolve_ident_and_run_inspections(&node.0)
    }

    fn walk_instance_arg(&mut self, node: &InstanceArg<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::FieldInitCanBeFolded,
            field_init_can_be_folded::check_instance_arg(self.checker, self.file_id, node)
        );

        if let Some(value) = node.value() {
            self.visit_expr(&value);
        }
    }

    fn walk_call(&mut self, node: &Call<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::NoBounceHandler,
            no_bounce_handler::check_call_expr(self.checker, self.file_id, node, self.current_decl)
        );

        if let Some(inference) = self.current_inference {
            run_rule!(
                self.checker,
                Rule::CreateMessageInlineSend,
                message_entity_naming::check_call_for_inline_send(
                    self.checker,
                    self.file_id,
                    node,
                    inference
                )
            );

            run_rule!(
                self.checker,
                Rule::SendModeLiteral,
                send_mode_literal::check_call(self.checker, self.file_id, node, Some(inference))
            );

            run_rule!(
                self.checker,
                Rule::ReserveModeLiteral,
                reserve_mode_literal::check_call(self.checker, self.file_id, node, Some(inference))
            );

            run_rule!(
                self.checker,
                Rule::DangerousSendModeMissingSafetyComment,
                dangerous_send_mode_missing_safety_comment::check_call(
                    self.checker,
                    self.file_id,
                    node,
                    Some(inference)
                )
            );

            run_rule!(
                self.checker,
                Rule::BlessCallMissingSafetyComment,
                bless_call_missing_safety_comment::check_call(
                    self.checker,
                    self.file_id,
                    node,
                    Some(inference)
                )
            );
        } else {
            run_rule!(
                self.checker,
                Rule::SendModeLiteral,
                send_mode_literal::check_call(self.checker, self.file_id, node, None)
            );

            run_rule!(
                self.checker,
                Rule::ReserveModeLiteral,
                reserve_mode_literal::check_call(self.checker, self.file_id, node, None)
            );

            run_rule!(
                self.checker,
                Rule::DangerousSendModeMissingSafetyComment,
                dangerous_send_mode_missing_safety_comment::check_call(
                    self.checker,
                    self.file_id,
                    node,
                    None
                )
            );

            run_rule!(
                self.checker,
                Rule::BlessCallMissingSafetyComment,
                bless_call_missing_safety_comment::check_call(
                    self.checker,
                    self.file_id,
                    node,
                    None
                )
            );
        }

        if let Some(callee) = node.callee() {
            self.visit_expr(&callee);
        }
        for arg in node.arguments() {
            self.walk_call_argument(&arg);
        }
        self.default_result()
    }

    fn walk_unary(&mut self, node: &Unary<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::NegatedIsTypeCanUseNotIs,
            negated_is_type_can_use_not_is::check_unary(self.checker, self.file_id, node)
        );

        if let Some(argument) = node.argument() {
            self.visit_expr(&argument);
        }
        self.default_result()
    }

    fn walk_not_null(&mut self, node: &NotNull<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::SeveralNotNullAssertions,
            several_not_null_assertions::check_not_null(self.checker, self.file_id, node)
        );

        if let Some(inner) = node.inner() {
            self.visit_expr(&inner);
        }
        self.default_result()
    }

    fn default_result(&self) -> Self::Result {}
}

impl<'a, 'b> CheckerWalker<'a, 'b> {
    fn resolve_ident_and_run_inspections(&mut self, node: &Node) {
        let Some(resolve_index) = &self.resolve_index else {
            return;
        };

        let node_span = node.span();
        let usage = if let Some(usage) = resolve_index.find_use(node_span.start()) {
            usage
        } else if let Some(inference) = self.current_inference
            && let Some(usage) = inference.resolve(node_span)
        {
            usage
        } else {
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
                    symbol,
                )
            );
        }
    }
}
