//! Logic for resolving symbols and tracking scopes.
//!
//! This module implements a visitor that traverses the AST and resolves each
//! identifier to its corresponding definition, taking into account scoping
//! rules and global symbol visibility.

use crate::FileIndex;
use crate::file_db::{FileDb, FileInfo};
use crate::file_index::{AstNodeSpanExt, FileId, SymbolId};
use crate::project_index::ProjectIndex;
use crate::resolve_index::{
    FileResolveIndex, LocalDef, LocalDefId, LocalDefKind, NameUse, NameUseKind, Resolved,
};
use std::collections::HashMap;
use std::sync::Arc;
use tolk_syntax::{
    AstNode, Constant, Enum, EnumMember, FuncBody, FunctionLike, GlobalVar, HasGenericParams,
    HasName, InstanceArg, Struct, StructField, TypeAlias, VarKind, Walker, ast,
};
use tree_sitter::Node;

/// Represents the global environment visible from a specific file.
pub struct GlobalEnv {
    /// Map from symbol name to a list of possible candidates (global symbols).
    pub visible: HashMap<Arc<str>, Vec<SymbolId>>, // name -> candidates
}

impl GlobalEnv {
    /// Creates a new `GlobalEnv` for the given file, including its own symbols,
    /// directly imported symbols, and symbols from `common.tolk`.
    pub fn new(index: &ProjectIndex, file_id: FileId) -> Self {
        let common_tolk = index
            .files()
            .values()
            .find(|f| f.path.ends_with("common.tolk"))
            .cloned();

        let file = index.get_file_index(file_id);

        // Since common.tolk is quite big, preallocate memory for the map to avoid reallocations
        let capacity = common_tolk.as_ref().map(|f| f.decls.len()).unwrap_or(0)
            + file.as_ref().map(|f| f.decls.len()).unwrap_or(0)
            + 50;

        let mut visible: HashMap<Arc<str>, Vec<SymbolId>> = HashMap::with_capacity(capacity);

        // common.tolk is available in any file
        if let Some(common_tolk) = common_tolk {
            Self::add_file_declaration(&mut visible, &common_tolk);
        }

        // add symbols from current file
        if let Some(file) = file {
            Self::add_file_declaration(&mut visible, file);
        }

        // and add symbols from direct imports
        if let Some(imports) = index.imports().get(&file_id) {
            for import in imports {
                let Some(target) = import.target() else {
                    continue;
                };
                let Some(index) = index.files().get(&target) else {
                    continue;
                };

                Self::add_file_declaration(&mut visible, index);
            }
        }

        GlobalEnv { visible }
    }

    fn add_file_declaration(visible: &mut HashMap<Arc<str>, Vec<SymbolId>>, file: &Arc<FileIndex>) {
        for decl in &file.decls {
            visible
                .entry(decl.name.clone())
                .or_insert_with(|| Vec::with_capacity(1)) // avoid reallocation for the most of the cases
                .push(decl.id);
        }
    }
}

/// Represents a lexical scope containing local variable definitions.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Map from variable name to its local definition ID.
    pub symbols: HashMap<Arc<str>, LocalDefId>,
    /// Index of the parent scope in the `SymbolResolver`'s scope list.
    pub parent: Option<usize>,
    /// If this scope fpr lambda.
    pub is_lambda: bool,
}

/// A visitor that tracks lexical scopes and resolves name usages.
pub struct SymbolResolver<'a> {
    scopes: Vec<Scope>,
    current_scope: usize,
    locals: Vec<LocalDef>,
    uses: Vec<NameUse>,
    errors: Vec<SymbolError>,
    project_index: &'a ProjectIndex,
    file: Arc<FileInfo>,
    env: GlobalEnv,
    decl: Option<ast::TopLevel<'a>>,
    inside_method_receiver: bool,
}

/// Represents an error encountered during symbol resolution.
#[derive(Debug, Clone)]
pub struct SymbolError {
    /// Human-readable error message.
    pub message: String,
    /// The tree-sitter node kind where the error occurred.
    pub node_kind: String,
}

