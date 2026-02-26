use crate::flow_inference::{
    ExprFlow, FlowContext, InferenceContext, InferenceResult, MethodKey, SinkExpr, UnreachableKind,
};
use crate::generics_helpers::GenericSubstitutionsDeducing;
use crate::generics_helpers::GenericsSubstitutions;
use crate::overload_resolution::{MethodCallCandidate, choose_only_method_to_call};
use crate::type_inference::TypeInferenceWalker;
use crate::type_interner::TyId;
use crate::type_substitutor::TypeSubstitutor;
use crate::type_unify::TypeInferringUnifyStrategy;
use crate::types::TyData;
use crate::{try_expr_flow, try_flow};
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use tolk_resolver::file_index::OptionalSyntaxNodeSpanExt;
use tolk_resolver::file_index::SymbolId;
use tolk_resolver::resolve_index::{LocalDefId, LocalDefKind, NameUse, NameUseKind, Resolved};
use tolk_resolver::{AstNodeSpanExt, Span, Symbol, SymbolKind};
use tolk_syntax::{
    AsCast, Assign, AstNode, Bin, BoolLit, Call, DotAccess, DotAccessField, Expr, HasName, Ident,
    Instantiation, IsType, Lambda, Lazy, Match, MatchArmBody, MatchPattern, NotNull, NullLit,
    NumberLit, ObjectLit, Paren, SetAssign, StringLit, Tensor, Ternary, TopLevel, Tuple, Type,
    Unary, Underscore, VarDecl, VarDeclPattern,
};

