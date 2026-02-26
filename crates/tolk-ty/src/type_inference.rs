use crate::flow_inference::{FlowContext, InferenceContext, InferenceResult, SinkExpr};
use crate::type_db::TypeDb;
use crate::type_interner::{TyId, TypeInterner};
use crate::type_substitutor::TypeSubstitutor;
use crate::type_unify::TypeInferringUnifyStrategy;
use crate::types::TyData;
use log::warn;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use std::collections::VecDeque;
use tolk_resolver::file_index::{
    AstNodeSpanExt, FileId, OptionalSyntaxNodeSpanExt, Span, SymbolId,
};
use tolk_resolver::resolve_index::LocalDefId;
use tolk_syntax::{
    AstNode, Constant, Enum, FuncBody, FunctionLike, GlobalVar, HasGenericParams, HasName, Method,
    Parameter, Struct, TopLevel, Type, TypeAlias,
};

/// Runs type inference on a top-level declaration.
///
/// This function analyzes the given declaration (function, const, global var, etc.) and infers types for all expressions within it.
/// It also performs control flow analysis to handle smart casts.
///
/// # Arguments
///
/// * `type_db` — The type database containing global type definitions.
/// * `file_id` — The ID of the file containing the declaration.
/// * `symbol_id` — The symbol ID of the declaration being inferred.
/// * `decl` — The AST node of the top-level declaration.
///
/// # Returns
///
/// An `InferenceResult` containing inferred types and resolved references.
pub fn infer(
    type_db: &mut TypeDb,
    file_id: FileId,
    symbol_id: SymbolId,
    decl: &TopLevel,
) -> InferenceResult {
    let ctx = InferenceContext::new(file_id, type_db, VecDeque::new());
    let mut walker = TypeInferenceWalker::new(ctx);
    walker.ctx.decl_start = decl.syntax().start_byte() as u32;

    match decl {
        TopLevel::GlobalVar(v) => {
            walker.infer_global_var(v, symbol_id);
        }
        TopLevel::Constant(v) => {
            walker.infer_constant(v, symbol_id);
        }
        TopLevel::TypeAlias(v) => {
            walker.infer_type_alias(v, symbol_id);
        }
        TopLevel::Struct(v) => {
            walker.infer_struct(v, symbol_id);
        }
        TopLevel::Enum(v) => {
            walker.infer_enum(v, symbol_id);
        }
        TopLevel::Func(fun) => {
            walker.ctx.caller_function = Some(symbol_id);
            walker.infer_function_base(fun, symbol_id);
        }
        TopLevel::Method(method) => {
            walker.ctx.caller_function = Some(symbol_id);
            walker.infer_method(method, symbol_id);
        }
        TopLevel::GetMethod(method) => {
            walker.ctx.caller_function = Some(symbol_id);
            walker.infer_function_base(method, symbol_id);
        }
        TopLevel::Import(_)
        | TopLevel::TolkRequiredVersion(_)
        | TopLevel::Contract(_)
        | TopLevel::EmptyStmt(_)
        | TopLevel::Unmapped(_) => {}
    }

    InferenceResult::new(walker.ctx)
}

pub(crate) struct TypeInferenceWalker<'db, 'a> {
    pub ctx: InferenceContext<'db, 'a>,
}

impl<'db, 'a> TypeInferenceWalker<'db, 'a> {
    pub(crate) const fn new(ctx: InferenceContext<'db, 'a>) -> Self {
        Self { ctx }
    }

    pub(crate) const fn intrn(&mut self) -> &mut TypeInterner {
        self.ctx.type_db.intrn
    }

    pub(crate) const fn const_intrn(&self) -> &TypeInterner {
        self.ctx.type_db.intrn
    }

    pub(crate) fn lower(&mut self, ty: Option<Type<'_>>) -> TyId {
        self.lower_or_none(ty)
            .unwrap_or(self.ctx.type_db.intrn.ty_undefined)
    }

