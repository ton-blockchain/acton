use crate::flow_inference::{FlowContext, InferenceContext, InferenceResult, SinkExpr};
use crate::type_db::TypeDb;
use crate::type_interner::{TyId, TypeInterner};
use crate::type_unify::TypeInferringUnifyStrategy;
use crate::types::TyData;
use log::warn;
use smol_str::SmolStr;
use std::collections::VecDeque;
use tolk_resolver::file_index::{
    AstNodeSpanExt, FileId, OptionalSyntaxNodeSpanExt, Span, SymbolId,
};
use tolk_resolver::resolve_index::LocalDefId;
use tolk_syntax::{
    AstNode, Constant, Enum, FuncBody, FunctionLike, GlobalVar, HasName, Method, Parameter, Struct,
    TopLevel, Type, TypeAlias,
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
        | TopLevel::EmptyStmt(_)
        | TopLevel::Unmapped(_) => {}
    }

    InferenceResult::new(walker.ctx)
}

pub struct TypeInferenceWalker<'db, 'a> {
    pub ctx: InferenceContext<'db, 'a>,
}

impl<'db, 'a> TypeInferenceWalker<'db, 'a> {
    pub fn new(ctx: InferenceContext<'db, 'a>) -> Self {
        Self { ctx }
    }

    pub(crate) fn intrn(&mut self) -> &mut TypeInterner {
        self.ctx.type_db.intrn
    }

    pub(crate) fn const_intrn(&self) -> &TypeInterner {
        self.ctx.type_db.intrn
    }

    pub(crate) fn lower(&mut self, ty: Option<Type<'_>>) -> TyId {
        self.lower_or_none(ty)
            .unwrap_or(self.ctx.type_db.intrn.ty_unknown)
    }

    pub(crate) fn lower_or_none(&mut self, ty: Option<Type<'_>>) -> Option<TyId> {
        self.ctx
            .type_db
            .lower_opt_type(self.ctx.file_id, ty.as_ref())
    }

    pub(crate) fn local_id_of(&mut self, span: Span) -> LocalDefId {
        LocalDefId {
            local: span.start,
            file_id: self.ctx.file_id,
        }
    }

    pub(crate) fn sink_of(&mut self, param: &Parameter) -> SinkExpr {
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

    pub fn infer_global_var<'t>(&mut self, v: &GlobalVar<'t>, symbol_id: SymbolId) {
        let declared_type = self.lower(v.typ());
        self.ctx.set_top_level_type(symbol_id, declared_type);
        self.ctx.set_node_type(v, declared_type);
    }

    pub fn infer_type_alias<'t>(&mut self, v: &TypeAlias<'t>, symbol_id: SymbolId) {
        // type alias was already inferred during top levels type inference
        let Some(name_ident) = v.name() else { return };
        let ty = self
            .ctx
            .get_top_level_type(symbol_id)
            .unwrap_or(self.intrn().ty_unknown);
        self.ctx.set_node_type(&name_ident, ty);
        self.ctx.set_node_type(v, ty);
    }

    pub fn infer_constant<'t>(&mut self, v: &Constant<'t>, symbol_id: SymbolId) {
        let declared_type = self.lower_or_none(v.typ());
        let flow = FlowContext::new();
        if let Some(value) = v.value() {
            self.infer_expr(value, flow, false, declared_type);
            let inferred_type = self.ctx.get_node_type_or_unknown(&value);
            let final_type = declared_type.unwrap_or(inferred_type);
            self.ctx.set_top_level_type(symbol_id, final_type);
            self.ctx.set_node_type(v, final_type);
        } else if let Some(declared_type) = declared_type {
            self.ctx.set_top_level_type(symbol_id, declared_type);
            self.ctx.set_node_type(v, declared_type);
        }
    }

    pub fn infer_struct<'t>(&mut self, v: &Struct<'t>, symbol_id: SymbolId) {
        let Some(name_ident) = v.name() else { return };
        let Some(body) = v.body() else { return };
        for field in body.fields() {
            if let Some(default_value) = field.default() {
                let declared_type = self.lower_or_none(field.typ());
                let flow = FlowContext::new();
                self.infer_expr(default_value, flow, false, declared_type);

                if let Some(declared_type) = declared_type {
                    self.ctx.set_node_type(&field, declared_type);
                }
            }
        }

        let Some(name) = self.text_or_none(&name_ident) else {
            return;
        };
        let struct_ty = self.intrn().struct_ty(symbol_id, name.into());
        self.ctx.set_node_type(&name_ident, struct_ty);
        self.ctx.set_node_type(v, struct_ty);
    }

    pub fn infer_enum<'t>(&mut self, v: &Enum<'t>, symbol_id: SymbolId) {
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
    pub fn infer_function_base<'t, 'b, F: FunctionLike<'b>>(
        &mut self,
        v: &F,
        symbol_id: SymbolId,
    ) -> Option<()> {
        let mut body_start = FlowContext::new();
        let declared_return_ty = self.lower_or_none(v.return_type());
        self.ctx.declared_return_ty = declared_return_ty;

        for param in v.parameters() {
            let Some(name) = param.name() else {
                // something very strange happened if we have a parameter without name
                continue;
            };

            let param_type = self.lower(param.typ());
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

    pub fn infer_method<'t>(&mut self, v: &Method<'t>, symbol_id: SymbolId) -> Option<()> {
        let mut body_start = FlowContext::new();
        let declared_return_ty = self.lower_or_none(v.return_type());
        self.ctx.declared_return_ty = declared_return_ty;

        let receiver_type = self.lower(v.receiver_type());

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

            let param_type = self.lower(param.typ());
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