impl<'a> SymbolResolver<'a> {
    /// Creates a new `SymbolResolver` for a file.
    pub fn new(
        project_index: &'a ProjectIndex,
        file: Arc<FileInfo>,
        env: GlobalEnv,
    ) -> SymbolResolver<'a> {
        let global_scope = Scope {
            symbols: HashMap::new(),
            parent: None,
            is_lambda: false,
        };
        Self {
            scopes: vec![global_scope],
            current_scope: 0,
            errors: Vec::new(),
            locals: Vec::new(),
            uses: Vec::new(),
            project_index,
            file,
            env,
            decl: None,
            inside_method_receiver: false,
        }
    }

    fn enter_scope(&mut self) {
        self.enter_scope_ext(false);
    }

    fn enter_lambda_scope(&mut self) {
        self.enter_scope_ext(true);
    }

    fn enter_scope_ext(&mut self, is_lambda: bool) {
        let new_scope = Scope {
            symbols: HashMap::new(),
            parent: Some(self.current_scope),
            is_lambda,
        };
        self.scopes.push(new_scope);
        self.current_scope = self.scopes.len() - 1;
    }

    fn exit_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope].parent {
            self.current_scope = parent;
        }
    }

    fn add_symbol(&mut self, node: &Node, name: String, kind: LocalDefKind) {
        let normalized_name = norm(&name);
        let local_def = LocalDef {
            id: LocalDefId {
                file_id: self.file.id(),
                local: node.start_byte() as u32, // this way LocalDefId can be constructed right away from Node
            },
            name: normalized_name.clone(),
            def_span: node.span(),
            kind,
        };

        self.locals.push(local_def.clone());
        self.scopes[self.current_scope]
            .symbols
            .insert(normalized_name, local_def.id);
    }

    fn resolve_symbol(&mut self, ident: &Node, use_kind: NameUseKind) -> Option<()> {
        let name = norm(self.file.text(ident).ok()?);

        let mut current = Some(self.current_scope);
        let decl_start = self.decl_start();

        // Local resolve upward
        while let Some(scope_idx) = current {
            let scope = &self.scopes[scope_idx];
            if let Some(symbol_id) = scope.symbols.get(&name) {
                self.uses.push(NameUse {
                    decl: decl_start,
                    span: ident.span(),
                    kind: use_kind,
                    name,
                    resolved: Resolved::Local(*symbol_id),
                });
                return Some(());
            }
            if scope.is_lambda {
                // don't look up in scopes out of lambda
                break;
            }
            current = scope.parent;
        }

        if use_kind == NameUseKind::LocalValue {
            // don't resolve local values in global symbols
            return None;
        }

        let empty_candidates = vec![];
        let candidates = self.env.visible.get(&name).unwrap_or(&empty_candidates);

        if candidates.is_empty() {
            if self.inside_method_receiver {
                // fun Foo<T>.foo() {}
                //         ^ unresolved

                let normalized_name = norm(&name);
                let local_def = LocalDef {
                    id: LocalDefId {
                        file_id: self.file.id(),
                        local: ident.start_byte() as u32, // this way LocalDefId can be constructed right away from Node
                    },
                    name: normalized_name.clone(),
                    def_span: ident.span(),
                    kind: LocalDefKind::TypeParameter,
                };

                self.locals.push(local_def.clone());
                self.scopes[self.current_scope]
                    .symbols
                    .insert(normalized_name, local_def.id);
                return None;
            }

            self.uses.push(NameUse {
                decl: decl_start,
                span: ident.span(),
                kind: use_kind,
                name: name.clone(),
                resolved: Resolved::Unresolved,
            });
            self.errors.push(SymbolError {
                message: format!("Undefined symbol: {}", name),
                node_kind: "identifier".to_string(),
            });
            return None;
        }

        if candidates.len() == 1
            && let Some(single) = candidates.first()
        {
            // fast path, single candidate
            self.uses.push(NameUse {
                decl: decl_start,
                span: ident.span(),
                kind: use_kind,
                name,
                resolved: Resolved::Global(*single),
            });
            return Some(());
        }

        // if candidates > 1 we need to handle clashes like `address` type and `address` function

        let mut filtered_candidate = candidates.iter().filter_map(|c| {
            // for example, we have `address` function and type
            let symbol = self.project_index.resolve_symbol(*c)?; // TODO: cache NamespaceKind?

            if use_kind == NameUseKind::Mixed {
                // no need to filter anything, we need all variants
                return Some(symbol);
            }

            // if we're looking for a type, filter `address` function
            if use_kind == NameUseKind::Type && !symbol.is_type() {
                return None;
            }
            // otherwise, filter `address` type
            if use_kind == NameUseKind::Value && symbol.is_type() {
                return None;
            }
            Some(symbol)
        });

        let Some(final_candidate) = filtered_candidate.next() else {
            // // should not be reachable
            // error!("no candidates after filtering for {name}");
            return None;
        };

        self.uses.push(NameUse {
            decl: decl_start,
            span: ident.span(),
            kind: use_kind,
            name,
            resolved: Resolved::Global(final_candidate.id),
        });
        Some(())
    }

    fn decl_start(&self) -> u32 {
        self.decl
            .map(|d| d.syntax().start_byte() as u32)
            .unwrap_or(0)
    }

    fn check_redeclaration(&mut self, name: &str, node_kind: &str) {
        let name = norm(name);
        let name = name.as_ref();
        if self.scopes[self.current_scope].symbols.contains_key(name) {
            self.errors.push(SymbolError {
                message: format!("Symbol '{}' redeclared in same scope", name),
                node_kind: node_kind.to_string(),
            });
        }
    }

    fn add_variables_from_pattern<'t>(&mut self, pat: &ast::VarDeclPattern<'t>, kind: VarKind) {
        match pat {
            ast::VarDeclPattern::VarDecl(var_decl) => {
                if let Some(name) = var_decl.name() {
                    let name_str = name.text(self.file_content()).to_string();
                    self.check_redeclaration(&name_str, "var_declaration");
                    let is_mutable = matches!(kind, VarKind::Var);
                    self.add_symbol(
                        &name.0,
                        name_str,
                        LocalDefKind::Var {
                            is_mutable,
                            has_type: var_decl.typ().is_some(),
                        },
                    );
                }

                if let Some(typ) = var_decl.typ() {
                    self.visit_type(&typ)
                }
            }
            ast::VarDeclPattern::TupleVars(tuple) => {
                for var_pattern in tuple.vars() {
                    self.add_variables_from_pattern(&var_pattern, kind);
                }
            }
            ast::VarDeclPattern::TensorVars(tensor) => {
                for var_pattern in tensor.vars() {
                    self.add_variables_from_pattern(&var_pattern, kind);
                }
            }
        }
    }

    fn file_content(&self) -> &str {
        self.file.source().source.as_ref()
    }
}

