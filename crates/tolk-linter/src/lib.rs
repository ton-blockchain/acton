extern crate core;

use crate::ast::name_case_checker::check_name_cases;
use crate::ast::{
    acton_import_in_contract, bless_call_missing_safety_comment,
    dangerous_send_mode_missing_safety_comment, deprecated_symbol_use, duplicated_condition,
    enum_cast_missing_safety_comment, explicit_return_type, identical_conditional_branches,
    incoming_messages_duplicate_opcode, missing_contract_header, negated_is_type_can_use_not_is,
    no_bounce_handler, no_global_variables, several_not_null_assertions,
};
use crate::rules::ast::{
    asm_function_missing_safety_comment, field_init_can_be_folded, import_path_can_use_mappings,
    message_entity_naming, method_can_be_static, mutable_parameter_can_be_immutable,
    mutable_variable_can_be_immutable, pure_function_call_unused, reserve_mode_literal,
    send_mode_literal, unused_expression, unused_import, unused_variable, used_ignored_identifier,
    write_only_variable,
};
use acton_config::config::{LintEntry, LintLevel};
use rules::diagnostic::{Diagnostic, Severity};
pub use rules::*;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::{FileId, SymbolId};
use tolk_resolver::resolve_index::FileResolveIndex;
use tolk_resolver::{AstNodeSpanExt, NameUse, Resolved};
use tolk_syntax::{
    AsCast, Call, Expr, ExprStmt, Func, FunctionLike, GetMethod, GlobalVar, HasAnnotations,
    HasGenericParams, HasName, Ident, If, IfAlt, InstanceArg, Method, NotNull, SourceFile, Ternary,
    TopLevel, TypeIdent, Unary, Walker, walk_ast,
};
use tolk_ty::InferenceResult;
use tolk_ty::TypeDb;
use tree_sitter::Node;

#[cfg(feature = "profile_rules")]
mod profiling;
mod rules;

use crate::dfa::{divide_before_multiply, random_requires_initialization, unauthorized_access};
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
    project_root: Option<PathBuf>,

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
            project_root: None,
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

    #[must_use]
    pub fn with_settings(mut self, settings: HashMap<Rule, LintLevel>) -> Self {
        self.settings = settings;
        self
    }

    pub fn with_project_root(mut self, project_root: impl Into<PathBuf>) -> Self {
        self.project_root = Some(project_root.into());
        self
    }

    #[must_use]
    pub fn project_root(&self) -> Option<&Path> {
        self.project_root.as_deref()
    }

    #[must_use]
    pub fn is_contract_root_file(&self, file_id: FileId) -> bool {
        self.file_db
            .get_by_id(file_id)
            .is_some_and(|f| f.is_contract_entry())
    }

    #[must_use]
    pub fn should_run(&self, rule: Rule) -> bool {
        self.settings
            .get(&rule)
            .is_none_or(|level| *level != LintLevel::Allow) // default to run
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

    #[must_use]
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

    #[must_use]
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

    pub fn cfg_for_symbol(
        &mut self,
        symbol_id: SymbolId,
    ) -> Option<Arc<tolk_dataflow::ControlFlowGraph>> {
        self.analysis_db.cfg_for_symbol(self.type_db, symbol_id)
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

impl<'file> Walker<'file> for CheckerWalker<'_, '_> {
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
            Rule::IncomingMessagesDuplicateOpcode,
            incoming_messages_duplicate_opcode::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::MissingContractHeader,
            missing_contract_header::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::UnauthorizedAccess,
            unauthorized_access::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::RandomRequiresInitialization,
            random_requires_initialization::check_file(self.checker, self.file_id)
        );
        run_rule!(
            self.checker,
            Rule::DivideBeforeMultiply,
            divide_before_multiply::check_file(self.checker, self.file_id)
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
        self.default_result();
    }

    fn walk_expr_stmt(&mut self, node: &ExprStmt<'file>) -> Self::Result {
        if let Some(expr) = node.expr() {
            run_rule!(
                self.checker,
                Rule::UnusedExpression,
                unused_expression::check_expr_stmt(self.checker, self.file_id, &expr)
            );
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
        self.resolve_ident_and_run_inspections(&node.0);
    }

    fn walk_type_ident(&mut self, node: &TypeIdent<'file>) -> Self::Result {
        self.resolve_ident_and_run_inspections(&node.0);
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

    fn walk_if(&mut self, node: &If<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::DuplicatedCondition,
            duplicated_condition::check_if(self.checker, self.file_id, node)
        );

        run_rule!(
            self.checker,
            Rule::IdenticalConditionalBranches,
            identical_conditional_branches::check_if(self.checker, self.file_id, node)
        );

        if let Some(condition) = node.condition() {
            self.visit_expr(&condition);
        }
        if let Some(body) = node.body() {
            self.walk_block(&body);
        }
        if let Some(alternative) = node.alternative() {
            match alternative {
                IfAlt::If(if_stmt) => {
                    self.walk_if(&if_stmt);
                }
                IfAlt::Block(block) => {
                    self.walk_block(&block);
                }
            }
        }
        self.default_result();
    }

    fn walk_ternary(&mut self, node: &Ternary<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::IdenticalConditionalBranches,
            identical_conditional_branches::check_ternary(self.checker, self.file_id, node)
        );

        if let Some(condition) = node.condition() {
            self.visit_expr(&condition);
        }
        if let Some(consequence) = node.consequence() {
            self.visit_expr(&consequence);
        }
        if let Some(alternative) = node.alternative() {
            self.visit_expr(&alternative);
        }
        self.default_result();
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
        self.default_result();
    }

    fn walk_as_cast(&mut self, node: &AsCast<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::EnumCastMissingSafetyComment,
            enum_cast_missing_safety_comment::check_as_cast(
                self.checker,
                self.file_id,
                node,
                self.current_inference
            )
        );

        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        if let Some(casted_to) = node.casted_to() {
            self.visit_type(&casted_to);
        }
        self.default_result();
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
        self.default_result();
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
        self.default_result();
    }

    fn walk_global_var(&mut self, node: &GlobalVar<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::NoGlobalVariables,
            no_global_variables::check_global_var(self.checker, self.file_id, node)
        );

        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        self.default_result();
    }

    fn walk_func(&mut self, node: &Func<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::ExplicitReturnType,
            explicit_return_type::check_return_type(
                self.checker,
                self.file_id,
                node,
                self.current_inference
            )
        );

        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(type_params) = node.type_parameters() {
            self.walk_type_parameters(&type_params);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for param in node.parameters() {
            self.walk_parameter(&param, false);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result();
    }

    fn walk_method(&mut self, node: &Method<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::ExplicitReturnType,
            explicit_return_type::check_return_type(
                self.checker,
                self.file_id,
                node,
                self.current_inference
            )
        );

        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(receiver) = node.receiver() {
            self.walk_method_receiver(&receiver);
        }
        if let Some(type_params) = node.type_parameters() {
            self.walk_type_parameters(&type_params);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for param in node.parameters() {
            self.walk_parameter(&param, false);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result();
    }

    fn walk_get_method(&mut self, node: &GetMethod<'file>) -> Self::Result {
        run_rule!(
            self.checker,
            Rule::ExplicitReturnType,
            explicit_return_type::check_return_type(
                self.checker,
                self.file_id,
                node,
                self.current_inference
            )
        );

        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for param in node.parameters() {
            self.walk_parameter(&param, false);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result();
    }

    fn default_result(&self) -> Self::Result {}
}

impl CheckerWalker<'_, '_> {
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