    pub(crate) fn lower_or_none(&mut self, ty: Option<Type<'_>>) -> Option<TyId> {
        self.ctx
            .type_db
            .lower_opt_type(self.ctx.file_id, ty.as_ref())
    }

    pub(crate) fn apply_defaults_to_type(&mut self, ty: TyId) -> TyId {
        if !self.intrn().has_generics(ty) {
            return ty;
        }
        let mapping = FxHashMap::default();
        let mut substitutor = TypeSubstitutor::new_with_defaults(self.intrn());
        substitutor.substitute(ty, &mapping)
    }

    pub(crate) const fn local_id_of(&self, span: Span) -> LocalDefId {
        LocalDefId {
            local: span.start,
            file_id: self.ctx.file_id,
        }
    }

    pub(crate) fn sink_of(&self, param: &Parameter) -> SinkExpr {
        let span = param.name().map(|p| p.0).span();
        let name = self.text_of(param);
        let id = self.local_id_of(span);

        SinkExpr::from_def(name, id, 0)
    }

    pub(crate) fn text_or_none<'node, Node: AstNode<'node>>(&self, node: &Node) -> Option<SmolStr> {
        self.ctx.text(node.span())
    }

    pub(crate) fn text_of<'node, Node: AstNode<'node>>(&self, node: &Node) -> SmolStr {
        self.ctx.text(node.span()).unwrap_or_else(|| {
            // very unlikely
            warn!(
                "cannot get text of node at {} inside file {}",
                node.span(),
                self.ctx.file_id
            );
            SmolStr::new_inline("unknown")
        })
    }