impl<'tree> Walker<'tree> for SymbolResolver<'_> {
    type Result = ();

    fn walk_global_var(&mut self, node: &GlobalVar<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        self.default_result()
    }

    fn walk_constant(&mut self, node: &Constant<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        if let Some(value) = node.value() {
            self.visit_expr(&value);
        }
        self.default_result()
    }

    fn walk_type_alias(&mut self, node: &TypeAlias<'tree>) -> Self::Result {
        self.enter_scope(); // for type parameters
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(type_params) = node.type_parameters() {
            self.walk_type_parameters(&type_params);
        }
        if let Some(underlying_type) = node.underlying_type() {
            self.walk_type_alias_underlying_type(&underlying_type);
        }
        self.exit_scope();
        self.default_result()
    }

    fn walk_struct(&mut self, node: &Struct<'tree>) -> Self::Result {
        self.enter_scope(); // for type parameters
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(type_params) = node.type_parameters() {
            self.walk_type_parameters(&type_params);
        }
        if let Some(body) = node.body() {
            self.walk_struct_body(&body);
        }
        self.exit_scope();
        self.default_result()
    }

    fn walk_struct_field(&mut self, node: &StructField<'tree>) -> Self::Result {
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        if let Some(default) = node.default() {
            self.visit_expr(&default);
        }
        self.default_result()
    }

    fn walk_enum(&mut self, node: &Enum<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(backed_type) = node.backed_type() {
            self.visit_type(&backed_type);
        }
        if let Some(body) = node.body() {
            self.walk_enum_body(&body);
        }
        self.default_result()
    }

    fn walk_enum_member(&mut self, node: &EnumMember<'tree>) -> Self::Result {
        if let Some(default) = node.default() {
            self.visit_expr(&default);
        }
        self.default_result()
    }

    fn walk_func(&mut self, node: &ast::Func<'tree>) -> Self::Result {
        self.enter_scope();

        let body = node.body();
        let is_common = matches!(body, Some(FuncBody::Block(_)));

        if let Some(params) = node.type_parameters() {
            self.walk_type_parameters(&params);
        }
        for param in node.parameters() {
            self.walk_parameter(&param, is_common);
        }
        if let Some(return_type) = node.return_type() {
            self.walk_type(&return_type);
        }
        if let Some(body) = body {
            self.walk_function_body(&body);
        }

        self.exit_scope();
    }

    fn walk_method(&mut self, node: &ast::Method<'tree>) -> Self::Result {
        self.enter_scope();

        let body = node.body();
        let is_common = matches!(body, Some(FuncBody::Block(_)));

        if let Some(receiver) = node.receiver() {
            self.inside_method_receiver = true;
            self.walk_method_receiver(&receiver);
            self.inside_method_receiver = false;
        }
        if let Some(params) = node.type_parameters() {
            self.walk_type_parameters(&params);
        }
        for param in node.parameters() {
            self.walk_parameter(&param, is_common);
        }
        if let Some(return_type) = node.return_type() {
            self.walk_type(&return_type);
        }
        if let Some(body) = body {
            self.walk_function_body(&body);
        }

        self.exit_scope();
    }

    fn walk_get_method(&mut self, node: &ast::GetMethod<'tree>) -> Self::Result {
        self.enter_scope();

        let body = node.body();
        let is_common = matches!(body, Some(FuncBody::Block(_)));

        if let Some(params) = node.type_parameters() {
            self.walk_type_parameters(&params);
        }
        for param in node.parameters() {
            self.walk_parameter(&param, is_common);
        }
        if let Some(return_type) = node.return_type() {
            self.walk_type(&return_type);
        }
        if let Some(body) = body {
            self.walk_function_body(&body);
        }

        self.exit_scope();
    }

    fn walk_block(&mut self, node: &ast::Block<'tree>) -> Self::Result {
        self.enter_scope();
        for stmt in node.stmts() {
            self.visit_stmt(&stmt);
        }
        self.exit_scope();
    }

    fn walk_do_while(&mut self, node: &ast::DoWhile<'tree>) -> Self::Result {
        self.enter_scope();
        if let Some(body) = node.body() {
            for stmt in body.stmts() {
                self.visit_stmt(&stmt);
            }
        }
        if let Some(condition) = node.condition() {
            self.visit_expr(&condition);
        }
        self.exit_scope();
    }

    fn walk_dot_access(&mut self, node: &ast::DotAccess<'tree>) -> Self::Result {
        if let Some(obj) = node.obj() {
            self.visit_expr(&obj);
        }
        // don't walk field, we don't have types yet
    }

    fn walk_match(&mut self, node: &ast::Match<'tree>) -> Self::Result {
        self.enter_scope();
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr)
        }
        if let Some(body) = node.body() {
            self.walk_match_body(&body);
        }
        self.exit_scope();
        self.default_result()
    }

    fn walk_lambda(&mut self, node: &ast::Lambda<'tree>) -> Self::Result {
        self.enter_lambda_scope();

        for param in node.parameters() {
            self.walk_lambda_parameter(&param);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_block(&body)
        }

        self.exit_scope();
    }

    fn walk_lambda_parameter(&mut self, node: &ast::LambdaParameter<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            let name_str = name.text(self.file_content()).to_string();
            self.check_redeclaration(&name_str, "parameter_declaration");
            self.add_symbol(
                &name.0,
                name_str,
                LocalDefKind::Param {
                    has_type: node.typ().is_some(),
                    is_mutable: node.mutate(),
                    is_self: false,           // there is no self parameters in lambdas
                    in_asm_or_builtin: false, // lambda cannot be assembly or builtin
                },
            );
        }

        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }

        if let Some(default) = node.default() {
            self.visit_expr(&default);
        }
    }

    fn walk_ident(&mut self, node: &ast::Ident<'tree>) -> Self::Result {
        self.resolve_symbol(&node.0, NameUseKind::Value);
    }

    fn walk_type_ident(&mut self, node: &ast::TypeIdent<'tree>) -> Self::Result {
        self.resolve_symbol(&node.0, NameUseKind::Type);
    }

    fn walk_type_parameter(&mut self, node: &ast::TypeParameter<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            let name_str = name.text(self.file_content()).to_string();
            self.add_symbol(&name.0, name_str, LocalDefKind::TypeParameter);
        }
    }

    fn walk_parameter(&mut self, node: &ast::Parameter<'tree>, in_common: bool) -> Self::Result {
        if let Some(name) = node.name() {
            let name_str = name.text(self.file_content()).to_string();
            let is_self = name_str == "self";
            self.check_redeclaration(&name_str, "parameter_declaration");
            self.add_symbol(
                &name.0,
                name_str,
                LocalDefKind::Param {
                    has_type: node.typ().is_some(),
                    is_mutable: node.mutate(),
                    is_self,
                    in_asm_or_builtin: !in_common,
                },
            );
        }

        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }

        if let Some(default) = node.default() {
            self.visit_expr(&default);
        }
    }

    fn walk_catch_clause(&mut self, node: &ast::CatchClause<'tree>) -> Self::Result {
        self.enter_scope();

        if let Some(var1) = node.catch_var1() {
            let name_str = var1.text(self.file_content()).to_string();
            self.add_symbol(&var1.0, name_str, LocalDefKind::Catch);
        }

        if let Some(var2) = node.catch_var2() {
            let name_str = var2.text(self.file_content()).to_string();
            self.add_symbol(&var2.0, name_str, LocalDefKind::Catch);
        }

        if let Some(body) = node.body() {
            self.walk_block(&body);
        }

        self.exit_scope();
    }

    fn walk_var_decl_lhs(&mut self, node: &ast::VarDeclLhs<'tree>) -> Self::Result {
        if let Some(pattern) = node.pattern() {
            self.add_variables_from_pattern(&pattern, node.kind());
        }
    }

    fn walk_match_arm(&mut self, node: &ast::MatchArm<'tree>) -> Self::Result {
        match node.pattern() {
            ast::MatchPattern::Type(typ) => {
                self.visit_type(&typ);
            }
            ast::MatchPattern::Expr(expr) => {
                if let ast::Expr::Ident(ident) = expr {
                    // we don't know if it is type or identifier
                    self.resolve_symbol(&ident.0, NameUseKind::Mixed);
                } else {
                    self.walk_expr(&expr);
                }
            }
            ast::MatchPattern::Else => {
                // nothing to do
            }
        }

        if let Some(body) = node.body() {
            match body {
                ast::MatchArmBody::Block(ref block) => self.walk_block(block),
                ast::MatchArmBody::Return(ref ret) => self.walk_return(ret),
                ast::MatchArmBody::Throw(ref throw) => self.walk_throw(throw),
                ast::MatchArmBody::Expr(ref expr) => self.visit_expr(expr),
            };
        }
    }

    fn walk_instance_arg(&mut self, node: &InstanceArg<'tree>) -> Self::Result {
        // in `Foo { foo }` we need to resolve `foo` as local variable
        // if there is some value like `{ foo: bar }` we don't need to process field name
        if let Some(value) = node.value() {
            self.visit_expr(&value);
            return;
        }

        let Some(name) = node.name() else { return };
        // so we have `{ foo }` now
        self.resolve_symbol(&name.syntax(), NameUseKind::LocalValue);
    }

    fn default_result(&self) -> Self::Result {}
}