impl<'db, 'a, 't> TypeInferenceWalker<'db, 'a> {
    pub(crate) fn infer_expr(
        &mut self,
        v: Expr<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        match v {
            Expr::NumberLit(lit) => self.infer_int_literal(lit, flow, as_cond),
            Expr::StringLit(lit) => self.infer_string_literal(lit, flow, as_cond),
            Expr::BoolLit(lit) => self.infer_bool_literal(lit, flow, as_cond),
            Expr::VarDeclLhs(decl) => {
                // process `var a: int?` without value;
                if let Some(pattern) = decl.pattern() {
                    let mut flow = self.infer_left_side_of_var_declaration(pattern, v, flow);
                    let rhs_ty = self.ctx.get_node_type_or_unknown(&pattern);
                    self.process_var_declaration_lhs_after_infer_rhs(pattern, rhs_ty, &mut flow);
                    return ExprFlow::create(flow, as_cond);
                }

                ExprFlow::create(flow, as_cond)
            }
            Expr::Assign(assignment) => self.infer_assignment(assignment, flow, as_cond),
            Expr::SetAssign(assignment) => self.infer_set_assignment(assignment, flow, as_cond),
            Expr::Unary(unary_op) => self.infer_unary_operator(unary_op, flow, as_cond),
            Expr::Bin(binary_op) => self.infer_binary_expression(binary_op, flow, as_cond),
            Expr::Ternary(ternary_op) => {
                self.infer_ternary_operator(ternary_op, flow, as_cond, hint)
            }
            Expr::AsCast(op) => self.infer_cast_as_operator(op, flow, as_cond),
            Expr::IsType(op) => self.infer_is_type_operator(op, flow, as_cond),
            Expr::NotNull(op) => self.infer_not_null_operator(op, flow, as_cond),
            Expr::Lazy(lazy) => self.infer_lazy_operator(lazy, flow, as_cond),
            Expr::Paren(op) => self.infer_parenthesized(op, flow, as_cond, hint),
            Expr::Ident(ident) => self.infer_reference(ident, flow, as_cond),
            Expr::DotAccess(dot) => self.infer_dot_access(dot, flow, as_cond, hint, None, None),
            Expr::Call(call) => self.infer_function_call(call, flow, as_cond, hint),
            Expr::Match(v) => self.infer_match(v, flow, as_cond, hint),
            Expr::ObjectLit(v) => self.infer_object_literal(v, flow, as_cond, hint),
            Expr::Tensor(expr) => self.infer_tensor(expr, flow, as_cond, hint),
            Expr::Tuple(expr) => self.infer_typed_tuple(expr, flow, as_cond, hint),
            Expr::NullLit(lit) => self.infer_null_literal(lit, flow, as_cond),
            Expr::Lambda(v) => self.infer_lambda_fun(v, flow, as_cond, hint),
            Expr::Instantiation(v) => self.infer_instantiation(v, flow, as_cond),
            Expr::Underscore(v) => self.infer_underscore(v, flow, as_cond, hint),
            _ => ExprFlow::create(flow, as_cond),
        }
    }

    //+ CHECKED
    pub(crate) fn infer_int_literal(
        &mut self,
        v: NumberLit<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let ty = self.intrn().ty_int;
        self.ctx.set_node_type(&v, ty);

        let mut after_v = ExprFlow::create(flow, as_cond);
        if as_cond {
            let value = self.text_of(&v);
            // `if (0)` always false
            if value == "0" {
                after_v
                    .true_flow
                    .mark_unreachable(UnreachableKind::CantHappen)
            } else {
                after_v
                    .false_flow
                    .mark_unreachable(UnreachableKind::CantHappen)
            }
        }
        after_v
    }

    //+ CHECKED
    pub(crate) fn infer_string_literal(
        &mut self,
        v: StringLit<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let ty = self.intrn().ty_string;
        self.ctx.set_node_type(&v, ty);
        ExprFlow::create(flow, as_cond)
    }

    //+ CHECKED
    pub(crate) fn infer_bool_literal(
        &mut self,
        v: BoolLit<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let ty = self.intrn().ty_bool;
        self.ctx.set_node_type(&v, ty);

        let mut after_v = ExprFlow::create(flow, as_cond);
        if as_cond {
            // `if (true)` always true
            if v.value() {
                after_v
                    .false_flow
                    .mark_unreachable(UnreachableKind::CantHappen)
            } else {
                after_v
                    .true_flow
                    .mark_unreachable(UnreachableKind::CantHappen)
            }
        }
        after_v
    }

    //+ CHECKED
    fn infer_local_var_lhs(
        &mut self,
        v: VarDecl<'t>,
        lhs: Expr<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let mut flow = flow;
        // `var v = rhs`, inferring is called for `v`
        // at the moment of inferring left side of assignment, we don't know type of rhs (since lhs is executed first)
        // so, mark `v` as undefined placeholder
        // later, v's inferred_type will be reassigned; see process_assignment_lhs_after_infer_rhs()
        if v.is_redefinition() {
            // for `a: redef` we need to find original declaration of `a` and set its type.
            let span = v.name().map(|n| n.0).span();
            let ty = if let Some(usage) = self.ctx.get_resolved(span) {
                if let Resolved::Local(local) = usage.resolved {
                    let decl_span = Span::from_def_id(local, usage.span.len() as u32);
                    self.ctx
                        .expression_types
                        .get(&decl_span)
                        .cloned()
                        .unwrap_or_else(|| self.const_intrn().ty_undefined)
                } else {
                    self.const_intrn().ty_undefined
                }
            } else {
                self.const_intrn().ty_undefined
            };
            self.ctx.set_node_type(&v, ty);
        } else {
            // if there is type hint, use it as a type
            let ty = self.lower(v.typ());
            self.ctx.set_node_type(&v, ty);
            self.ctx.set_node_type(&lhs, ty);

            if let Some(name) = v.name() {
                self.ctx.set_node_type(&name, ty);
            }

            if let Some(sink) = self.extract_sink_expression_from_var_decl(v) {
                flow.register_known_type(sink, self.intrn().ty_undefined); // undefined before assigned
            }
        }
        ExprFlow::create(flow, as_cond)
    }

    //+ CHECKED
    fn infer_left_side_of_var_declaration(
        &mut self,
        v: VarDeclPattern<'t>,
        lhs: Expr<'t>,
        flow: FlowContext,
    ) -> FlowContext {
        let mut flow = flow;
        match v {
            VarDeclPattern::TensorVars(lhs_tensor) => {
                let vars = lhs_tensor.vars().collect::<Vec<_>>();
                let mut types_list = Vec::with_capacity(vars.len());
                for element in vars {
                    flow = self.infer_left_side_of_var_declaration(element, lhs, flow);
                    types_list.push(self.ctx.get_node_type_or_unknown(&element));
                }
                let ty = self.intrn().tensor(types_list);
                self.ctx.set_node_type(&lhs_tensor, ty);
                self.ctx.set_node_type(&lhs, ty);
            }
            VarDeclPattern::TupleVars(lhs_tuple) => {
                let vars = lhs_tuple.vars().collect::<Vec<_>>();
                let mut types_list = Vec::with_capacity(vars.len());
                for element in vars {
                    flow = self.infer_left_side_of_var_declaration(element, lhs, flow);
                    types_list.push(self.ctx.get_node_type_or_unknown(&element));
                }
                let ty = self.intrn().tuple(types_list);
                self.ctx.set_node_type(&lhs_tuple, ty);
                self.ctx.set_node_type(&lhs, ty);
            }
            VarDeclPattern::VarDecl(var) => {
                let after_lhs = self.infer_local_var_lhs(var, lhs, flow, false);
                flow = after_lhs.out_flow;
            }
        }
        flow
    }

    //+ CHECKED
    fn process_var_declaration_lhs_after_infer_rhs(
        &mut self,
        pat: VarDeclPattern<'t>,
        rhs_ty: TyId,
        flow: &mut FlowContext,
    ) {
        match pat {
            VarDeclPattern::TensorVars(lhs_tensor) => {
                let rhs_unwrapped = self.intrn().unwrap_alias(rhs_ty);
                let rhs_items =
                    if let TyData::Tensor(items) = self.intrn().data(rhs_unwrapped).clone() {
                        items
                    } else {
                        return;
                    };

                let vars: Vec<_> = lhs_tensor.vars().collect();
                let mut types_list = Vec::with_capacity(vars.len());
                for (i, element) in vars.iter().enumerate() {
                    let item_rhs_ty = rhs_items
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| self.intrn().ty_undefined);
                    self.process_var_declaration_lhs_after_infer_rhs(*element, item_rhs_ty, flow);
                    types_list.push(self.ctx.get_node_type_or_unknown(&element.syntax()));
                }
                let ty = self.intrn().tensor(types_list);
                self.ctx.set_node_type(&lhs_tensor, ty);
            }
            VarDeclPattern::TupleVars(lhs_tuple) => {
                let rhs_unwrapped = self.intrn().unwrap_alias(rhs_ty);
                let rhs_items =
                    if let TyData::Tuple(items) = self.intrn().data(rhs_unwrapped).clone() {
                        items
                    } else {
                        return;
                    };

                let vars: Vec<_> = lhs_tuple.vars().collect();
                let mut types_list = Vec::with_capacity(vars.len());
                for (i, element) in vars.iter().enumerate() {
                    let item_rhs_ty = rhs_items
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| self.intrn().ty_undefined);
                    self.process_var_declaration_lhs_after_infer_rhs(*element, item_rhs_ty, flow);
                    types_list.push(self.ctx.get_node_type_or_unknown(&element.syntax()));
                }
                let ty = self.intrn().tuple(types_list);
                self.ctx.set_node_type(&lhs_tuple, ty);
            }
            VarDeclPattern::VarDecl(var) => {
                let current_ty = self.ctx.get_node_type_or_unknown(&var);
                if current_ty == self.intrn().ty_undefined {
                    self.ctx.set_node_type(&var, rhs_ty);

                    if let Some(name) = var.name() {
                        self.ctx.set_node_type(&name, rhs_ty);
                    }
                }

                let declared_type = if var.is_redefinition() {
                    current_ty
                } else {
                    let ty = self.lower(var.typ());
                    self.apply_defaults_to_type(ty)
                };

                let smartcasted_type = if !var.is_redefinition()
                    && declared_type != self.intrn().ty_undefined
                    && rhs_ty != self.intrn().ty_undefined
                {
                    self.calc_smart_cast_type_on_assignment(declared_type, rhs_ty)
                } else {
                    rhs_ty
                };

                if let Some(sink) = self.extract_sink_expression_from_var_decl(var) {
                    flow.register_known_type(sink, smartcasted_type);
                }
            }
        }
    }

    //+ CHECKED
    fn extract_sink_expression_from_var_decl(&self, v: VarDecl<'t>) -> Option<SinkExpr> {
        let name_node = v.name()?;
        let span = name_node.span();
        let def_id = LocalDefId::new(self.ctx.file_id, span.start);
        let name = self.text_of(&name_node);
        Some(SinkExpr::from_def(name, def_id, 0))
    }

    //+ CHECKED
    pub(crate) fn infer_assignment(
        &mut self,
        v: Assign<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        // v is assignment: `x = 5` / `var x = 5` / `var x: slice = 5` / `(cs,_) = f()` / `val (a,[b],_) = (a,t,0)`
        // execution flow is: lhs first, rhs second (at IR generation, also lhs is evaluated first, unlike FunC)
        // after inferring lhs, use it for hint when inferring rhs
        // example: `var i: int = t.tupleAt(0)` is ok (hint=int, T=int), but `var i = t.tupleAt(0)` not, since `tupleAt<T>(t,i): T`

        let lhs = try_expr_flow!(flow, v.left());
        let rhs = try_expr_flow!(flow, v.right());

        let flow = self.infer_left_side_of_assignment(lhs, flow);

        // a: int = t.get(), use `int` as hint for right side
        let lhs_ty = self.ctx.get_node_type(&lhs);
        let flow = self.infer_expr(rhs, flow, false, lhs_ty).out_flow;

        let mut flow = flow;
        let rhs_ty = self.ctx.get_node_type_or_unknown(&rhs);
        self.process_assignment_lhs_after_infer_rhs(lhs, rhs_ty, &mut flow);

        self.ctx.set_node_type(&v, rhs_ty); // note, that the resulting type is rhs, not lhs

        ExprFlow::create(flow, as_cond)
    }

    //+ CHECKED
    pub(crate) fn infer_set_assignment(
        &mut self,
        v: SetAssign<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let lhs = try_expr_flow!(flow, v.left());
        let rhs = try_expr_flow!(flow, v.right());

        let after_lhs = self.infer_expr(lhs, flow, false, None);
        let rhs_flow = after_lhs.out_flow;
        let lhs_ty = self.ctx.get_node_type(&lhs);
        let after_rhs = self.infer_expr(rhs, rhs_flow, false, lhs_ty);

        self.ctx.set_node_type(
            &v,
            lhs_ty.unwrap_or_else(|| self.const_intrn().ty_undefined),
        );

        ExprFlow::create(after_rhs.out_flow, as_cond)
    }

    //+ CHECKED
    /// for `v = rhs` (NOT `var v = lhs`), variable `v` may be smart cast at this point
    /// the purpose of this function is to drop smart casts from expressions used as left side of assignments
    /// another example: `x.0 = rhs`, smart cast is dropped for `x.0` (not for `x`)
    /// the goal of dropping smart casts is to have lhs->inferred_type as actually declared, used as hint to infer rhs
    fn infer_left_side_of_assignment(
        &mut self,
        lhs: Expr<'t>,
        mut flow: FlowContext,
    ) -> FlowContext {
        match lhs {
            Expr::VarDeclLhs(node) => {
                let pattern = try_flow!(flow, node.pattern());
                return self.infer_left_side_of_var_declaration(pattern, lhs, flow);
            }
            Expr::Tensor(lhs_tensor) => {
                let mut types_list = Vec::with_capacity(lhs_tensor.elements().count());
                for element in lhs_tensor.elements() {
                    flow = self.infer_left_side_of_assignment(element, flow);
                    let element_ty = self.ctx.get_node_type_or_unknown(&element);
                    types_list.push(element_ty);
                }
                let ty = self.intrn().tensor(types_list);
                self.ctx.set_node_type(&lhs_tensor, ty);
            }
            Expr::Tuple(lhs_tuple) => {
                let mut types_list = Vec::with_capacity(lhs_tuple.elements().count());
                for element in lhs_tuple.elements() {
                    flow = self.infer_left_side_of_assignment(element, flow);
                    let element_ty = self.ctx.get_node_type_or_unknown(&element);
                    types_list.push(element_ty);
                }
                let ty = self.intrn().tuple(types_list);
                self.ctx.set_node_type(&lhs_tuple, ty);
            }
            Expr::Paren(lhs_paren) => {
                if let Some(inner) = lhs_paren.inner() {
                    flow = self.infer_left_side_of_assignment(inner, flow);
                    let ty = self.ctx.get_node_type_or_unknown(&inner);
                    self.ctx.set_node_type(&lhs_paren, ty);
                }
            }
            _ => {
                flow = self.infer_expr(lhs, flow, false, None).out_flow;
                if self.extract_sink_expression(lhs).is_some() {
                    let lhs_declared_type = self.calc_declared_type_before_smart_cast(lhs);
                    self.ctx.set_node_type(&lhs, lhs_declared_type);
                }
            }
        }
        flow
    }

    //+ CHECKED
    /// handle (and dig recursively) into `var lhs = rhs`
    /// at this point, both lhs and rhs are already inferred, but lhs newly-declared vars are unknown (unless have declared_type)
    /// examples: `var z = 5`, `var (x, [y]) = (2, [3])`, `var (x, [y]) = xy`
    /// the purpose is to update inferred_type of lhs vars (z, x, y)
    /// and to re-assign types of tensors/tuples inside: `var (x,[y]) = ...` was `(unknown,[unknown])`, becomes `(int,[int])`
    /// while recursing, keep track of rhs if lhs and rhs have common shape (5 for z, 2 for x, [3] for [y], 3 for y)
    /// (so that on type mismatch, point to corresponding rhs, example: `var (x, y:slice) = (1, 2)` point to 2
    fn process_assignment_lhs_after_infer_rhs(
        &mut self,
        lhs: Expr<'t>,
        rhs_ty: TyId,
        flow: &mut FlowContext,
    ) {
        match lhs {
            // inside `var v: int = rhs` / `var _ = rhs` / `var v redef = rhs` (lhs is "v" / "_" / "v")
            Expr::VarDeclLhs(node) => {
                if let Some(pattern) = node.pattern() {
                    self.process_var_declaration_lhs_after_infer_rhs(pattern, rhs_ty, flow);
                }
            }
            // `(v1, v2) = rhs` / `var (v1, v2) = rhs` (rhs may be `(1,2)` or `tensorVar` or `someF()`, doesn't matter)
            // dig recursively into v1 and v2 with corresponding rhs i-th item of a tensor
            Expr::Tensor(lhs_tensor) => {
                let rhs_unwrapped = self.intrn().unwrap_alias(rhs_ty);
                let rhs_items =
                    if let TyData::Tensor(items) = self.intrn().data(rhs_unwrapped).clone() {
                        items
                    } else {
                        return;
                    };

                let mut types_list = Vec::with_capacity(lhs_tensor.elements().count());
                for (i, element) in lhs_tensor.elements().enumerate() {
                    let item_rhs_ty = rhs_items
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| self.intrn().ty_undefined);
                    self.process_assignment_lhs_after_infer_rhs(element, item_rhs_ty, flow);
                    types_list.push(self.ctx.get_node_type_or_unknown(&element));
                }
                let ty = self.intrn().tensor(types_list);
                self.ctx.set_node_type(&lhs_tensor.0, ty);
            }
            // `[v1, v2] = rhs` / `var [v1, v2] = rhs` (rhs may be `[1,2]` or `tupleVar` or `someF()`, doesn't matter)
            // dig recursively into v1 and v2 with corresponding rhs i-th item of a tuple
            Expr::Tuple(lhs_tuple) => {
                let rhs_unwrapped = self.intrn().unwrap_alias(rhs_ty);
                let rhs_items =
                    if let TyData::Tuple(items) = self.intrn().data(rhs_unwrapped).clone() {
                        items
                    } else {
                        return;
                    };

                let mut types_list = Vec::with_capacity(lhs_tuple.elements().count());
                for (i, element) in lhs_tuple.elements().enumerate() {
                    let item_rhs_ty = rhs_items
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| self.intrn().ty_undefined);
                    self.process_assignment_lhs_after_infer_rhs(element, item_rhs_ty, flow);
                    types_list.push(self.ctx.get_node_type_or_unknown(&element));
                }
                let ty = self.intrn().tuple(types_list);
                self.ctx.set_node_type(&lhs_tuple.0, ty);
            }
            // `(v) = (rhs)`, just surrounded by parenthesis
            Expr::Paren(lhs_paren) => {
                let Some(inner) = lhs_paren.inner() else {
                    return;
                };
                self.process_assignment_lhs_after_infer_rhs(inner, rhs_ty, flow);
                let ty = self.ctx.get_node_type_or_unknown(&inner);
                self.ctx.set_node_type(&lhs_paren, ty);
            }
            _ => {
                // here is `v = rhs` (just assignment, not `var v = rhs`) / `a.0 = rhs` / `getObj(z=f()).0 = rhs` etc.
                // for instance, `tensorVar.0 = rhs` / `obj.field = rhs` has already checked index correctness while inferring lhs
                // for strange lhs like `f() = rhs` type inferring (and later checking) will pass, but will fail lvalue check later
                if let Some(sink) = self.extract_sink_expression(lhs) {
                    let lhs_declared_type = self.calc_declared_type_before_smart_cast(lhs);
                    let smartcasted_type =
                        self.calc_smart_cast_type_on_assignment(lhs_declared_type, rhs_ty);
                    flow.register_known_type(sink, smartcasted_type);
                    self.ctx.set_node_type(&lhs, lhs_declared_type);
                }
            }
        }
    }

    //+ CHECKED
    pub(crate) fn infer_unary_operator(
        &mut self,
        v: Unary<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let rhs = try_expr_flow!(flow, v.argument());
        let operator = try_expr_flow!(flow, v.operator());
        let operator_name = try_expr_flow!(flow, self.text_or_none(&operator));

        let mut after_rhs = self.infer_expr(rhs, flow, as_cond, None);

        match operator_name.as_str() {
            "-" | "+" | "~" => {
                let ty = self.intrn().ty_int;
                self.ctx.set_node_type(&v, ty);
            }
            "!" => {
                let ty = self.intrn().ty_bool;
                self.ctx.set_node_type(&v, ty);

                std::mem::swap(&mut after_rhs.false_flow, &mut after_rhs.true_flow)
            }
            _ => {
                // unknown operator
            }
        }

        if as_cond {
            ExprFlow::new(
                after_rhs.out_flow,
                after_rhs.true_flow,
                after_rhs.false_flow,
            )
        } else {
            ExprFlow::create(after_rhs.out_flow, false)
        }
    }

    //+ CHECKED
    fn infer_binary_expression(
        &mut self,
        v: Bin<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let lhs = try_expr_flow!(flow, v.left());
        let operator = try_expr_flow!(flow, v.operator());
        let operator_name = try_expr_flow!(flow, self.text_or_none(&operator));

        if operator_name == "??" {
            let after_lhs = self.infer_expr(lhs, flow, false, None);
            let lhs_type = self.ctx.get_node_type_or_unknown(&lhs);

            let Some(rhs) = v.right() else {
                self.ctx.set_node_type(&v, lhs_type);
                return ExprFlow::create(after_lhs.out_flow, as_cond);
            };

            let mut rhs_flow = after_lhs.out_flow.clone();
            if let Some(s_expr) = self.extract_sink_expression(lhs) {
                rhs_flow.register_known_type(s_expr, self.intrn().ty_null);
            }

            let after_rhs = self.infer_expr(rhs, rhs_flow, false, None);
            let ty_null = self.intrn().ty_null;
            let lhs_unwrapped = self.const_intrn().unwrap_alias(lhs_type);
            let without_null_ty =
                if matches!(self.const_intrn().data(lhs_unwrapped), TyData::Union(_)) {
                    self.intrn()
                        .calculate_type_subtract_rhs_type(lhs_type, ty_null)
                } else {
                    lhs_type
                };

            if lhs_type == ty_null {
                let rhs_ty = self.ctx.get_node_type_or_unknown(&rhs);
                self.ctx.set_node_type(&v, rhs_ty);
            } else if without_null_ty == self.intrn().ty_never {
                self.ctx.set_node_type(&v, lhs_type);
            } else {
                let rhs_ty = self.ctx.get_node_type_or_unknown(&rhs);
                let result_ty = self.intrn().calculate_type_lca(without_null_ty, rhs_ty);
                self.ctx.set_node_type(&v, result_ty);
            }

            let out_flow = after_lhs
                .out_flow
                .merge_flow(&after_rhs.out_flow, self.intrn());
            return ExprFlow::create(out_flow, as_cond);
        }

        let rhs = try_expr_flow!(flow, v.right());

        match operator_name.as_str() {
            // comparison operators, returning bool
            "<" | ">" | "<=" | ">=" | "<=>" => {
                let ty = self.intrn().ty_bool;
                let flow = self.infer_expr(lhs, flow, false, None).out_flow;
                let flow = self.infer_expr(rhs, flow, false, None).out_flow;
                self.ctx.set_node_type(&v, ty);
                ExprFlow::create(flow, as_cond)
            }
            "!=" | "==" => {
                let is_negated = operator_name == "!=";

                if let Expr::NullLit(_) = lhs {
                    return self.infer_is_null_check(v, rhs, is_negated, flow, as_cond);
                } else if let Expr::NullLit(_) = rhs {
                    return self.infer_is_null_check(v, lhs, is_negated, flow, as_cond);
                }

                let ty = self.intrn().ty_bool;
                let flow = self.infer_expr(lhs, flow, false, None).out_flow;
                let flow = self.infer_expr(rhs, flow, false, None).out_flow;
                self.ctx.set_node_type(&v, ty);
                ExprFlow::create(flow, as_cond)
            }
            // & | ^ are "overloaded" both for integers and booleans
            "&" | "|" | "^" => {
                let flow = self.infer_expr(lhs, flow, false, None).out_flow;
                let flow = self.infer_expr(rhs, flow, false, None).out_flow;

                let lhs_type = try_expr_flow!(flow, self.ctx.get_node_type(&lhs));
                let rhs_type = try_expr_flow!(flow, self.ctx.get_node_type(&rhs));

                let lhs_type = self.intrn().unwrap_alias(lhs_type);
                let rhs_type = self.intrn().unwrap_alias(rhs_type);

                let bool_ty = self.const_intrn().ty_bool;

                if lhs_type == bool_ty && rhs_type == bool_ty {
                    self.ctx.set_node_type(&v, bool_ty);
                } else {
                    let ty = self.intrn().ty_int;
                    self.ctx.set_node_type(&v, ty);
                }

                ExprFlow::create(flow, as_cond)
            }
            // && || result in booleans, but building flow facts is tricky due to short-circuit
            "&&" => {
                let after_lhs = self.infer_expr(lhs, flow, true, None);
                let after_rhs = self.infer_expr(rhs, after_lhs.true_flow, true, None);

                let ty = self.const_intrn().ty_bool;
                self.ctx.set_node_type(&v, ty);

                let intrn = self.intrn();

                if !as_cond {
                    let out_flow = after_lhs.false_flow.merge_flow(&after_rhs.out_flow, intrn);
                    return ExprFlow::create(out_flow, false);
                }

                let out_flow = after_lhs.out_flow.merge_flow(&after_rhs.out_flow, intrn);
                let true_flow = after_rhs.true_flow;
                let false_flow = after_lhs
                    .false_flow
                    .merge_flow(&after_rhs.false_flow, intrn);

                ExprFlow {
                    out_flow,
                    true_flow,
                    false_flow,
                }
            }
            "||" => {
                let after_lhs = self.infer_expr(lhs, flow, true, None);
                let after_rhs = self.infer_expr(rhs, after_lhs.false_flow, true, None);

                let ty = self.const_intrn().ty_bool;
                self.ctx.set_node_type(&v, ty);

                let intrn = self.intrn();

                if !as_cond {
                    let out_flow = after_lhs.true_flow.merge_flow(&after_rhs.out_flow, intrn);
                    return ExprFlow::create(out_flow, false);
                }

                let out_flow = after_lhs.out_flow.merge_flow(&after_rhs.out_flow, intrn);
                let true_flow = after_lhs.true_flow.merge_flow(&after_rhs.true_flow, intrn);
                let false_flow = after_rhs.false_flow;

                ExprFlow {
                    out_flow,
                    true_flow,
                    false_flow,
                }
            }
            // others are mathematical: + * ...
            // they are allowed for intN (int16 + int32 is ok) and always "fall back" to general int
            _ => {
                let flow = self.infer_expr(lhs, flow, false, None).out_flow;
                let flow = self.infer_expr(rhs, flow, false, None).out_flow;

                let int_ty = self.intrn().ty_int;
                let coins_ty = self.intrn().ty_coins;

                let lhs_type = self.ctx.get_node_type(&lhs);
                if matches!(operator_name.as_str(), "+" | "-") && lhs_type == Some(coins_ty) {
                    self.ctx.set_node_type(&v, coins_ty); // coins + coins = coins
                } else {
                    self.ctx.set_node_type(&v, int_ty); // int8 + int8 = int, as well as other operators/types
                }
                ExprFlow::create(flow, as_cond)
            }
        }
    }

    //+ CHECKED
    fn infer_ternary_operator(
        &mut self,
        v: Ternary<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        let cond = try_expr_flow!(flow, v.condition());
        let after_cond = self.infer_expr(cond, flow, true, None);

        let when_true = try_expr_flow!(after_cond.out_flow, v.consequence());
        let when_false = try_expr_flow!(after_cond.out_flow, v.alternative());

        let after_true = self.infer_expr(when_true, after_cond.true_flow, as_cond, hint);
        let after_false = self.infer_expr(when_false, after_cond.false_flow, as_cond, hint);

        // always true/false omitted, TODO: do we need them?

        let mut branches_unifier = TypeInferringUnifyStrategy::new();
        let true_ty = self.ctx.get_node_type_or_unknown(&when_true);
        let false_ty = self.ctx.get_node_type_or_unknown(&when_false);
        branches_unifier.unify_with(true_ty, hint, self.intrn());
        branches_unifier.unify_with(false_ty, hint, self.intrn());

        // if branches_unifier.is_union_of_different_types() {
        //     // `... ? intVar : sliceVar` results in `int | slice`, probably it's not what the user expected
        //     // example: `var v = ternary`, show an inference error
        //     // do NOT show an error for `var v: T = ternary` (T is hint); it will be checked by type checker later
        //     // TODO: report error if hint is unknown?
        // }

        let ty = branches_unifier.get_result(self.const_intrn());
        self.ctx.set_node_type(&v.0, ty);

        let out_flow = after_true
            .out_flow
            .merge_flow(&after_false.out_flow, self.intrn());

        ExprFlow::new(out_flow, after_true.true_flow, after_false.false_flow)
    }

    //+ CHECKED
    fn infer_cast_as_operator(
        &mut self,
        v: AsCast<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let expr = try_expr_flow!(flow, v.expr());
        let cast_ty = {
            let ty = self.lower(v.casted_to());
            self.apply_defaults_to_type(ty)
        };

        // for `expr as <type>`, use this type for hint, so that `t.tupleAt(0) as int` is ok
        let after_expr = self.infer_expr(expr, flow, false, Some(cast_ty));
        self.ctx.set_node_type(&v, cast_ty);

        if !as_cond {
            return after_expr;
        }

        ExprFlow::create(after_expr.out_flow, true)
    }

    fn infer_is_type_operator(
        &mut self,
        v: IsType<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let expr = try_expr_flow!(flow, v.expr());
        let operator = try_expr_flow!(flow, v.operator());
        let rhs_type = try_expr_flow!(flow, v.rhs_type());

        let after_expr = self.infer_expr(expr, flow, false, None);
        let bool_ty = self.intrn().ty_bool;
        self.ctx.set_node_type(&v, bool_ty);

        let mut rhs_ty = self.lower(v.rhs_type());
        let expr_ty = self.ctx.get_node_type_or_unknown(&expr);

        match self.intrn().data(rhs_ty).clone() {
            TyData::Struct { def, args, .. } if args.is_some() => {
                // `v is Wrapper`, detect T based on type of v (`Wrapper<int> | int` => `Wrapper<int>`)
                if let Some(inst_rhs_type) =
                    self.try_pick_instantiated_generic_from_hint(expr_ty, def)
                {
                    rhs_ty = inst_rhs_type;
                    self.ctx.set_node_type(&rhs_type, rhs_ty);
                }
            }
            TyData::TypeAlias { def, args, .. } if args.is_some() => {
                // `v is WrapperAlias`, detect T similar to structures
                if let Some(inst_rhs_type) =
                    self.try_pick_instantiated_generic_from_hint_alias(expr_ty, def)
                {
                    rhs_ty = inst_rhs_type;
                    self.ctx.set_node_type(&rhs_type, rhs_ty);
                }
            }
            TyData::GenericTypeWithTs { inner_ty, .. } => match self.intrn().data(inner_ty).clone()
            {
                TyData::Struct { def, .. } => {
                    if let Some(inst_rhs_type) =
                        self.try_pick_instantiated_generic_from_hint(expr_ty, def)
                    {
                        rhs_ty = inst_rhs_type;
                        self.ctx.set_node_type(&rhs_type, rhs_ty);
                    }
                }
                TyData::TypeAlias { def, .. } => {
                    if let Some(inst_rhs_type) =
                        self.try_pick_instantiated_generic_from_hint_alias(expr_ty, def)
                    {
                        rhs_ty = inst_rhs_type;
                        self.ctx.set_node_type(&rhs_type, rhs_ty);
                    }
                }
                _ => {}
            },
            _ => {}
        }

        let rhs_ty = self.intrn().unwrap_alias(rhs_ty);
        let non_rhs_ty = self
            .intrn()
            .calculate_type_subtract_rhs_type(expr_ty, rhs_ty);

        // TODO: always true/false

        if !as_cond {
            return after_expr;
        }

        let operator_name = self.text_or_none(&operator);
        let operator_name = operator_name.as_deref().unwrap_or("is");
        let is_negated = operator_name == "!is";

        let mut is_always_true = false;
        let mut is_always_false = false;

        if self.intrn().equals(expr_ty, rhs_ty) {
            // `expr is <type>` is always true
            is_always_true = !is_negated;
            is_always_false = is_negated;
        } else if non_rhs_ty == self.intrn().ty_never {
            // `expr is <type>` is always false
            is_always_true = is_negated;
            is_always_false = !is_negated;
        }

        let mut true_flow = after_expr.out_flow.clone();
        let mut false_flow = after_expr.out_flow.clone();

        if let Some(s_expr) = self.extract_sink_expression(expr) {
            if is_always_true {
                false_flow.mark_unreachable(UnreachableKind::CantHappen);
                false_flow.register_known_type(s_expr, self.intrn().ty_never);
            } else if is_always_false {
                true_flow.mark_unreachable(UnreachableKind::CantHappen);
                true_flow.register_known_type(s_expr, self.intrn().ty_never);
            } else if !is_negated {
                true_flow.register_known_type(s_expr.clone(), rhs_ty);
                false_flow.register_known_type(s_expr, non_rhs_ty);
            } else {
                true_flow.register_known_type(s_expr.clone(), non_rhs_ty);
                false_flow.register_known_type(s_expr, rhs_ty);
            }
        }

        ExprFlow::new(after_expr.out_flow, true_flow, false_flow)
    }

    fn infer_is_null_check(
        &mut self,
        v: Bin<'t>,
        expr: Expr<'t>,
        is_negated: bool,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let after_expr = self.infer_expr(expr, flow, false, None);
        let bool_ty = self.intrn().ty_bool;
        self.ctx.set_node_type(&v, bool_ty);

        let expr_ty = self.ctx.get_node_type_or_unknown(&expr);
        let rhs_ty = self.intrn().ty_null;
        let non_rhs_ty = self
            .intrn()
            .calculate_type_subtract_rhs_type(expr_ty, rhs_ty);

        if !as_cond {
            return after_expr;
        }

        let mut is_always_true = false;
        let mut is_always_false = false;

        if self.intrn().equals(expr_ty, rhs_ty) {
            // `expr == null` is always true
            is_always_true = !is_negated;
            is_always_false = is_negated;
        } else if non_rhs_ty == self.intrn().ty_never {
            // `expr == null` is always false
            is_always_true = is_negated;
            is_always_false = !is_negated;
        }

        let mut true_flow = after_expr.out_flow.clone();
        let mut false_flow = after_expr.out_flow.clone();

        if let Some(s_expr) = self.extract_sink_expression(expr) {
            if is_always_true {
                false_flow.mark_unreachable(UnreachableKind::CantHappen);
                false_flow.register_known_type(s_expr, self.intrn().ty_never);
            } else if is_always_false {
                true_flow.mark_unreachable(UnreachableKind::CantHappen);
                true_flow.register_known_type(s_expr, self.intrn().ty_never);
            } else if !is_negated {
                true_flow.register_known_type(s_expr.clone(), rhs_ty);
                false_flow.register_known_type(s_expr, non_rhs_ty);
            } else {
                true_flow.register_known_type(s_expr.clone(), non_rhs_ty);
                false_flow.register_known_type(s_expr, rhs_ty);
            }
        }

        ExprFlow::new(after_expr.out_flow, true_flow, false_flow)
    }

    //+ CHECKED
    /// given `lhs = rhs` (and `var x = rhs`), calculate probable smart cast for lhs
    /// it's NOT directly type of rhs! see comment at the top of the file about internal structure of tensors/tuples.
    /// obvious example: `var x: int? = 5`, it's `int` (most cases are like this)
    /// obvious example: `var x: (int,int)? = null`, it's `null` (`x == null` is always true, `x` can be passed to any `T?`)
    /// not obvious example: `var x: (int?, int?)? = (3,null)`, result is `(int?,int?)`, whereas type of rhs is `(int,null)`
    fn calc_smart_cast_type_on_assignment(
        &mut self,
        lhs_declared_type: TyId,
        rhs_inferred_type: TyId,
    ) -> TyId {
        let intrn = self.const_intrn();
        let lhs_unwrapped = intrn.unwrap_alias(lhs_declared_type);
        if let TyData::Union(lhs_variants) = intrn.data(lhs_unwrapped).clone() {
            // example: `var x: T? = null`, result is null
            // example: `var x: int | (int, User?) = (5, null)`, result is `(int, User?)`
            if let Some(lhs_subtype) = intrn.calculate_exact_variant_to_fit_rhs(
                lhs_unwrapped,
                &lhs_variants,
                rhs_inferred_type,
            ) {
                return lhs_subtype;
            }

            // example: `var x: int | slice | cell = 4`, result is int
            // example: `var x: T1 | T2 | T3 = y as T3 | T1`, result is `T1 | T3`
            if let TyData::Union(rhs_variants) = intrn.data(rhs_inferred_type).clone()
                && intrn.has_all_variants_of(lhs_unwrapped, rhs_inferred_type)
                && rhs_variants.len() < lhs_variants.len()
            {
                let mut subtypes_of_lhs = Vec::new();
                for &lhs_variant in &lhs_variants {
                    if intrn.has_variant_equal_to(&rhs_variants, lhs_variant) {
                        subtypes_of_lhs.push(lhs_variant);
                    }
                }
                if subtypes_of_lhs.len() == 1 {
                    return subtypes_of_lhs[0];
                }
                return self.intrn().union(subtypes_of_lhs);
            }
        }

        // no smart cast, type is the same as declared
        // example: `var x: (int?,slice?) = (1, null)`, it's `(int?,slice?)`, not `(int,null)`
        lhs_declared_type
    }

    //+ CHECKED
    /// given `lhs = rhs`, calculate "original" type of `lhs`
    /// example: `var x: int? = ...; if (x != null) { x (here) = null; }`
    /// "(here)" x is `int` (smart cast), but originally declared as `int?`
    /// example: `if (x is (int,int)?) { x!.0 = rhs }`, here `x!.0` is `int`
    fn calc_declared_type_before_smart_cast(&mut self, expr: Expr<'t>) -> TyId {
        match expr {
            Expr::Ident(ident) => {
                let span = ident.span();
                if let Some(usage) = self.ctx.get_resolved(span)
                    && let Resolved::Local(local) = usage.resolved
                {
                    let decl_span = Span::from_def_id(local, usage.span.len() as u32);
                    return self
                        .ctx
                        .expression_types
                        .get(&decl_span)
                        .cloned()
                        .unwrap_or_else(|| self.intrn().ty_undefined);
                }
            }
            Expr::DotAccess(dot) => {
                let Some(obj) = dot.obj() else {
                    return self.ctx.get_node_type_or_unknown(&expr);
                };

                let obj_ty = self.ctx.get_node_type_or_unknown(&obj.syntax());
                let obj_ty = self.intrn().unwrap_alias(obj_ty);

                let Some(field) = dot.field() else {
                    return self.intrn().ty_undefined;
                };

                match field {
                    DotAccessField::Ident(ident) => {
                        if let Some(usage) = self.ctx.get_resolved_node(&ident.0)
                            && let Resolved::Global(sym_id) = usage.resolved
                            && let Some(ty) = self.ctx.get_top_level_type(sym_id)
                        {
                            return ty;
                        }
                    }
                    DotAccessField::NumericIndex(idx) => {
                        let index_str = self.text_of(&idx);
                        if let Ok(index_at) = index_str.parse::<usize>() {
                            match self.intrn().data(obj_ty) {
                                TyData::Tensor(items) => {
                                    if let Some(ty) = items.get(index_at) {
                                        return *ty;
                                    }
                                }
                                TyData::Tuple(items) => {
                                    if let Some(ty) = items.get(index_at) {
                                        return *ty;
                                    }
                                }
                                _ => {
                                    if let Some(item_ty) = self.array_element_type(obj_ty) {
                                        return item_ty;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        self.ctx.get_node_type_or_unknown(&expr)
    }

    /// from `expr!` get `expr`
    fn unwrap_not_null(&self, expr: Expr<'t>) -> Expr<'t> {
        let mut cur = expr;
        while let Expr::NotNull(op) = cur {
            if let Some(inner) = op.inner() {
                cur = inner;
            } else {
                break;
            }
        }
        cur
    }

    /// given any expression vertex, extract SinkExpression is possible
    /// example: `x.0` is { var_ref: x, index_path: 1 }
    /// example: `x.1` is { var_ref: x, index_path: 2 }
    /// example: `x!.1` is the same
    /// example: `x.1.2` is { var_ref: x, index_path: 2<<8 + 3 }
    /// example: `x!.1!.2` is the same
    /// not SinkExpressions: `globalVar` / `f()` / `obj.method().1`
    fn extract_sink_expression(&self, expr: Expr<'t>) -> Option<SinkExpr> {
        match expr {
            Expr::VarDeclLhs(node) => {
                return if let Some(VarDeclPattern::VarDecl(v)) = node.pattern() {
                    self.extract_sink_expression_from_var_decl(v)
                } else {
                    None
                };
            }
            Expr::Ident(ident) => {
                let span = ident.span();
                if let Some(usage) = self.ctx.get_resolved(span)
                    && let Resolved::Local(def_id) = usage.resolved
                {
                    let name = self.text_of(&ident);
                    return Some(SinkExpr::from_def(name, def_id, 0));
                }
            }
            Expr::DotAccess(dot) => {
                let mut cur_dot = dot;
                let mut index_path: u64 = 0;
                while let Some(field) = cur_dot.field() {
                    let index_at = match field {
                        DotAccessField::NumericIndex(idx) => {
                            let text = self.text_of(&idx);
                            text.parse::<u64>().unwrap_or(0)
                        }
                        DotAccessField::Ident(ident) => {
                            let name = self.text_of(&ident);
                            let name = name.as_str();

                            let obj_ty = self.ctx.get_node_type(&cur_dot.obj()?.syntax())?;
                            let def = self.ctx.type_db.find_struct(obj_ty)?;

                            let resolved = self.ctx.type_db.project_index.resolve_symbol(def)?;
                            let SymbolKind::Struct { fields, .. } = &resolved.kind else {
                                return None;
                            };

                            fields
                                .iter()
                                .position(|field| field.name == name.into())
                                .unwrap_or(0) as u64
                        }
                    };

                    index_path = (index_path << 8) + index_at + 1;

                    let Some(obj) = cur_dot.obj() else {
                        break;
                    };

                    if let Expr::DotAccess(parent_dot) = self.unwrap_not_null(obj) {
                        cur_dot = parent_dot;
                    } else {
                        break;
                    }
                }

                if let Expr::Ident(ident) = self.unwrap_not_null(cur_dot.obj()?)
                    && let Some(usage) = self.ctx.get_resolved_node(&ident.0)
                    && let Resolved::Local(def_id) = usage.resolved
                {
                    let name = self.text_of(&ident);
                    return Some(SinkExpr::from_def(name, def_id, index_path));
                }
            }
            Expr::Paren(par) => {
                if let Some(inner) = par.inner() {
                    return self.extract_sink_expression(inner);
                }
            }
            Expr::Assign(assign) => {
                if let Some(lhs) = assign.left() {
                    return self.extract_sink_expression(lhs);
                }
            }
            _ => {}
        }
        None
    }

    fn collect_union_variants_from_hint(&mut self, hint: TyId, depth: usize) -> Option<Vec<TyId>> {
        if depth > 8 {
            return None;
        }

        let unwrapped = self.const_intrn().unwrap_alias(hint);
        match self.const_intrn().data(unwrapped).clone() {
            TyData::Union(variants) => Some(variants),
            TyData::TypeAlias { inner_ty, .. } => {
                self.collect_union_variants_from_hint(inner_ty, depth + 1)
            }
            TyData::GenericTypeWithTs { inner_ty, types } => {
                let inner_data = self.const_intrn().data(inner_ty).clone();
                if let TyData::TypeAlias {
                    inner_ty: alias_inner,
                    args: Some(formal_args),
                    ..
                } = inner_data
                {
                    let mut mapping = FxHashMap::default();
                    for (&formal, &actual) in formal_args.iter().zip(types.iter()) {
                        if let TyData::TypeParameter { name, .. } = self.const_intrn().data(formal)
                        {
                            mapping.insert(name.clone(), actual);
                        }
                    }

                    let instantiated = if mapping.is_empty() {
                        alias_inner
                    } else {
                        let mut substitutor = TypeSubstitutor::new(self.intrn());
                        substitutor.substitute(alias_inner, &mapping)
                    };
                    return self.collect_union_variants_from_hint(instantiated, depth + 1);
                }

                self.collect_union_variants_from_hint(inner_ty, depth + 1)
            }
            _ => None,
        }
    }

    fn struct_def_of(&self, ty: TyId) -> Option<SymbolId> {
        let unwrapped = self.const_intrn().unwrap_alias(ty);
        match self.const_intrn().data(unwrapped) {
            TyData::Struct { def, .. } => Some(*def),
            TyData::GenericTypeWithTs { inner_ty, .. } => {
                if let TyData::Struct { def, .. } = self.const_intrn().data(*inner_ty) {
                    Some(*def)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn type_alias_def_of(&self, ty: TyId) -> Option<SymbolId> {
        let unwrapped = self.const_intrn().unwrap_alias(ty);
        match self.const_intrn().data(unwrapped) {
            TyData::TypeAlias { def, .. } => Some(*def),
            TyData::GenericTypeWithTs { inner_ty, .. } => {
                if let TyData::TypeAlias { def, .. } = self.const_intrn().data(*inner_ty) {
                    Some(*def)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// helper function: given hint = `Ok<int> | Err<slice>` and struct `Ok`, return `Ok<int>`
    /// example: `match (...) { Ok => ... }` we need to deduce `Ok<T>` based on subject
    fn try_pick_instantiated_generic_from_hint(
        &mut self,
        hint: TyId,
        lookup_def: SymbolId,
    ) -> Option<TyId> {
        let unwrapped = self.const_intrn().unwrap_alias(hint);
        if self
            .struct_def_of(unwrapped)
            .is_some_and(|def| def == lookup_def)
        {
            return Some(unwrapped);
        }

        let variants = self.collect_union_variants_from_hint(hint, 0)?;
        let mut only_variant = None; // hint `Ok<int8> | Ok<int16>` is ambiguous
        for variant in variants {
            let v_unwrapped = self.const_intrn().unwrap_alias(variant);
            if self
                .struct_def_of(v_unwrapped)
                .is_some_and(|def| def == lookup_def)
            {
                if only_variant.is_some() {
                    return None; // Ambiguous
                }
                only_variant = Some(v_unwrapped);
            }
        }

        only_variant
    }

    /// helper function, similar to the above, but for generic type aliases
    /// example: `v is OkAlias`, need to deduce `OkAlias<T>` based on type of v
    fn try_pick_instantiated_generic_from_hint_alias(
        &mut self,
        hint: TyId,
        lookup_def: SymbolId,
    ) -> Option<TyId> {
        let unwrapped = self.const_intrn().unwrap_alias(hint);
        if self
            .type_alias_def_of(unwrapped)
            .is_some_and(|def| def == lookup_def)
        {
            return Some(unwrapped);
        }

        let variants = self.collect_union_variants_from_hint(hint, 0)?;
        let mut only_variant = None;
        for variant in variants {
            let v_unwrapped = self.const_intrn().unwrap_alias(variant);
            if self
                .type_alias_def_of(v_unwrapped)
                .is_some_and(|def| def == lookup_def)
            {
                if only_variant.is_some() {
                    return None; // Ambiguous
                }
                only_variant = Some(v_unwrapped);
            }
        }

        only_variant
    }

    //+ CHECKED
    fn infer_not_null_operator(
        &mut self,
        v: NotNull<'t>,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let expr = try_expr_flow!(flow, v.inner());
        let after_expr = self.infer_expr(expr, flow, false, None);
        let expr_ty = self.ctx.get_node_type_or_unknown(&expr);

        let intrn = self.intrn();
        let ty_null = intrn.ty_null;

        let without_null_type = intrn.calculate_type_subtract_rhs_type(expr_ty, ty_null);
        let ty_never = intrn.ty_never;
        let final_ty = if without_null_type != ty_never {
            without_null_type
        } else {
            expr_ty
        };
        self.ctx.set_node_type(&v, final_ty);

        if !as_cond {
            return after_expr;
        }

        ExprFlow::create(after_expr.out_flow, true)
    }

    //+ CHECKED
    fn infer_lazy_operator(&mut self, v: Lazy<'t>, flow: FlowContext, as_cond: bool) -> ExprFlow {
        let expr = try_expr_flow!(flow, v.expr());
        let lazy_expr = self.infer_expr(expr, flow, as_cond, None);
        let ty = self.ctx.get_node_type_or_unknown(&expr);
        self.ctx.set_node_type(&v, ty); // there is no Lazy<T>, so `lazy expr` is just typeof expr
        lazy_expr
    }

    //+ CHECKED
    fn infer_parenthesized(
        &mut self,
        v: Paren<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        let inner = try_expr_flow!(flow, v.inner());
        let after_expr = self.infer_expr(inner, flow, as_cond, hint);
        let ty = self.ctx.get_node_type_or_unknown(&inner);
        self.ctx.set_node_type(&v, ty);
        after_expr
    }

    fn infer_reference(&mut self, ident: Ident, flow: FlowContext, as_cond: bool) -> ExprFlow {
        // at current point, v is a reference:
        // - either a standalone: `local_var` / `SOME_CONST` / `globalF` / `genericFn<int>`
        // - or inside a call: `globalF()` / `genericFn()` / `genericFn<int>()` / `local_var()`

        let Some(resolved) = self.ctx.get_resolved_node(&ident) else {
            return ExprFlow::create(flow, as_cond);
        };

        // local variables, parameters, type parameters
        if let Resolved::Local(def_id) = resolved.resolved {
            let name = self.text_of(&ident);
            let sink_expr = SinkExpr::from_def(name, def_id, 0);
            let span = Span::from_def_id(def_id, resolved.span.len() as u32);

            let mut declared_type = self.ctx.get_type(span);

            // Local type parameters declared in method receivers (`fun T.foo()`)
            // may not have a value-type entry in flow state, so recover them from resolver locals.
            if declared_type.is_none()
                && let Some(resolved_uses) = self
                    .ctx
                    .type_db
                    .project_index
                    .get_resolved_uses(def_id.file_id)
                && let Some(local) = resolved_uses.find_local(def_id)
                && matches!(local.kind, LocalDefKind::TypeParameter)
            {
                declared_type = Some(self.intrn().type_parameter(local.name.to_string(), None));
            }

            let declared_type = declared_type.unwrap_or_else(|| self.intrn().ty_undefined);

            let declared_or_smart_casted =
                flow.smart_cast_or_original(sink_expr, declared_type, self.intrn());

            self.ctx.set_node_type(&ident, declared_or_smart_casted);
            return ExprFlow::create(flow, as_cond);
        }

        // constants, globals, functions, type aliases
        if let Resolved::Global(def_id) = resolved.resolved {
            let decl_type = self.ctx.get_top_level_type(def_id);
            if let Some(decl_type) = decl_type {
                self.ctx.set_node_type(&ident, decl_type);
            }
        }

        ExprFlow::create(flow, as_cond)
    }

    pub(crate) fn infer_dot_access(
        &mut self,
        v: DotAccess<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
        out_f_called: Option<&mut Option<SymbolId>>,
        out_dot_obj: Option<&mut Option<Expr<'t>>>,
    ) -> ExprFlow {
        // at current point, v is a dot access to a field / index / method:
        // - either a standalone: `user.id` / `getUser().id` / `var.0` / `t.size` / `Point.create` / `t.tupleAt<slice>`
        // - or inside a call: `user.getId()` / `<any_expr>.method()` / `Point.create()` / `t.tupleAt<slice>(1)`

        let mut flow = flow;

        let obj = try_expr_flow!(flow, v.obj());
        let field = try_expr_flow!(flow, v.field());

        let mut is_static_call = false; // to be filled for `<dot_obj>.(field/index/method)`, nullptr for `Point.create`
        let mut fun_ref: Option<SymbolId> = None; // to be filled for `<any_expr>.method` / `Point.create` (both standalone or in a call)
        let mut substituted_ts = GenericsSubstitutions::new();

        flow = self.infer_expr(obj, flow, false, None).out_flow;

        let obj_type = self
            .ctx
            .get_node_type(&obj)
            .unwrap_or_else(|| self.const_intrn().ty_undefined);

        let unwrapped_obj_type = self.intrn().unwrap_alias(obj_type);

        // a.foo
        //   ^^^^ field
        let field_name = match field {
            DotAccessField::Ident(ident) => self.text_of(&ident),
            DotAccessField::NumericIndex(idx) => {
                self.text_or_none(&idx).unwrap_or_else(|| "0".into())
            }
        };

        // handle `Point.create` / `Container<int>.wrap` / `Color.Red`: lhs is a type, looking up a constant/method
        if let Some(Expr::Ident(obj)) = v.obj()
            && let Some(usage) = self.ctx.get_resolved_node(&obj)
        {
            match usage.resolved {
                Resolved::Global(sym_id) => {
                    if let Some(receiver_type) = self.ctx.get_top_level_type(sym_id) {
                        // `Color.Red` (enum member) — just fill v->target and done
                        let unwrapped = self.intrn().unwrap_alias(receiver_type);
                        if let TyData::Enum { def, .. } = self.const_intrn().data(unwrapped) {
                            let member = self.ctx.type_db.find_enum_member(*def, &field_name);
                            if let Some(member) = member {
                                self.ctx.set_resolved(NameUse {
                                    decl: self.ctx.decl_start,
                                    span: field.span(),
                                    kind: NameUseKind::Value,
                                    name: field_name.into(),
                                    resolved: Resolved::Global(member.id),
                                });
                                self.ctx.set_node_type(&v, unwrapped); // `ColorAlias.Red` is `Color`
                                self.ctx.set_node_type(&field, unwrapped); // type of field itself
                                return ExprFlow::create(flow, as_cond);
                            }
                        }

                        if let Ok(Some(candidate)) =
                            self.choose_only_method_to_call(&field_name, receiver_type)
                        {
                            is_static_call = true;
                            fun_ref = Some(candidate.method_id);
                            substituted_ts.mapping = candidate.substitutions;
                        }
                    }
                }
                Resolved::Local(local_id) => {
                    if let Some(resolved_uses) = self
                        .ctx
                        .type_db
                        .project_index
                        .get_resolved_uses(local_id.file_id)
                        && let Some(local) = resolved_uses.find_local(local_id)
                        && matches!(local.kind, LocalDefKind::TypeParameter)
                    {
                        // `T.fromSlice(...)` where `T` comes from method receiver.
                        is_static_call = true;
                    }
                }
                Resolved::Unresolved => {}
            }
        }

        // Wrapper<T>.foo
        if let Some(Expr::Instantiation(_)) = v.obj() {
            is_static_call = true;
        }

        // check for field access (`user.id`) when obj resolves to a struct,
        // including generic wrappers like `Expectation<T>`
        if let Some(def_id) = self.ctx.type_db.find_struct(unwrapped_obj_type) {
            let struct_ty = self
                .ctx
                .type_db
                .get_top_level_type(None, def_id)
                .unwrap_or_default();

            if let Some(field_def) = self.ctx.type_db.find_struct_field(def_id, &field_name) {
                self.ctx.set_resolved(NameUse {
                    decl: self.ctx.decl_start,
                    span: field.span(),
                    kind: NameUseKind::Value,
                    name: field_name.into(),
                    resolved: Resolved::Global(field_def.id),
                });
                let mut inferred_type = field_def.declared_type;

                let mut deducer = GenericSubstitutionsDeducing::new();
                deducer.auto_deduce_from_argument(struct_ty, unwrapped_obj_type, self.intrn());

                if self.const_intrn().has_generics(inferred_type) {
                    let mut substitutor = TypeSubstitutor::new(self.intrn());
                    inferred_type =
                        substitutor.substitute(inferred_type, &deducer.substitutions.mapping)
                }

                if let Some(s_expr) = self.extract_sink_expression(Expr::DotAccess(v)) {
                    inferred_type =
                        flow.smart_cast_or_original(s_expr, inferred_type, self.intrn());
                }

                self.ctx.set_node_type(&v, inferred_type);
                self.ctx.set_node_type(&field, inferred_type);
                return ExprFlow::create(flow, as_cond);
            }
            // if field_name doesn't exist, don't fire an error now — maybe, it's `user.method()`
        }

        // check for indexed access (`tensorVar.0` / `tupleVar.1`)
        if fun_ref.is_none()
            && let DotAccessField::NumericIndex(_) = field
            && let Ok(index_at) = field_name.parse::<usize>()
        {
            match self.intrn().data(unwrapped_obj_type).clone() {
                TyData::Tensor(items) => {
                    if index_at >= items.len() {
                        let unknown_ty = self.const_intrn().ty_unknown;
                        self.ctx.set_node_type(&v, unknown_ty);
                        self.ctx.set_node_type(&field, unknown_ty);
                        return ExprFlow::create(flow, as_cond);
                    }

                    let mut inferred_type = items[index_at];
                    if let Some(s_expr) = self.extract_sink_expression(Expr::DotAccess(v)) {
                        inferred_type =
                            flow.smart_cast_or_original(s_expr, inferred_type, self.intrn());
                    }
                    self.ctx.set_node_type(&v, inferred_type);
                    self.ctx.set_node_type(&field, inferred_type);
                    return ExprFlow::create(flow, as_cond);
                }
                TyData::Tuple(items) => {
                    if index_at >= items.len() {
                        let unknown_ty = self.const_intrn().ty_unknown;
                        self.ctx.set_node_type(&v, unknown_ty);
                        self.ctx.set_node_type(&field, unknown_ty);
                        return ExprFlow::create(flow, as_cond);
                    }

                    let mut inferred_type = items[index_at];
                    if let Some(s_expr) = self.extract_sink_expression(Expr::DotAccess(v)) {
                        inferred_type =
                            flow.smart_cast_or_original(s_expr, inferred_type, self.intrn());
                    }
                    self.ctx.set_node_type(&v, inferred_type);
                    self.ctx.set_node_type(&field, inferred_type);
                    return ExprFlow::create(flow, as_cond);
                }
                TyData::UntypedTuple => {
                    let item_type = hint.unwrap_or_else(|| self.intrn().ty_undefined);

                    self.ctx.set_node_type(&v, item_type);
                    self.ctx.set_node_type(&field, item_type);
                    return ExprFlow::create(flow, as_cond);
                }
                _ => {
                    if let Some(mut inferred_type) = self.array_element_type(unwrapped_obj_type) {
                        if let Some(s_expr) = self.extract_sink_expression(Expr::DotAccess(v)) {
                            inferred_type =
                                flow.smart_cast_or_original(s_expr, inferred_type, self.intrn());
                        }
                        self.ctx.set_node_type(&v, inferred_type);
                        self.ctx.set_node_type(&field, inferred_type);
                        return ExprFlow::create(flow, as_cond);
                    }
                }
            }
        }

        // check for method (`t.size` / `user.getId`); even `i.0()` can be here if `fun int.0(self)` exists
        // for `T.copy` / `Container<T>.create`, substitution for T is also returned
        if fun_ref.is_none()
            && let Ok(Some(candidate)) = self.choose_only_method_to_call(&field_name, obj_type)
        {
            fun_ref = Some(candidate.method_id);
            substituted_ts.mapping = candidate.substitutions;
        }

        if let Some(out_f) = out_f_called {
            // so, it's `user.method()` / `t.tupleAt()` / `t.tupleAt<int>()` / `Point.create`
            *out_f = fun_ref; // (it's still may be a generic one, then Ts will be deduced from arguments)
            if let Some(out_o) = out_dot_obj
                && !is_static_call
            {
                *out_o = Some(obj);
            }
            if let Some(fun_ref) = fun_ref {
                let typ = self.ctx.get_top_level_type(fun_ref);
                if let Some(mut typ) = typ {
                    if self.const_intrn().has_generics(typ) {
                        let mut substitutor = TypeSubstitutor::new(self.intrn());
                        typ = substitutor.substitute(typ, &substituted_ts.mapping)
                    }
                    self.ctx.set_node_type(&v, typ);
                    self.ctx.set_node_type(&field, typ);
                }
                self.ctx.set_resolved(NameUse {
                    decl: self.ctx.decl_start,
                    span: field.span(),
                    kind: NameUseKind::Value,
                    name: field_name.into(),
                    resolved: Resolved::Global(fun_ref),
                })
            }
        } else if let Some(fun_ref) = fun_ref {
            let symbol = self.ctx.type_db.project_index.resolve_symbol(fun_ref);
            let mut typ = self.ctx.get_top_level_type(fun_ref);
            if let Some(current) = typ
                && self.const_intrn().has_generics(current)
            {
                let mut substitutor = TypeSubstitutor::new_with_defaults(self.intrn());
                typ = Some(substitutor.substitute(current, &substituted_ts.mapping));
            }
            let f_callable = typ
                .map(|t| self.const_intrn().unwrap_alias(t))
                .and_then(|t| self.return_type_or_none(t));

            if let Some(f_callable) = f_callable
                && let Some(symbol) = symbol
            {
                let mut return_ty = f_callable.1;

                // if return type is omitted we need to infer function body first
                // once inferred, subsequent `return_ty` for this function will be non-auto
                if return_ty == self.const_intrn().ty_auto
                    && let Some(inferred_ty) = self.infer_auto_return_type_of_function(symbol)
                {
                    let func_ty = self.intrn().func(f_callable.0.clone(), inferred_ty);
                    self.ctx.set_top_level_type(symbol.id, func_ty);
                    return_ty = inferred_ty
                }

                let func_ty = self.intrn().func(f_callable.0, return_ty);
                self.ctx.set_node_type(&v, func_ty);
            }

            self.ctx.set_resolved(NameUse {
                decl: self.ctx.decl_start,
                span: field.span(),
                kind: NameUseKind::Value,
                name: field_name.into(),
                resolved: Resolved::Global(fun_ref),
            });
        }

        ExprFlow::create(flow, as_cond)
    }

    fn choose_only_method_to_call(
        &mut self,
        field_name: &SmolStr,
        receiver_type: TyId,
    ) -> Result<Option<MethodCallCandidate>, String> {
        let key = MethodKey(receiver_type, field_name.clone());
        if let Some(cached) = self.ctx.computed_methods.get(&key) {
            return Ok(cached.clone());
        }

        let result = choose_only_method_to_call(receiver_type, field_name, self.ctx.type_db)?;
        self.ctx.computed_methods.insert(key, result.clone());
        Ok(result)
    }

    fn infer_function_call(
        &mut self,
        v: Call<'t>,
        flow: FlowContext,
        as_cond: bool,
        _hint: Option<TyId>,
    ) -> ExprFlow {
        let mut flow = flow;
        let callee = try_expr_flow!(flow, v.callee());

        // v is `globalF(args)` / `globalF<int>(args)` / `obj.method(args)` / `local_var(args)` / `getF()(args)`
        let mut self_obj: Option<Expr<'t>> = None; // for `obj.method()`, obj will be here (but for `Point.create()`, no obj exists)
        let mut fun_ref: Option<SymbolId> = None;
        let mut instantiation_types: Option<Vec<Type<'t>>> = None;

        let actual_callee = if let Expr::Instantiation(inst) = callee {
            instantiation_types = inst
                .instantiation_ts()
                .map(|t| t.types().collect::<Vec<_>>());
            inst.expr().unwrap_or(callee)
        } else {
            callee
        };

        if let Expr::Ident(ident) = actual_callee {
            flow = self.infer_reference(ident, flow, false).out_flow;

            if let Some(resolved) = self.ctx.get_resolved_node(&ident) {
                if let Resolved::Global(id) = resolved.resolved {
                    fun_ref = Some(id);
                } else if let Resolved::Local(_) = resolved.resolved {
                    // Local variable call (lambda?) - handled below via inferred type
                }
            }
        } else if let Expr::DotAccess(dot) = actual_callee {
            flow = self
                .infer_dot_access(
                    dot,
                    flow,
                    false,
                    None,
                    Some(&mut fun_ref),
                    Some(&mut self_obj),
                )
                .out_flow;
        } else {
            flow = self.infer_expr(actual_callee, flow, false, None).out_flow;

            if let Some(resolved) = self.ctx.get_resolved_node(&callee) {
                if let Resolved::Global(id) = resolved.resolved {
                    fun_ref = Some(id);
                } else if let Resolved::Local(_) = resolved.resolved {
                    // Local variable call (lambda?) - handled below via inferred type
                }
            }
        }

        // callee must have "callable" inferred type
        let f_callable = self
            .ctx
            .get_node_type(&actual_callee)
            .map(|t| self.const_intrn().unwrap_alias(t))
            .and_then(|t| self.return_type_or_none(t));
        let Some(f_callable) = f_callable else {
            // fallback, at least infer arguments
            for arg in v.arguments() {
                let Some(arg_expr) = arg.expr() else { continue };
                flow = self.infer_expr(arg_expr, flow, false, None).out_flow;
            }
            return ExprFlow::create(flow, as_cond);
        };

        // handle `local_var()` / `getF()()` / `5()` / `SOME_CONST()` / `obj.method()()()` / `tensorVar.0()`
        if fun_ref.is_none() {
            for (i, arg) in v.arguments().enumerate() {
                let Some(arg_expr) = arg.expr() else { continue };
                let Some(param_ty) = f_callable.0.get(i) else {
                    continue;
                };
                flow = self
                    .infer_expr(arg_expr, flow, false, Some(*param_ty))
                    .out_flow;
                let arg_ty = self.ctx.get_node_type(&arg);
                let Some(arg_ty) = arg_ty else { continue };
                self.ctx.set_node_type(&v, arg_ty);
            }

            self.ctx.set_node_type(&v, f_callable.1);
            return ExprFlow::create(flow, as_cond);
        }

        // from now, we know which function we call
        let fun_ref = fun_ref.expect("unreachable");
        if let Some(parent_function) = self.ctx.caller_function {
            self.ctx
                .type_db
                .call_graph
                .entry(parent_function)
                .or_default()
                .insert(fun_ref);
            self.ctx
                .type_db
                .inverted_call_graph
                .entry(fun_ref)
                .or_default()
                .insert(parent_function);
        }
        // so, we have a call `f(args)` or `obj.f(args)`, f is fun_ref (function / method) (code / asm / builtin)
        // we're going to iterate over passed arguments, and (if generic) infer substitutedTs
        // at first, check argument count
        let delta_self = if self_obj.is_some() { 1 } else { 0 };

        // for every passed argument, we need to infer its type
        // for generic functions, we need to infer type arguments (substitutedTs) on the fly
        // (if they are specified by a user like `f<int>(args)` / `t.tupleAt<slice>()`, fun_ref is already instantiated)
        let mut deducing_ts = GenericSubstitutionsDeducing::new();

        let Some(declaration) = self.ctx.type_db.project_index.resolve_symbol(fun_ref) else {
            // something strange if we cannot resolve reference
            return ExprFlow::create(flow, as_cond);
        };
        let (parameters, type_parameters) = match &declaration.kind {
            SymbolKind::Function {
                parameters,
                type_parameters,
                ..
            } => (parameters, type_parameters),
            SymbolKind::Method {
                parameters,
                type_parameters,
                ..
            } => (parameters, type_parameters),
            SymbolKind::GetMethod {
                parameters,
                type_parameters,
                ..
            } => (parameters, type_parameters),
            _ => return ExprFlow::create(flow, as_cond),
        };

        if let Some(instantiation_types) = instantiation_types {
            for (type_parameter, ty) in type_parameters.iter().zip(instantiation_types) {
                let ty = self.ctx.type_db.lower_type(self.ctx.file_id, &ty);
                deducing_ts
                    .substitutions
                    .set_type_t(type_parameter.name.to_string(), ty)
            }
        }

        // let receiver_ty = self.ctx.type_db.receiver_types.get(&fun_ref);

        // for `obj.method()` obj is the first argument (passed to `self` parameter)
        if let Some(self_expr) = self_obj {
            let self_obj_ty = self.ctx.get_node_type(&self_expr);
            let params = &f_callable.0;
            if let Some(&param_ty) = params.first()
                && let Some(self_obj_ty) = self_obj_ty
            {
                let mut param_ty = param_ty;
                if self.intrn().has_generics(param_ty) {
                    param_ty =
                        deducing_ts.auto_deduce_from_argument(param_ty, self_obj_ty, self.intrn());
                }

                if let SymbolKind::Method {
                    is_mutable: true, ..
                } = &declaration.kind
                    && self.const_intrn().equals(self_obj_ty, param_ty)
                    && let Some(s_expr) = self.extract_sink_expression(self_expr)
                {
                    let ty = self.calc_declared_type_before_smart_cast(self_expr);
                    self.ctx.set_node_type(&self_expr, ty);
                    flow.register_known_type(s_expr, param_ty);
                }
            }
        }

        // loop over every argument, one by one, like control flow goes
        for (i, arg) in v.arguments().enumerate() {
            let Some(arg_i) = arg.expr() else {
                continue;
            };

            let param_idx = i + delta_self;
            let mut param_ty = f_callable
                .0
                .get(param_idx)
                .cloned()
                .unwrap_or_else(|| self.intrn().ty_undefined);

            param_ty = if self.intrn().has_generics(param_ty) {
                // `fun f<T>(a:T, b:T)` T was fixated on `a`, use it as hint for `b`
                deducing_ts.replace_ts_with_currently_deduced(param_ty, self.intrn())
            } else {
                param_ty
            };

            let after_arg = self.infer_expr(arg_i, flow, false, Some(param_ty));
            flow = after_arg.out_flow;

            let arg_ty = self.ctx.get_node_type_or_unknown(&arg_i);
            if self.intrn().has_generics(param_ty) {
                param_ty = deducing_ts.auto_deduce_from_argument(param_ty, arg_ty, self.intrn());
            }

            self.ctx.set_node_type(&arg_i, param_ty); // arg itself is an expression
            let is_mutable = parameters.get(i).is_some_and(|p| p.is_mutate);
            if is_mutable
                && !self.const_intrn().equals(arg_ty, param_ty)
                && let Some(s_expr) = self.extract_sink_expression(arg_i)
            {
                let ty = self.calc_declared_type_before_smart_cast(arg_i);
                self.ctx.set_node_type(&arg_i, ty);
                flow.register_known_type(s_expr, param_ty);
            }
        }

        let mut return_ty = f_callable.1;

        // if return type is omitted we need to infer function body first
        // once inferred, subsequent `return_ty` for this function will be non-auto
        if return_ty == self.const_intrn().ty_auto
            && let Some(inferred_ty) = self.infer_auto_return_type_of_function(declaration)
        {
            let func_ty = self.intrn().func(f_callable.0.clone(), inferred_ty);
            self.ctx.set_top_level_type(declaration.id, func_ty);
            return_ty = inferred_ty
        }

        let final_return_ty = if self.intrn().has_generics(return_ty) {
            let mut substitutor = TypeSubstitutor::new_with_defaults(self.intrn());
            substitutor.substitute(return_ty, &deducing_ts.substitutions.mapping)
        } else {
            return_ty
        };

        self.ctx.set_node_type(&v, final_return_ty);

        let func_ty = self.intrn().func(f_callable.0, final_return_ty);
        self.ctx.set_node_type(&callee, func_ty);

        let ty_never = self.const_intrn().ty_never;
        if self.const_intrn().equals(final_return_ty, ty_never) {
            flow.mark_unreachable(UnreachableKind::CallNeverReturnFunction)
        }

        ExprFlow::create(flow, as_cond)
    }

    fn return_type_or_none(&self, t: TyId) -> Option<(Vec<TyId>, TyId)> {
        if let TyData::Func { params, return_ty } = self.const_intrn().data(t) {
            Some((params.clone(), *return_ty))
        } else {
            None
        }
    }

    //+ CHECKED
    fn infer_tensor(
        &mut self,
        v: Tensor<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        let mut flow = flow;
        let tensor_hint = hint.and_then(|h| {
            let unwrapped = self.intrn().unwrap_alias(h);
            if let TyData::Tensor(items) = self.intrn().data(unwrapped).clone() {
                Some(items)
            } else {
                None
            }
        });

        let elements: Vec<_> = v.elements().collect();
        let mut types_list = Vec::with_capacity(elements.len());
        for (i, item) in elements.iter().enumerate() {
            let item_hint = tensor_hint.as_ref().and_then(|h| h.get(i).cloned());
            let after_item = self.infer_expr(*item, flow, false, item_hint);
            flow = after_item.out_flow;
            let item_ty = self.ctx.get_node_type_or_unknown(item);
            types_list.push(item_ty);
        }

        let ty = self.intrn().tensor(types_list);
        self.ctx.set_node_type(&v, ty);
        ExprFlow::create(flow, as_cond)
    }

    fn array_type_from_element(&mut self, element_ty: TyId, _preferred_hint: Option<TyId>) -> TyId {
        self.intrn().array(element_ty)
    }

    fn array_element_type(&self, ty: TyId) -> Option<TyId> {
        let ty = self.const_intrn().unwrap_alias(ty);
        match self.const_intrn().data(ty) {
            TyData::Array(item) => Some(*item),
            _ => None,
        }
    }

    fn lisp_list_element_type(&self, ty: TyId) -> Option<TyId> {
        let ty = self.const_intrn().unwrap_alias(ty);
        match self.const_intrn().data(ty) {
            TyData::Struct { name, args, .. } => {
                if name.as_ref() == "lisp_list" {
                    return args
                        .as_ref()
                        .and_then(|args| args.first().copied())
                        .or_else(|| Some(self.const_intrn().ty_undefined));
                }
                None
            }
            TyData::GenericTypeWithTs { inner_ty, types } => {
                if let TyData::Struct { name, .. } = self.const_intrn().data(*inner_ty)
                    && name.as_ref() == "lisp_list"
                {
                    return types
                        .first()
                        .copied()
                        .or_else(|| Some(self.const_intrn().ty_undefined));
                }
                None
            }
            _ => None,
        }
    }

    fn pick_unique_tuple_hint(&self, hint: TyId) -> Option<TyId> {
        let hint = self.const_intrn().unwrap_alias(hint);
        match self.const_intrn().data(hint).clone() {
            TyData::Tuple(_) => Some(hint),
            TyData::Union(variants) => {
                let mut found: Option<TyId> = None;
                for variant in variants {
                    if let Some(candidate) = self.pick_unique_tuple_hint(variant) {
                        if let Some(existing) = found {
                            if !self.const_intrn().equals(existing, candidate) {
                                return None;
                            }
                        } else {
                            found = Some(candidate);
                        }
                    }
                }
                found
            }
            _ => None,
        }
    }

    fn pick_unique_array_hint(&self, hint: TyId) -> Option<TyId> {
        let hint = self.const_intrn().unwrap_alias(hint);
        if self.array_element_type(hint).is_some() {
            return Some(hint);
        }

        if let TyData::Union(variants) = self.const_intrn().data(hint).clone() {
            let mut found: Option<TyId> = None;
            for variant in variants {
                if let Some(candidate) = self.pick_unique_array_hint(variant) {
                    if let Some(existing) = found {
                        if !self.const_intrn().equals(existing, candidate) {
                            return None;
                        }
                    } else {
                        found = Some(candidate);
                    }
                }
            }
            return found;
        }

        None
    }

    fn pick_unique_lisp_list_hint(&self, hint: TyId) -> Option<TyId> {
        let hint = self.const_intrn().unwrap_alias(hint);
        if self.lisp_list_element_type(hint).is_some() {
            return Some(hint);
        }

        if let TyData::Union(variants) = self.const_intrn().data(hint).clone() {
            let mut found: Option<TyId> = None;
            for variant in variants {
                if let Some(candidate) = self.pick_unique_lisp_list_hint(variant) {
                    if let Some(existing) = found {
                        if !self.const_intrn().equals(existing, candidate) {
                            return None;
                        }
                    } else {
                        found = Some(candidate);
                    }
                }
            }
            return found;
        }

        None
    }

    fn infer_typed_tuple(
        &mut self,
        v: Tuple<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        let mut flow = flow;
        let explicit_type = v.typ().map(|type_node| self.lower(Some(type_node)));

        let mut effective_hint = hint;
        if effective_hint
            .map(|h| {
                let unwrapped = self.const_intrn().unwrap_alias(h);
                matches!(self.const_intrn().data(unwrapped), TyData::Undefined)
            })
            .unwrap_or(true)
        {
            effective_hint = explicit_type;
        }

        let tuple_hint_items = effective_hint
            .and_then(|h| self.pick_unique_tuple_hint(h))
            .and_then(|h| {
                if let TyData::Tuple(items) = self.const_intrn().data(h).clone() {
                    Some(items)
                } else {
                    None
                }
            });

        let array_hint = effective_hint.and_then(|h| self.pick_unique_array_hint(h));
        let array_item_hint = array_hint.and_then(|h| self.array_element_type(h));

        let lisp_list_hint = effective_hint.and_then(|h| self.pick_unique_lisp_list_hint(h));
        let lisp_list_item_hint = lisp_list_hint.and_then(|h| self.lisp_list_element_type(h));

        let elements: Vec<_> = v.elements().collect();
        let mut types_list = Vec::with_capacity(elements.len());
        for (i, item) in elements.iter().enumerate() {
            let item_hint = array_item_hint
                .or(lisp_list_item_hint)
                .or_else(|| tuple_hint_items.as_ref().and_then(|h| h.get(i).copied()));

            let after_item = self.infer_expr(*item, flow, false, item_hint);
            flow = after_item.out_flow;
            let item_ty = self.ctx.get_node_type_or_unknown(item);
            types_list.push(item_ty);
        }

        if let Some(explicit_type) = explicit_type {
            self.ctx.set_node_type(&v, explicit_type);
            return ExprFlow::create(flow, as_cond);
        }

        if tuple_hint_items.is_some() {
            let ty = self.intrn().tuple(types_list);
            self.ctx.set_node_type(&v, ty);
            return ExprFlow::create(flow, as_cond);
        }

        if let Some(lisp_list_hint) = lisp_list_hint {
            self.ctx.set_node_type(&v, lisp_list_hint);
            return ExprFlow::create(flow, as_cond);
        }

        if types_list.is_empty() {
            let ty = if let Some(array_hint) = array_hint {
                let element_ty = self
                    .array_element_type(array_hint)
                    .unwrap_or_else(|| self.intrn().ty_unknown);
                self.array_type_from_element(element_ty, Some(array_hint))
            } else {
                let unknown_ty = self.intrn().ty_unknown;
                self.array_type_from_element(unknown_ty, None)
            };
            self.ctx.set_node_type(&v, ty);
            return ExprFlow::create(flow, as_cond);
        }

        let mut inferred_element_ty = types_list[0];
        for ty in types_list.iter().skip(1).copied() {
            inferred_element_ty = self.intrn().calculate_type_lca(inferred_element_ty, ty);
        }
        let ty = self.array_type_from_element(inferred_element_ty, array_hint);

        self.ctx.set_node_type(&v, ty);
        ExprFlow::create(flow, as_cond)
    }

    //+ CHECKED
    fn infer_null_literal(&mut self, v: NullLit<'t>, flow: FlowContext, as_cond: bool) -> ExprFlow {
        let ty = self.intrn().ty_null;
        self.ctx.set_node_type(&v, ty);
        ExprFlow::create(flow, as_cond)
    }

    pub(crate) fn infer_lambda_fun(
        &mut self,
        v: Lambda<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        // parameters of a lambda are allowed to be untyped: they are inferred before instantiation, e.g.
        // > fun call(f: (int) -> slice) { ... }
        // > call(fun(i) { ... })
        // then from a hint, calculate params_types=[int], return_type=slice
        let hint_unwrapped = hint.map(|h| self.const_intrn().unwrap_alias(h));
        let h_callable = hint_unwrapped.and_then(|h| self.return_type_or_none(h));

        let mut params_types = Vec::new();
        let mut body_start = FlowContext::new();

        for (i, param) in v.parameters().enumerate() {
            let param_ty = if let Some(ty_node) = param.typ() {
                self.lower(Some(ty_node))
            } else if let Some((h_params, _)) = &h_callable
                && i < h_params.len()
                && !self.const_intrn().has_generics(h_params[i])
            {
                h_params[i]
            } else {
                self.const_intrn().ty_undefined
            };
            params_types.push(param_ty);

            if let Some(name_node) = param.name() {
                let span = name_node.span();
                let def_id = LocalDefId::new(self.ctx.file_id, span.start);
                let name = self.text_of(&name_node);
                let sink = SinkExpr::from_def(name, def_id, 0);
                body_start.register_known_type(sink, param_ty);
                self.ctx.set_node_type(&name_node, param_ty);
            }
            self.ctx.set_node_type(&param, param_ty);
        }

        let return_type = if let Some(ret_node) = v.return_type() {
            self.lower(Some(ret_node))
        } else if let Some((_, h_ret)) = &h_callable
            && !self.const_intrn().has_generics(*h_ret)
        {
            *h_ret
        } else {
            self.const_intrn().ty_undefined
        };

        if let Some(body) = v.body() {
            let old_return_types = std::mem::take(&mut self.ctx.return_types);
            let old_declared_return_type = self.ctx.declared_return_ty;
            let old_inferred_return_type = self.ctx.inferred_return_type;

            self.ctx.declared_return_ty = if return_type == self.const_intrn().ty_undefined {
                None
            } else {
                Some(return_type)
            };
            self.ctx.inferred_return_type = None;

            let body_end = self.process_block_stmt(body, body_start);

            self.infer_return_type_if_needed(self.ctx.declared_return_ty, &body_end);

            let final_return_type = self.ctx.inferred_return_type.unwrap_or(return_type);

            self.ctx.return_types = old_return_types;
            self.ctx.declared_return_ty = old_declared_return_type;
            self.ctx.inferred_return_type = old_inferred_return_type;

            let lambda_ty = self.intrn().func(params_types, final_return_type);
            self.ctx.set_node_type(&v, lambda_ty);
        } else {
            let lambda_ty = self.intrn().func(params_types, return_type);
            self.ctx.set_node_type(&v, lambda_ty);
        }

        ExprFlow::create(flow, as_cond)
    }

    pub(crate) fn infer_match(
        &mut self,
        v: Match<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        let mut flow = flow;
        let subject_expr = v.expr();

        // If subject is LocalVarsDeclaration, we first infer it.
        // But infer_local_vars_declaration returns ExprFlow, and handles declaration logic.
        // For now, let's assume it's mostly Expression.
        // If it is LocalVarsDeclaration, we need to handle it.
        // Tolk C++: `flow = infer_any_expr(v->get_subject(), std::move(flow), false).out_flow;`
        // In Rust, LocalVarsDeclaration is a Statement in `statement_inference.rs`, but here it is wrapped in MatchExpr.
        // We can call `infer_expression` if it is Expression.
        // If it is LocalVarsDeclaration, we might need to duplicate logic or expose `infer_local_vars_declaration`.
        // `infer_local_vars_declaration` IS exposed in `TypeInferenceWalker` (in `expression_inference.rs` it seems? No, line 142 of expression_inference.rs)

        let subject_flow = if let Some(expr) = subject_expr {
            self.infer_expr(expr, flow, false, None)
        } else {
            return ExprFlow::create(flow, as_cond);
        };

        flow = subject_flow.out_flow;
        let s_expr = if let Some(expr) = subject_expr {
            self.extract_sink_expression(expr)
        } else {
            None
        };

        let subject_ty = if let Some(expr) = subject_expr {
            self.ctx.get_node_type_or_unknown(&expr.syntax())
        } else {
            self.intrn().ty_undefined
        };

        let mut branches_unifier = TypeInferringUnifyStrategy::new();
        let arms_entry_facts = flow.clone();
        let mut match_out_flow: Option<FlowContext> = None;

        let mut has_type_arm = false;
        let mut has_expr_arm = false;
        let mut has_else_arm = false;

        for arm in v.arms() {
            let mut arm_flow = arms_entry_facts.clone();

            // Infer pattern
            match arm.pattern() {
                MatchPattern::Type(ty_node) => {
                    let mut exact_type = self.lower(Some(ty_node));

                    // Handle generic instantiation logic (Wrapper<T> | int => Wrapper<int>)
                    match self.intrn().data(exact_type).clone() {
                        TyData::Struct { def, .. } => {
                            if let Some(inst_exact_type) =
                                self.try_pick_instantiated_generic_from_hint(subject_ty, def)
                            {
                                exact_type = inst_exact_type;
                                self.ctx.set_node_type(&ty_node.syntax(), exact_type);
                            }
                        }
                        TyData::TypeAlias { def, .. } => {
                            if let Some(inst_exact_type) =
                                self.try_pick_instantiated_generic_from_hint_alias(subject_ty, def)
                            {
                                exact_type = inst_exact_type;
                                self.ctx.set_node_type(&ty_node.syntax(), exact_type);
                            }
                        }
                        TyData::GenericTypeWithTs { inner_ty, .. } => {
                            match self.intrn().data(inner_ty).clone() {
                                TyData::Struct { def, .. } => {
                                    if let Some(inst_exact_type) = self
                                        .try_pick_instantiated_generic_from_hint(subject_ty, def)
                                    {
                                        exact_type = inst_exact_type;
                                        self.ctx.set_node_type(&ty_node.syntax(), exact_type);
                                    }
                                }
                                TyData::TypeAlias { def, .. } => {
                                    if let Some(inst_exact_type) = self
                                        .try_pick_instantiated_generic_from_hint_alias(
                                            subject_ty, def,
                                        )
                                    {
                                        exact_type = inst_exact_type;
                                        self.ctx.set_node_type(&ty_node.syntax(), exact_type);
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }

                    if let Some(ref s) = s_expr {
                        arm_flow.register_known_type(s.clone(), exact_type);
                    }
                    has_type_arm = true;
                }
                MatchPattern::Expr(p_expr) => {
                    arm_flow = self.infer_expr(p_expr, arm_flow, false, None).out_flow;
                    has_expr_arm = true;
                }
                MatchPattern::Else => {
                    has_else_arm = true;
                }
            }

            let _ = has_expr_arm;

            // Infer body
            let body_ty;
            if let Some(body) = arm.body() {
                match body {
                    MatchArmBody::Block(block) => {
                        arm_flow = self.process_block_stmt(block, arm_flow);
                        // Assuming Void for block statement unless we have yield (not supported yet?)
                        body_ty = self.intrn().ty_void;
                    }
                    MatchArmBody::Expr(expr) => {
                        let flow_res = self.infer_expr(expr, arm_flow, false, hint);
                        arm_flow = flow_res.out_flow;
                        body_ty = self.ctx.get_node_type_or_unknown(&expr.syntax());
                    }
                    MatchArmBody::Return(ret) => {
                        arm_flow = self.process_return_stmt(ret, arm_flow);
                        body_ty = self.intrn().ty_never;
                    }
                    MatchArmBody::Throw(throw) => {
                        arm_flow = self.process_throw_stmt(throw, arm_flow);
                        body_ty = self.intrn().ty_never;
                    }
                }
            } else {
                body_ty = self.intrn().ty_void;
            }

            match_out_flow = match match_out_flow {
                Some(f) => Some(f.merge_flow(&arm_flow, self.intrn())),
                None => Some(arm_flow),
            };

            branches_unifier.unify_with(body_ty, hint, self.intrn());
        }

        let mut final_flow = match_out_flow.unwrap_or(arms_entry_facts);

        // Exhaustiveness check (basic)
        // TODO: check enum members
        let is_exhaustive = has_else_arm || has_type_arm;

        // If not exhaustive, merge with implicit else (empty)
        if !is_exhaustive && v.arms().count() > 0 {
            // Implicit else -> fallthrough with entry facts
            // Actually, if it's not exhaustive, it means we might skip match?
            // In Tolk C++, it merges with else_flow (which is entry facts if no side effects)
            // `match_out_flow = FlowContext::merge_flow(std::move(match_out_flow), std::move(else_flow));`
            // where else_flow is `process_any_statement(empty_expression, arms_entry_facts)`
            // so it is essentially arms_entry_facts.
            // But we need to merge it with current final_flow.
            let else_flow = flow; // arms_entry_facts was derived from flow
            final_flow = final_flow.merge_flow(&else_flow, self.intrn());
        }

        let result_ty = branches_unifier.get_result(self.intrn());
        self.ctx.set_node_type(&v.0, result_ty);

        ExprFlow::create(final_flow, as_cond)
    }

    pub(crate) fn infer_object_literal(
        &mut self,
        v: ObjectLit<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        let mut flow = flow;

        // the goal is to detect struct_ref
        // either by lhs hint `var u: User = { ... }, or by explicitly provided ref `User { ... }`
        let mut struct_def_id = None;
        let mut struct_name = None;
        let mut explicit_ty = None;

        // `User { ... }` / `UserAlias { ... }` / `Wrapper { ... }` / `Wrapper<int> { ... }`
        if let Some(type_node) = v.typ() {
            let explicit_type = self.lower(Some(type_node));
            if let Some(def) = self.ctx.type_db.find_struct(explicit_type) {
                struct_def_id = Some(def);
                struct_name = self
                    .ctx
                    .type_db
                    .project_index
                    .resolve_symbol(def)
                    .map(|sym| sym.name.clone());
            }

            explicit_ty = Some(explicit_type);

            // example: `var v: Ok<int> = Ok { ... }`, now struct_ref is "Ok<T>", take "Ok<int>" from hint
            let is_generic = struct_def_id.is_some_and(|id| self.ctx.type_db.is_struct_generic(id));
            if is_generic
                && let Some(struct_id) = struct_def_id
                && let Some(hint) = hint
                && let Some(inst_explicit_ty) =
                    self.try_pick_instantiated_generic_from_hint(hint, struct_id)
                && let Some(inst_explicit_ref) = self.ctx.type_db.find_struct(inst_explicit_ty)
            {
                struct_def_id = Some(inst_explicit_ref)
            }
        }

        // try to find struct def from hint
        if struct_def_id.is_none()
            && let Some(h) = hint
        {
            if let Some(def) = self.ctx.type_db.find_struct(h) {
                struct_def_id = Some(def);
                struct_name = self
                    .ctx
                    .type_db
                    .project_index
                    .resolve_symbol(def)
                    .map(|sym| sym.name.clone());
            } else {
                let unwrapped = self.const_intrn().unwrap_alias(h);
                if let TyData::Union(variants) = self.const_intrn().data(unwrapped) {
                    // Find struct in variants
                    let mut found_def = None;
                    for &var in variants {
                        if let Some(def) = self.ctx.type_db.find_struct(var) {
                            if found_def.is_some() {
                                found_def = None; // Ambiguous
                                break;
                            }
                            struct_name = self
                                .ctx
                                .type_db
                                .project_index
                                .resolve_symbol(def)
                                .map(|sym| sym.name.clone());
                            found_def = Some(def);
                        }
                    }
                    struct_def_id = found_def;
                }
            }
        }

        let Some(def_id) = struct_def_id else {
            return ExprFlow::create(flow, as_cond);
        };
        let Some(struct_name) = struct_name else {
            return ExprFlow::create(flow, as_cond);
        };

        // so, we have struct_ref, so we can check field names and infer values
        // if it's a generic struct, we need to deduce Ts by field values, like for a function call
        let mut deducing_ts = GenericSubstitutionsDeducing::new();

        for arg in v.arguments() {
            let Some(name_node) = arg.name() else {
                continue;
            };
            let field_name = self.text_of(&name_node);

            if let Some(field) = self.ctx.type_db.find_struct_field(def_id, &field_name) {
                self.ctx.set_resolved(NameUse {
                    decl: self.ctx.decl_start,
                    span: name_node.span(),
                    kind: NameUseKind::Value,
                    name: field.name.clone(),
                    resolved: Resolved::Global(field.id),
                });

                let mut field_ty = field.declared_type;
                if self.const_intrn().has_generics(field_ty) {
                    field_ty = deducing_ts.replace_ts_with_currently_deduced(field_ty, self.intrn())
                }

                if let Some(val) = arg.value() {
                    let after_val = self.infer_expr(val, flow, false, Some(field_ty));
                    flow = after_val.out_flow;

                    let val_ty = self.ctx.get_node_type_or_unknown(&val);
                    if self.const_intrn().has_generics(field_ty) {
                        deducing_ts.auto_deduce_from_argument(field_ty, val_ty, self.intrn());
                    }

                    self.ctx.set_node_type(&arg, val_ty);
                } else {
                    // incomplete code
                    self.ctx.set_node_type(&arg, field_ty);
                }
            } else if let Some(val) = arg.value() {
                // unknown field, just infer value type
                flow = self.infer_expr(val, flow, false, None).out_flow;
            }
        }

        let mut result_ty = explicit_ty
            .or_else(|| self.ctx.get_top_level_type(def_id))
            .unwrap_or_else(|| self.intrn().struct_ty(def_id, struct_name));

        if self.const_intrn().has_generics(result_ty) {
            let mut substitutor = TypeSubstitutor::new_with_defaults(self.intrn());
            result_ty = substitutor.substitute(result_ty, &deducing_ts.substitutions.mapping);
        }
        if explicit_ty.is_some() {
            result_ty = self.intrn().unwrap_alias(result_ty);
        }

        self.ctx.set_node_type(&v, result_ty);
        ExprFlow::create(flow, as_cond)
    }

    fn infer_instantiation(
        &mut self,
        v: Instantiation,
        flow: FlowContext,
        as_cond: bool,
    ) -> ExprFlow {
        let Some(expr) = v.expr() else {
            return ExprFlow::create(flow, as_cond);
        };
        let mut flow = flow;

        // Dot-access instantiations (`obj.method<T>`) need dot resolution to expose method symbol.
        if let Expr::DotAccess(dot) = expr {
            flow = self
                .infer_dot_access(dot, flow, false, None, None, None)
                .out_flow;
        }

        let resolved_symbol = match expr {
            Expr::Ident(ident) => self.ctx.get_resolved_node(&ident),
            Expr::DotAccess(dot) => match dot.field() {
                Some(DotAccessField::Ident(field_ident)) => {
                    self.ctx.get_resolved_node(&field_ident)
                }
                _ => None,
            },
            _ => self.ctx.get_resolved_node(&expr),
        };

        if let Some(resolved) = resolved_symbol
            && let Resolved::Global(id) = resolved.resolved
            && let Some(symbol) = self.ctx.type_db.project_index.resolve_symbol(id)
        {
            if symbol.is_type()
                && let Some(ty) = self.ctx.type_db.convert_instantiated(&v, self.ctx.file_id)
            {
                self.ctx.set_node_type(&v, ty);
            } else if let Some(instantiation_types) = v.instantiation_ts() {
                let mut substituted_ts = GenericsSubstitutions::new();
                let provided_types = instantiation_types
                    .types()
                    .map(|t| self.ctx.type_db.lower_type(self.ctx.file_id, &t))
                    .collect::<Vec<_>>();

                let type_parameters = match &symbol.kind {
                    SymbolKind::Function {
                        type_parameters, ..
                    } => Some(type_parameters),
                    SymbolKind::Method {
                        type_parameters, ..
                    } => Some(type_parameters),
                    SymbolKind::GetMethod {
                        type_parameters, ..
                    } => Some(type_parameters),
                    _ => None,
                };

                if let Some(type_parameters) = type_parameters {
                    for (param, ty) in type_parameters.iter().zip(provided_types) {
                        substituted_ts.set_type_t(param.name.to_string(), ty);
                    }
                }

                if let Some(mut func_ty) = self
                    .ctx
                    .get_node_type(&expr)
                    .or_else(|| self.ctx.get_top_level_type(symbol.id))
                {
                    if self.intrn().has_generics(func_ty) {
                        let mut substitutor = TypeSubstitutor::new_with_defaults(self.intrn());
                        func_ty = substitutor.substitute(func_ty, &substituted_ts.mapping);
                    }
                    self.ctx.set_node_type(&v, func_ty);
                }
            }

            return ExprFlow::create(flow, as_cond);
        }

        if let Some(ty) = self.ctx.type_db.convert_instantiated(&v, self.ctx.file_id) {
            self.ctx.set_node_type(&v, ty);
        }
        ExprFlow::create(flow, as_cond)
    }

    //+ CHECKED
    fn infer_underscore(
        &mut self,
        v: Underscore<'t>,
        flow: FlowContext,
        as_cond: bool,
        hint: Option<TyId>,
    ) -> ExprFlow {
        // if execution is here, underscore is either used as lhs of assignment, or incorrectly, like `f(_)`
        // more precise is to always set unknown here, but for incorrect usages, instead of an error
        // "can not pass unknown to X" would better be an error it can't be used as a value, at later steps
        let ty = hint.unwrap_or_else(|| self.intrn().ty_unknown);
        self.ctx.set_node_type(&v.0, ty);
        ExprFlow::create(flow, as_cond)
    }

    /// infer return type "on demand"
    /// example: `fun f() { return g(); } fun g() { ... }`
    /// when analyzing `f()`, we need to infer what fun_ref=g returns
    fn infer_auto_return_type_of_function(&mut self, symbol: &Symbol) -> Option<TyId> {
        if self.ctx.call_stack.contains(&symbol.id) {
            // prevent recursion of untyped functions, like `fun f() { return g(); } fun g() { return f(); }`
            return None;
        }

        self.ctx.call_stack.push_back(symbol.id);
        let file = self.ctx.type_db.file_db.get_by_id(symbol.id.file_id)?;
        let func_decl = file.find_syntax_declaration(symbol.id)?;

        let ctx = InferenceContext::new(
            symbol.id.file_id,
            self.ctx.type_db,
            self.ctx.call_stack.clone(), // to prevent recursive dependencies
        );
        let mut walker = TypeInferenceWalker::new(ctx);
        walker.ctx.decl_start = func_decl.syntax().start_byte() as u32;

        match &func_decl {
            TopLevel::Func(fun) => {
                walker.infer_function_base(fun, symbol.id);
            }
            TopLevel::Method(method) => {
                walker.infer_method(method, symbol.id);
            }
            TopLevel::GetMethod(method) => {
                walker.infer_function_base(method, symbol.id);
            }
            _ => {}
        }
        self.ctx.call_stack.pop_back();

        let result = InferenceResult::new(walker.ctx);
        result.inferred_return_type
    }
}