    pub(crate) fn infer_global_var<'t>(&mut self, v: &GlobalVar<'t>, symbol_id: SymbolId) {
        let declared_type = self.lower(v.typ());
        self.ctx.set_top_level_type(symbol_id, declared_type);
        self.ctx.set_node_type(v, declared_type);
        if let Some(name) = v.name() {
            self.ctx.set_node_type(&name, declared_type);
        }
    }

    pub(crate) fn infer_type_alias<'t>(&mut self, v: &TypeAlias<'t>, symbol_id: SymbolId) {
        // type alias was already inferred during top levels type inference
        let Some(name_ident) = v.name() else { return };
        let ty = self
            .ctx
            .get_top_level_type(symbol_id)
            .unwrap_or_else(|| self.intrn().ty_undefined);
        self.ctx.set_node_type(&name_ident, ty);
        self.ctx.set_node_type(v, ty);

        self.infer_type_parameters(v);

        if let Some(name) = v.name() {
            self.ctx.set_node_type(&name, ty);
        }
    }

    pub(crate) fn infer_constant<'t>(&mut self, v: &Constant<'t>, symbol_id: SymbolId) {
        let declared_type = self.lower_or_none(v.typ());
        let flow = FlowContext::new();
        if let Some(value) = v.value() {
            self.infer_expr(value, flow, false, declared_type);
            let inferred_type = self.ctx.get_node_type_or_unknown(&value);
            let final_type = declared_type.unwrap_or(inferred_type);
            self.ctx.set_top_level_type(symbol_id, final_type);
            self.ctx.set_node_type(v, final_type);
            if let Some(name) = v.name() {
                self.ctx.set_node_type(&name, final_type);
            }
        } else if let Some(declared_type) = declared_type {
            self.ctx.set_top_level_type(symbol_id, declared_type);
            self.ctx.set_node_type(v, declared_type);
            if let Some(name) = v.name() {
                self.ctx.set_node_type(&name, declared_type);
            }
        }
    }

    pub(crate) fn infer_struct<'t>(&mut self, v: &Struct<'t>, symbol_id: SymbolId) {
        let Some(name_ident) = v.name() else { return };
        let Some(body) = v.body() else { return };

        self.infer_type_parameters(v);

        for field in body.fields() {
            let declared_type = self.lower_or_none(field.typ());
            if let Some(default_value) = field.default() {
                let flow = FlowContext::new();
                self.infer_expr(default_value, flow, false, declared_type);
            }

            if let Some(declared_type) = declared_type {
                self.ctx.set_node_type(&field, declared_type);
                if let Some(name) = field.name() {
                    self.ctx.set_node_type(&name, declared_type);
                }
            }
        }

        let Some(name) = self.text_or_none(&name_ident) else {
            return;
        };
        let struct_ty = self.intrn().struct_ty(symbol_id, name.into());
        self.ctx.set_node_type(&name_ident, struct_ty);
        self.ctx.set_node_type(v, struct_ty);
        if let Some(name) = v.name() {
            self.ctx.set_node_type(&name, struct_ty);
        }
    }

    pub(crate) fn infer_enum<'t>(&mut self, v: &Enum<'t>, symbol_id: SymbolId) {
        let Some(name_ident) = v.name() else { return };
        let Some(body) = v.body() else { return };
        for member in body.members() {
            if let Some(default_value) = member.default() {
                let flow = FlowContext::new();
                self.infer_expr(default_value, flow, false, None);
            }
        }

        let Some(name) = self.text_or_none(&name_ident) else {
            return;
        };
        let enum_ty = self.intrn().enum_ty(symbol_id, name.into());
        self.ctx.set_node_type(&name_ident, enum_ty);
        self.ctx.set_node_type(v, enum_ty);
        if let Some(name) = v.name() {
            self.ctx.set_node_type(&name, enum_ty);
        }
    }

    fn infer_type_parameters<'b, Node: HasGenericParams<'b>>(&mut self, v: &Node) {
        let Some(type_parameters) = v.type_parameters() else {
            return;
        };

        for param in type_parameters.parameters() {
            let Some(param_name_node) = param.name() else {
                continue;
            };
            let param_name = self
                .ctx
                .type_db
                .file_db
                .text_of(self.ctx.file_id, &param_name_node)
                .unwrap_or_default()
                .to_string();

            let default_type = param.default();
            let default_ty = self.lower_or_none(default_type);
            let param_ty = self.intrn().type_parameter(param_name, default_ty);

            self.ctx.set_node_type(&param, param_ty);
            self.ctx.set_node_type(&param_name_node, param_ty);

            if let Some(default) = default_type {
                self.ctx.set_node_type(&default, param_ty);
            }
        }
    }

    fn update_function_return_type(&mut self, symbol_id: SymbolId) {
        if let Some(inferred_ty) = self.ctx.inferred_return_type
            && let Some(existing_ty) = self.ctx.get_top_level_type(symbol_id)
            && let TyData::Func { params, .. } = self.ctx.type_db.intrn.data(existing_ty)
        {
            let new_fun_ty = self.ctx.type_db.intrn.func(params.clone(), inferred_ty);
            self.ctx.set_top_level_type(symbol_id, new_fun_ty);
        }
    }

    /// Infers both standalone functions and get methods.
    pub(crate) fn infer_function_base<'t, 'b, F: FunctionLike<'b> + HasGenericParams<'b>>(
        &mut self,
        v: &F,
        symbol_id: SymbolId,
    ) -> Option<()> {
        let mut body_start = FlowContext::new();
        let is_generic_declaration = v.type_parameters().is_some();
        let declared_return_ty = self.lower_or_none(v.return_type()).map(|ty| {
            if is_generic_declaration {
                ty
            } else {
                self.apply_defaults_to_type(ty)
            }
        });

        self.infer_type_parameters(v);

        if let Some(return_type) = v.return_type()
            && let Some(declared_return_ty) = declared_return_ty
        {
            self.ctx.set_node_type(&return_type, declared_return_ty)
        }
        self.ctx.declared_return_ty = declared_return_ty;

        for param in v.parameters() {
            let Some(name) = param.name() else {
                // something very strange happened if we have a parameter without name
                continue;
            };

            let param_type = {
                let ty = self.lower(param.typ());
                if is_generic_declaration {
                    ty
                } else {
                    self.apply_defaults_to_type(ty)
                }
            };
            body_start.register_known_type(self.sink_of(&param), param_type);
            self.ctx.set_node_type(&name, param_type);

            if let Some(default) = param.default() {
                let after_default = self.infer_expr(default, body_start, false, Some(param_type));
                body_start = after_default.out_flow;
            }
        }

        let Some(FuncBody::Block(body)) = v.body() else {
            return None;
        };

        let body_end = self.process_block_stmt(body, body_start);

        self.infer_return_type_if_needed(declared_return_ty, &body_end);
        self.update_function_return_type(symbol_id);

        Some(())
    }

    pub(crate) fn infer_method<'t>(&mut self, v: &Method<'t>, symbol_id: SymbolId) -> Option<()> {
        let mut body_start = FlowContext::new();
        let is_generic_declaration = v.type_parameters().is_some();
        let declared_return_ty = self.lower_or_none(v.return_type()).map(|ty| {
            if is_generic_declaration {
                ty
            } else {
                self.apply_defaults_to_type(ty)
            }
        });
        self.ctx.declared_return_ty = declared_return_ty;

        self.infer_type_parameters(v);

        let receiver_type = {
            let ty = self.lower(v.receiver_type());
            if is_generic_declaration {
                ty
            } else {
                self.apply_defaults_to_type(ty)
            }
        };

        for param in v.parameters() {
            let Some(name) = param.name() else {
                // something very strange happened if we have a parameter without name
                continue;
            };

            let is_self = self.ctx.text_matches(&name, "self");
            if is_self {
                body_start.register_known_type(self.sink_of(&param), receiver_type);
                self.ctx.set_node_type(&name, receiver_type);
                continue;
            }

            let param_type = {
                let ty = self.lower(param.typ());
                if is_generic_declaration {
                    ty
                } else {
                    self.apply_defaults_to_type(ty)
                }
            };
            body_start.register_known_type(self.sink_of(&param), param_type);
            self.ctx.set_node_type(&name, param_type);

            if let Some(default) = param.default() {
                let after_default = self.infer_expr(default, body_start, false, Some(param_type));
                body_start = after_default.out_flow;
            }
        }

        let Some(FuncBody::Block(body)) = v.body() else {
            return None;
        };

        let body_end = self.process_block_stmt(body, body_start);

        self.infer_return_type_if_needed(declared_return_ty, &body_end);
        self.update_function_return_type(symbol_id);

        Some(())
    }

    pub(crate) fn infer_return_type_if_needed(
        &mut self,
        declared_return_type: Option<TyId>,
        body_end: &FlowContext,
    ) {
        if declared_return_type.is_some() {
            return;
        }

        let return_types = self.ctx.return_types.clone();
        let mut return_unifier = TypeInferringUnifyStrategy::new();

        let interner = self.intrn();
        let ty_void = interner.ty_void;
        let ty_never = interner.ty_never;

        for &return_ty in &return_types {
            return_unifier.unify_with(return_ty, None, interner);
        }

        let inferred_return_type = if return_types.is_empty() {
            // No return statements at all
            if body_end.is_unreachable() {
                ty_never
            } else {
                ty_void
            }
        } else {
            return_unifier.get_result(interner)
        };

        self.ctx.inferred_return_type = Some(inferred_return_type);
    }
}

#[macro_export]
macro_rules! try_flow {
    ($flow:expr, $expr:expr) => {
        match $expr {
            Some(value) => value,
            None => {
                return $flow;
            }
        }
    };
}

#[macro_export]
macro_rules! try_expr_flow {
    ($flow:expr, $expr:expr) => {
        match $expr {
            Some(value) => value,
            None => {
                return ExprFlow::create($flow, false);
            }
        }
    };
}