fn norm(name: &str) -> Arc<str> {
    Arc::from(name.trim_matches('`'))
}

/// Resolves all symbols in all files present in the `ProjectIndex`.
pub fn resolve(db: &FileDb, index: &mut ProjectIndex) {
    let files = index.files().keys().cloned().collect::<Vec<_>>();
    for file_id in files {
        let Some(file_index) = resolve_file(db, index, file_id) else {
            continue;
        };

        index.resolved_uses.insert(file_id, Arc::new(file_index));
    }
}

/// Resolves all symbols within a single file and updates the `ProjectIndex`.
pub fn resolve_file(db: &FileDb, index: &ProjectIndex, file: FileId) -> Option<FileResolveIndex> {
    let file_info = &db.get_by_id(file)?;
    let env = GlobalEnv::new(index, file);

    let mut resolver = SymbolResolver::new(index, file_info.clone(), env);

    for decl in file_info.source().top_levels() {
        resolver.decl = Some(decl);
        match decl {
            ast::TopLevel::TolkRequiredVersion(_) => {}
            ast::TopLevel::Import(_) => {}
            ast::TopLevel::GlobalVar(decl) => resolver.walk_global_var(&decl),
            ast::TopLevel::Constant(decl) => resolver.walk_constant(&decl),
            ast::TopLevel::TypeAlias(decl) => resolver.walk_type_alias(&decl),
            ast::TopLevel::Struct(decl) => resolver.walk_struct(&decl),
            ast::TopLevel::Enum(decl) => resolver.walk_enum(&decl),
            ast::TopLevel::Func(func) => resolver.walk_func(&func),
            ast::TopLevel::Method(method) => resolver.walk_method(&method),
            ast::TopLevel::GetMethod(method) => resolver.walk_get_method(&method),
            ast::TopLevel::EmptyStmt(_) => {}
            ast::TopLevel::Unmapped(_) => {}
        }
    }

    resolver.uses.sort_by_key(|u| u.span.start);

    Some(FileResolveIndex {
        file_id: resolver.file.id(),
        locals: resolver.locals,
        uses: resolver.uses,
    })
}
